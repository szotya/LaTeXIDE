//! Phase2 tests use real processes and private fixtures; the app is not wired.
use super::*;
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(1);

#[path = "inherited_fd_tests.rs"]
mod inherited_fd_tests;
struct Fixture {
    base: PathBuf,
    project: PathBuf,
    outside: PathBuf,
    roots: WritableRoots,
}
impl Fixture {
    fn new() -> Self {
        let base = std::env::temp_dir().join(format!(
            "latexide-p18-execution-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&base).unwrap();
        fs::set_permissions(&base, fs::Permissions::from_mode(0o700)).unwrap();
        let base = base.canonicalize().unwrap();
        let project = base.join("project");
        let outside = base.join("outside");
        let roots = WritableRoots {
            output: base.join("output"),
            cache: base.join("cache"),
            temporary: base.join("temporary"),
        };
        for path in [
            &project,
            &outside,
            &roots.output,
            &roots.cache,
            &roots.temporary,
        ] {
            fs::create_dir(path).unwrap();
            fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
        }
        fs::write(project.join("main.tex"), b"own project").unwrap();
        fs::write(outside.join("sentinel"), b"own outside sentinel").unwrap();
        Self {
            base,
            project,
            outside,
            roots,
        }
    }
    fn config(&self, script: &str) -> HostExecutionConfig {
        HostExecutionConfig {
            trusted: true,
            project_root: self.project.clone(),
            project_inputs: vec![self.project.join("main.tex")],
            read_only_roots: vec![],
            writable: self.roots.clone(),
            command: HostCommand {
                executable: "/usr/bin/perl".into(),
                arguments: vec!["-e".into(), script.into()],
            },
            executable_allowlist: vec!["/usr/bin/perl".into()],
            environment: ExecutionEnvironment::Minimal,
            sandbox_executable: "/usr/bin/sandbox-exec".into(),
            timeout: Duration::from_secs(3),
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.base);
    }
}

#[test]
fn phase2_adapter_alone_does_not_attest_content_freshness() {
    use std::{fs::FileTimes, os::unix::fs::MetadataExt};
    let f = Fixture::new();
    let input = f.project.join("main.tex");
    let receipt = f.roots.output.join("consumed-input.bytes");
    let mut config = f.config("open(my $i,'<',$ARGV[0]) or die $!; binmode($i); local $/; my $s=<$i>; open(my $o,'>',$ARGV[1]) or die $!; binmode($o); print $o $s; close($o) or die $!;");
    config.command.arguments.extend([
        input.clone().into_os_string(),
        receipt.clone().into_os_string(),
    ]);
    let runner = CompilerSandbox::new(config).unwrap();
    let before = fs::metadata(&input).unwrap();
    fs::write(&input, b"bad project").unwrap();
    fs::File::options()
        .write(true)
        .open(&input)
        .unwrap()
        .set_times(FileTimes::new().set_modified(before.modified().unwrap()))
        .unwrap();
    let after = fs::metadata(&input).unwrap();
    assert_eq!(before.ino(), after.ino());
    assert_eq!(before.len(), after.len());
    assert_eq!(before.modified().unwrap(), after.modified().unwrap());
    let result = runner.run(&AtomicBool::new(false));
    eprintln!(
        "PHASE3_FROZEN_GAP result={result:?}; consumed={:?}",
        fs::read(&receipt)
    );
    std::mem::forget(f); // Preserve the own RED fixture and raw child artifacts.
    assert!(matches!(
        result,
        Ok(ExecutionOutcome {
            state: ExecutionState::Succeeded,
            ..
        })
    ));
    assert_eq!(fs::read(receipt).unwrap(), b"bad project");
}

#[test]
fn compiler_adapter_runs_a_real_child_with_only_declared_writes() {
    let f = Fixture::new();
    let receipt = f.roots.output.join("child.receipt");
    let mut config = f.config(
        "open(my $f, '>', $ARGV[0]) or die $!; print $f 'protected child'; close($f) or die $!;",
    );
    config
        .command
        .arguments
        .push(receipt.clone().into_os_string());
    let runner = CompilerSandbox::new(config).unwrap();
    let outcome = runner.run(&AtomicBool::new(false)).unwrap();
    assert_eq!(outcome.state, ExecutionState::Succeeded);
    assert_eq!(fs::read(receipt).unwrap(), b"protected child");
    assert_eq!(
        fs::read(f.outside.join("sentinel")).unwrap(),
        b"own outside sentinel"
    );
}

#[test]
fn compiler_adapter_rejects_missing_or_substitute_isolation_before_the_child() {
    let f = Fixture::new();
    for sandbox in [
        f.base.join("missing-sandbox"),
        PathBuf::from("/usr/bin/true"),
    ] {
        let mut config = f.config("die 'must never execute'");
        config.sandbox_executable = sandbox;
        assert!(matches!(
            CompilerSandbox::new(config),
            Err(ExecutionError::IsolationUnavailable)
        ));
    }
    assert_eq!(fs::read_dir(&f.roots.output).unwrap().count(), 0);
}

#[test]
fn compiler_adapter_rejects_overlapping_read_only_and_writable_roots() {
    let f = Fixture::new();
    let mut config = f.config("die 'must never execute'");
    config.read_only_roots.push(f.roots.cache.clone());
    assert!(matches!(
        CompilerSandbox::new(config),
        Err(ExecutionError::InvalidConfiguration)
    ));
    let mut config = f.config("die 'must never execute'");
    config.writable.output = f.base.clone();
    assert!(matches!(
        CompilerSandbox::new(config),
        Err(ExecutionError::InvalidConfiguration)
    ));
}

#[test]
fn compiler_adapter_cancelled_before_launch_creates_no_child_or_output() {
    let f = Fixture::new();
    let runner = CompilerSandbox::new(f.config("die 'must never execute'")).unwrap();
    assert_eq!(
        runner.run(&AtomicBool::new(true)).unwrap().state,
        ExecutionState::Cancelled
    );
    assert_eq!(fs::read_dir(&f.roots.output).unwrap().count(), 0);
}

#[test]
fn compiler_adapter_untrusted_host_project_never_starts_code() {
    let f = Fixture::new();
    let mut config = f.config("die 'must never execute'");
    config.trusted = false;
    assert!(matches!(
        CompilerSandbox::new(config),
        Err(ExecutionError::Untrusted)
    ));
    assert_eq!(fs::read_dir(&f.roots.output).unwrap().count(), 0);
}

#[test]
fn compiler_adapter_invalid_profile_fails_without_executing_the_target() {
    let f = Fixture::new();
    let receipt = f.roots.output.join("must-not-exist");
    let mut config = f.config("open(my $f,'>',$ARGV[0]) or die $!; print $f 'ran';");
    config
        .command
        .arguments
        .push(receipt.clone().into_os_string());
    let runner = CompilerSandbox::new(config).unwrap();
    let result = runner
        .prepared
        .run(
            &AtomicBool::new(false),
            "(version 1) (invalid-p18-phase2-form)",
        )
        .unwrap();
    assert_eq!(result.state, ExecutionState::Failed);
    assert_ne!(result.exit_code, Some(0));
    assert!(!receipt.exists());
    assert!(!fs::read(result.stderr_log.unwrap()).unwrap().is_empty());
}

#[test]
fn compiler_adapter_timeout_terminates_the_child_and_ordinary_descendant() {
    let f = Fixture::new();
    let script = r#"my $pid=fork(); defined($pid) or die $!; if (!$pid) { open(my $p,'>>',$ARGV[0]) or die $!; $p->autoflush(1); while(1) { print $p 'x'; select(undef,undef,undef,0.01); } } sleep(10);"#;
    let pulse = f.roots.output.join("descendant.pulse");
    let mut config = f.config(script);
    config
        .command
        .arguments
        .push(pulse.clone().into_os_string());
    config.timeout = Duration::from_millis(200);
    let runner = CompilerSandbox::new(config).unwrap();
    let outcome = runner.run(&AtomicBool::new(false)).unwrap();
    assert_eq!(outcome.state, ExecutionState::TimedOut);
    let size = fs::metadata(&pulse).unwrap().len();
    assert!(size > 0, "descendant must actually have run");
    std::thread::sleep(Duration::from_millis(80));
    assert_eq!(fs::metadata(pulse).unwrap().len(), size);
}

#[test]
fn compiler_adapter_cancel_terminates_the_child_and_ordinary_descendant() {
    let f = Fixture::new();
    let pulse = f.roots.output.join("descendant.pulse");
    let script = r#"my $pid=fork(); defined($pid) or die $!; if (!$pid) { open(my $p,'>>',$ARGV[0]) or die $!; $p->autoflush(1); while(1) { print $p 'x'; select(undef,undef,undef,0.01); } } sleep(10);"#;
    let mut config = f.config(script);
    config
        .command
        .arguments
        .push(pulse.clone().into_os_string());
    let runner = CompilerSandbox::new(config).unwrap();
    let cancel = AtomicBool::new(false);
    let result = std::thread::scope(|scope| {
        let worker = scope.spawn(|| runner.run(&cancel));
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while !fs::metadata(&pulse).is_ok_and(|m| m.len() > 0) {
            assert!(
                std::time::Instant::now() < deadline,
                "controlled descendant did not start"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
        cancel.store(true, Ordering::Release);
        worker.join().unwrap().unwrap()
    });
    assert_eq!(result.state, ExecutionState::Cancelled);
    let size = fs::metadata(&pulse).unwrap().len();
    std::thread::sleep(Duration::from_millis(80));
    assert_eq!(fs::metadata(pulse).unwrap().len(), size);
}

#[test]
fn compiler_adapter_natural_exit_does_not_leave_a_running_ordinary_descendant() {
    let f = Fixture::new();
    let pulse = f.roots.output.join("descendant.pulse");
    let script = r#"my $pid=fork(); defined($pid) or die $!; if (!$pid) { open(my $p,'>>',$ARGV[0]) or die $!; $p->autoflush(1); while(1) { print $p 'x'; select(undef,undef,undef,0.01); } } for (1..100) { exit(0) if -s $ARGV[0]; select(undef,undef,undef,0.01); } die 'no descendant';"#;
    let mut config = f.config(script);
    config
        .command
        .arguments
        .push(pulse.clone().into_os_string());
    let runner = CompilerSandbox::new(config).unwrap();
    let result = runner.run(&AtomicBool::new(false)).unwrap();
    assert_eq!(result.state, ExecutionState::Succeeded);
    let size = fs::metadata(&pulse).unwrap().len();
    assert!(size > 0);
    std::thread::sleep(Duration::from_millis(80));
    assert_eq!(fs::metadata(pulse).unwrap().len(), size);
}

#[test]
fn compiler_adapter_enforces_file_size_limit_before_target_writes() {
    let f = Fixture::new();
    let target = f.roots.output.join("bounded.log");
    let mut config = f.config("open(my $f,'>',$ARGV[0]) or die $!; my $b='x' x 65536; while(1) { print $f $b or die $!; }");
    config
        .command
        .arguments
        .push(target.clone().into_os_string());
    let result = CompilerSandbox::new(config)
        .unwrap()
        .run(&AtomicBool::new(false))
        .unwrap();
    assert_eq!(result.state, ExecutionState::Failed);
    assert_eq!(fs::metadata(target).unwrap().len(), 16 * 1024 * 1024);
}

const BOUNDARY_PROBE: &str = r#"
use Fcntl qw(O_RDONLY O_WRONLY O_APPEND O_CREAT O_EXCL);
use Socket qw(PF_INET SOCK_STREAM IPPROTO_TCP sockaddr_in);
use Errno qw(EPERM EACCES);
my ($project,$outside,$output,$port,$role,$source)=@ARGV;
sub verdict { my ($ok)=@_; return 'allowed' if $ok; my $e=0+$!; die "unexpected errno $e" unless $e==EPERM || $e==EACCES; return 'denied'; }
my $inside = sysopen(my $i,"$project/main.tex",O_RDONLY); $inside &&= defined(sysread($i,my $b,1)); my $ir=verdict($inside); close($i) if $inside;
my $r=sysopen(my $s,"$outside/sentinel",O_RDONLY); $r &&= defined(sysread($s,my $c,1)); my $read=verdict($r); close($s) if $r;
my $w=sysopen(my $t,"$outside/sentinel",O_WRONLY|O_APPEND); $w &&= defined(syswrite($t,'probe')); my $write=verdict($w); close($t) if $w;
my $n=socket(my $sock,PF_INET,SOCK_STREAM,IPPROTO_TCP); $n &&= connect($sock,sockaddr_in($port,pack('C4',127,0,0,1))); my $net=verdict($n); close($sock) if $n;
my $secret=exists($ENV{P18_SYNTHETIC_SECRET}) ? 'present' : 'absent';
sysopen(my $receipt,"$output/$role.receipt",O_WRONLY|O_CREAT|O_EXCL,0600) or die $!;
print $receipt "inside=$ir\nread=$read\nwrite=$write\nnetwork=$net\nsynthetic_env=$secret\n"; close($receipt) or die $!;
if ($role eq 'parent') { my $child=fork(); defined($child) or die $!; if (!$child) { exec('/usr/bin/perl','-e',$source,'--',$project,$outside,$output,$port,'child',$source); die $!; } waitpid($child,0); die 'child failed' if $?; }
"#;

#[test]
fn compiler_adapter_enforces_boundary_and_environment_in_exec_descendant() {
    use std::{
        net::{Ipv4Addr, TcpListener},
        process::Command,
    };
    if std::env::var("P18_SYNTHETIC_SECRET").ok().as_deref() != Some("OWN_P18_TEST_ONLY") {
        // Give only this private test subprocess a known synthetic parent value.
        // Removing env_clear from the real adapter must then fail this test.
        let temp = std::env::temp_dir();
        let mut child = Command::new(std::env::current_exe().unwrap());
        child.env_clear().env("TMPDIR",temp).env("P18_SYNTHETIC_SECRET","OWN_P18_TEST_ONLY")
            .args(["--exact","security_service::execution::tests::compiler_adapter_enforces_boundary_and_environment_in_exec_descendant","--nocapture"]);
        let output = child.output().unwrap();
        assert!(
            output.status.success(),
            "synthetic-parent subprocess failed: {} {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    let f = Fixture::new();
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let port = listener.local_addr().unwrap().port().to_string();
    let arguments = |output: &PathBuf| {
        vec![
            "-e".into(),
            BOUNDARY_PROBE.into(),
            "--".into(),
            f.project.clone().into_os_string(),
            f.outside.clone().into_os_string(),
            output.clone().into_os_string(),
            port.clone().into(),
            "parent".into(),
            BOUNDARY_PROBE.into(),
        ]
    };
    // Explicit test-only positive control, never a production fallback.
    let control = Command::new("/usr/bin/perl")
        .args(arguments(&f.roots.output))
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("P18_SYNTHETIC_SECRET", "OWN_P18_TEST_ONLY")
        .current_dir(&f.project)
        .output()
        .unwrap();
    assert!(
        control.status.success(),
        "positive control: {}",
        String::from_utf8_lossy(&control.stderr)
    );
    for role in ["parent", "child"] {
        assert_eq!(
            fs::read_to_string(f.roots.output.join(format!("{role}.receipt"))).unwrap(),
            "inside=allowed\nread=allowed\nwrite=allowed\nnetwork=allowed\nsynthetic_env=present\n"
        );
    }
    fs::write(f.outside.join("sentinel"), b"own outside sentinel").unwrap();
    let protected_output = f.base.join("protected-output");
    fs::create_dir(&protected_output).unwrap();
    fs::set_permissions(&protected_output, fs::Permissions::from_mode(0o700)).unwrap();
    let mut config = f.config(BOUNDARY_PROBE);
    config.writable.output = protected_output.clone();
    config.command.arguments = arguments(&protected_output);
    let outcome = CompilerSandbox::new(config)
        .unwrap()
        .run(&AtomicBool::new(false))
        .unwrap();
    assert_eq!(
        outcome.state,
        ExecutionState::Succeeded,
        "stderr: {}",
        fs::read_to_string(outcome.stderr_log.as_ref().unwrap()).unwrap()
    );
    for role in ["parent", "child"] {
        assert_eq!(
            fs::read_to_string(protected_output.join(format!("{role}.receipt"))).unwrap(),
            "inside=allowed\nread=denied\nwrite=denied\nnetwork=denied\nsynthetic_env=absent\n"
        );
    }
    assert_eq!(
        fs::read(f.outside.join("sentinel")).unwrap(),
        b"own outside sentinel"
    );
    println!(
        "P18 execution boundary: parent and fork/exec descendant denied outside read/write and loopback connect; own synthetic credential absent. Profile {:?}, stdout {:?}",
        outcome.profile, outcome.stdout_log
    );
}

#[test]
fn compiler_adapter_changed_read_only_root_and_reused_output_fail_closed() {
    let f = Fixture::new();
    let resource = f.base.join("runtime");
    fs::create_dir(&resource).unwrap();
    let mut config = f.config("exit(0)");
    config.read_only_roots.push(resource.clone());
    let runner = CompilerSandbox::new(config).unwrap();
    fs::rename(&resource, f.base.join("old-runtime")).unwrap();
    fs::create_dir(&resource).unwrap();
    assert!(matches!(
        runner.run(&AtomicBool::new(false)),
        Err(ExecutionError::IdentityChanged)
    ));
    assert_eq!(fs::read_dir(&f.roots.output).unwrap().count(), 0);
    let f = Fixture::new();
    let runner = CompilerSandbox::new(f.config("exit(0)")).unwrap();
    assert_eq!(
        runner.run(&AtomicBool::new(false)).unwrap().state,
        ExecutionState::Succeeded
    );
    assert!(matches!(
        runner.run(&AtomicBool::new(false)),
        Err(ExecutionError::OutputNotFresh)
    ));
}

#[test]
fn compiler_adapter_trusted_selected_shell_stays_inside_the_sandbox() {
    let f = Fixture::new();
    let receipt = f.roots.output.join("shell.receipt");
    let mut config = f.config("");
    config.command = HostCommand {
        executable: "/bin/sh".into(),
        arguments: vec![
            "-c".into(),
            "printf 'own shell' > \"$1\"".into(),
            "p18".into(),
            receipt.clone().into_os_string(),
        ],
    };
    config.executable_allowlist = vec!["/bin/sh".into(), "/bin/bash".into()];
    let result = CompilerSandbox::new(config)
        .unwrap()
        .run(&AtomicBool::new(false))
        .unwrap();
    assert_eq!(result.state, ExecutionState::Succeeded);
    assert_eq!(fs::read(receipt).unwrap(), b"own shell");
}

#[test]
fn compiler_adapter_allows_only_metadata_for_declared_path_ancestors() {
    let f = Fixture::new();
    let receipt = f.roots.output.join("metadata.receipt");
    let mut config=f.config("use Fcntl qw(O_RDONLY);use Errno qw(EPERM EACCES); lstat($ARGV[0]) or die 'ancestor metadata denied'; if(sysopen(my $f,$ARGV[1],O_RDONLY)){die 'outside contents allowed'} die 'wrong error' unless (0+$!)==EPERM || (0+$!)==EACCES; open(my $r,'>',$ARGV[2]) or die $!;print $r 'metadata only';close($r);");
    config.command.arguments.extend([
        f.base.clone().into_os_string(),
        f.outside.join("sentinel").into_os_string(),
        receipt.clone().into_os_string(),
    ]);
    let result = CompilerSandbox::new(config)
        .unwrap()
        .run(&AtomicBool::new(false))
        .unwrap();
    assert_eq!(result.state, ExecutionState::Succeeded);
    assert_eq!(fs::read(receipt).unwrap(), b"metadata only");
}

#[test]
fn compiler_adapter_cannot_read_outside_sentinel_through_preexisting_project_hardlink() {
    let f = Fixture::new();
    let alias = f.project.join("hardlink-alias");
    fs::hard_link(f.outside.join("sentinel"), &alias).unwrap();
    let mut config=f.config("use Fcntl qw(O_RDONLY);use Errno qw(EPERM EACCES); my $ok=sysopen(my $f,$ARGV[0],O_RDONLY);if($ok){my $n=sysread($f,my $b,1);die 'OUTSIDE_HARDLINK_READ_BYTE_COUNT='.$n if defined($n) && $n>0;die 'unexpected EOF' if defined($n);} die 'unexpected error' unless (0+$!)==EPERM || (0+$!)==EACCES;");
    config.command.arguments.push(alias.into_os_string());
    let result = CompilerSandbox::new(config)
        .unwrap()
        .run(&AtomicBool::new(false))
        .unwrap();
    assert_eq!(
        result.state,
        ExecutionState::Succeeded,
        "OS must deny the alias; stderr {}",
        fs::read_to_string(result.stderr_log.unwrap()).unwrap()
    );
}

#[test]
fn compiler_adapter_cannot_read_outside_sentinel_through_project_symlink() {
    let f = Fixture::new();
    let alias = f.project.join("symlink-alias");
    std::os::unix::fs::symlink(f.outside.join("sentinel"), &alias).unwrap();
    let mut config=f.config("use Fcntl qw(O_RDONLY);use Errno qw(EPERM EACCES); if(sysopen(my $f,$ARGV[0],O_RDONLY)){die 'outside symlink contents readable'} die 'unexpected error' unless (0+$!)==EPERM || (0+$!)==EACCES;");
    config.command.arguments.push(alias.into_os_string());
    let result = CompilerSandbox::new(config)
        .unwrap()
        .run(&AtomicBool::new(false))
        .unwrap();
    assert_eq!(result.state, ExecutionState::Succeeded);
}

#[test]
fn compiler_adapter_cannot_create_an_outside_hardlink_in_its_output() {
    let f = Fixture::new();
    let alias = f.roots.output.join("late-alias");
    let mut config=f.config("use Errno qw(EPERM EACCES); if(link($ARGV[0],$ARGV[1])){die 'outside hardlink creation allowed'} die 'unexpected error' unless (0+$!)==EPERM || (0+$!)==EACCES;");
    config.command.arguments.extend([
        f.outside.join("sentinel").into_os_string(),
        alias.clone().into_os_string(),
    ]);
    let result = CompilerSandbox::new(config)
        .unwrap()
        .run(&AtomicBool::new(false))
        .unwrap();
    assert_eq!(result.state, ExecutionState::Succeeded);
    assert!(!alias.exists());
}

#[test]
fn compiler_adapter_declared_hardlink_input_is_rejected_before_execution() {
    let f = Fixture::new();
    let alias = f.project.join("alias.tex");
    fs::hard_link(f.outside.join("sentinel"), &alias).unwrap();
    let mut config = f.config("die 'must never execute'");
    config.project_inputs.push(alias);
    assert!(matches!(
        CompilerSandbox::new(config),
        Err(ExecutionError::IdentityChanged)
    ));
}

#[test]
fn compiler_adapter_prepopulated_writable_roots_are_rejected_before_execution() {
    let f = Fixture::new();
    fs::hard_link(f.outside.join("sentinel"), f.roots.cache.join("alias")).unwrap();
    assert!(matches!(
        CompilerSandbox::new(f.config("die 'must never execute'")),
        Err(ExecutionError::InvalidConfiguration)
    ));
}

#[test]
fn compiler_adapter_project_cannot_be_reclassified_as_trusted_runtime_root() {
    let f = Fixture::new();
    let mut config = f.config("die 'must never execute'");
    config.read_only_roots.push(f.project.clone());
    assert!(matches!(
        CompilerSandbox::new(config),
        Err(ExecutionError::InvalidConfiguration)
    ));
}

#[test]
fn compiler_adapter_replaced_declared_input_prevents_launch() {
    let f = Fixture::new();
    let runner = CompilerSandbox::new(f.config("die 'must never execute'")).unwrap();
    fs::rename(f.project.join("main.tex"), f.project.join("old.tex")).unwrap();
    fs::write(f.project.join("main.tex"), b"replacement").unwrap();
    assert!(matches!(
        runner.run(&AtomicBool::new(false)),
        Err(ExecutionError::IdentityChanged)
    ));
    assert_eq!(fs::read_dir(&f.roots.output).unwrap().count(), 0);
}

#[test]
fn compiler_adapter_cannot_link_a_read_only_input_into_writable_output() {
    let f = Fixture::new();
    let alias = f.roots.output.join("input-alias");
    let mut config=f.config("use Errno qw(EPERM EACCES); if(link($ARGV[0],$ARGV[1])){die 'read-only input hardlinked into output'} die 'unexpected error' unless (0+$!)==EPERM || (0+$!)==EACCES;");
    config.command.arguments.extend([
        f.project.join("main.tex").into_os_string(),
        alias.clone().into_os_string(),
    ]);
    let result = CompilerSandbox::new(config)
        .unwrap()
        .run(&AtomicBool::new(false))
        .unwrap();
    assert_eq!(
        result.state,
        ExecutionState::Succeeded,
        "stderr {}",
        fs::read_to_string(result.stderr_log.unwrap()).unwrap()
    );
    assert!(!alias.exists());
    assert_eq!(
        fs::read(f.project.join("main.tex")).unwrap(),
        b"own project"
    );
}

#[test]
fn compiler_adapter_late_hardlink_and_cache_injection_prevent_launch() {
    let f = Fixture::new();
    let runner = CompilerSandbox::new(f.config("die 'must never execute'")).unwrap();
    fs::hard_link(
        f.project.join("main.tex"),
        f.outside.join("late-input-alias"),
    )
    .unwrap();
    assert!(matches!(
        runner.run(&AtomicBool::new(false)),
        Err(ExecutionError::IdentityChanged)
    ));
    assert_eq!(fs::read_dir(&f.roots.output).unwrap().count(), 0);
    let f = Fixture::new();
    let receipt = f.roots.output.join("must-not-exist");
    let mut config = f.config("open(my $f,'>',$ARGV[0]) or die $!;print $f 'ran';");
    config
        .command
        .arguments
        .push(receipt.clone().into_os_string());
    let runner = CompilerSandbox::new(config).unwrap();
    fs::hard_link(f.outside.join("sentinel"), f.roots.cache.join("late-alias")).unwrap();
    assert!(matches!(
        runner.run(&AtomicBool::new(false)),
        Err(ExecutionError::IdentityChanged)
    ));
    assert!(!receipt.exists());
}

#[test]
fn compiler_adapter_child_cannot_replace_the_recorded_profile_silently() {
    let f = Fixture::new();
    let mut config =
        f.config("open(my $f,'>',$ARGV[0]) or die $!;print $f 'fake profile';close($f);");
    config
        .command
        .arguments
        .push(f.roots.output.join("isolation.profile.sb").into_os_string());
    let runner = CompilerSandbox::new(config).unwrap();
    assert!(matches!(
        runner.run(&AtomicBool::new(false)),
        Err(ExecutionError::EvidenceChanged)
    ));
}

fn record_command(command: &std::process::Command, destination: &std::path::Path) {
    use std::{io::Write, os::unix::ffi::OsStrExt};
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)
        .unwrap();
    for arg in std::iter::once(command.get_program()).chain(command.get_args()) {
        file.write_all(arg.as_bytes()).unwrap();
        file.write_all(&[0]).unwrap();
    }
}

fn run_positive_control(mut command: std::process::Command, output: &std::path::Path) {
    use std::{os::unix::process::CommandExt, process::Stdio};
    let stdout = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output.join("control.stdout.log"))
        .unwrap();
    let stderr = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output.join("control.stderr.log"))
        .unwrap();
    command
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr))
        .stdin(Stdio::null())
        .process_group(0);
    let mut child = command.spawn().unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(
                status.success(),
                "positive compiler control failed; preserved logs at {:?}",
                output
            );
            break;
        }
        if std::time::Instant::now() >= deadline {
            unsafe extern "C" {
                fn kill(pid: i32, signal: i32) -> i32;
            }
            // No reap has occurred on this path, so the child's group ID is reserved.
            unsafe {
                kill(-(child.id() as i32), 9);
            }
            let _ = child.kill();
            let _ = child.wait();
            panic!("positive control timed out; not a PASS");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
#[ignore = "opt-in: existing TeX Live only; preserves phase2 compiler/PDF evidence"]
fn compiler_adapter_real_tex_offline() {
    use std::{
        io::Write,
        net::{Ipv4Addr, TcpListener},
        process::Command,
    };
    let phase = PathBuf::from(
        std::env::var_os("LATEXIDE_P18_PHASE2_ROOT").expect("own phase2 root required"),
    )
    .canonicalize()
    .unwrap();
    let label = std::env::var("LATEXIDE_P18_TEX_RUN").unwrap();
    assert!(!label.is_empty() && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'));
    let run = phase.join("artifacts").join(label);
    fs::create_dir(&run).unwrap();
    fs::set_permissions(&run, fs::Permissions::from_mode(0o700)).unwrap();
    let project = run.join("Árvíztűrő projekt");
    fs::create_dir(&project).unwrap();
    fs::set_permissions(&project, fs::Permissions::from_mode(0o700)).unwrap();
    let prefix = PathBuf::from(
        std::env::var_os("LATEXIDE_P18_TEX_PREFIX").expect("existing verified TeX prefix required"),
    )
    .canonicalize()
    .unwrap();
    let bin = prefix.join("bin/universal-darwin");
    let main = r#"\documentclass{article}
\usepackage{fontspec}
\setmainfont{lmroman10-regular.otf}[BoldFont=lmroman10-bold.otf,ItalicFont=lmroman10-italic.otf,BoldItalicFont=lmroman10-bolditalic.otf]
\usepackage{amsmath}
\begin{document}
\section{P18 Phase2}
Árvíztűrő tükörfúrógép: áéíóöőúüű.
\input{szöveg}
A képlet száma: \eqref{eq:eredmeny}.
\end{document}
"#;
    fs::write(project.join("main.tex"), main).unwrap();
    fs::write(project.join("szöveg.tex"),"\\section{Második fájl}\nSaját offline, többfájlos teszt.\n\\begin{equation}\\label{eq:eredmeny} E=mc^2,\\quad \\alpha+\\beta=\\gamma.\\end{equation}\n").unwrap();
    let outside = run.join("outside");
    fs::create_dir(&outside).unwrap();
    fs::write(outside.join("sentinel"), b"own outside sentinel").unwrap();
    let make_config = |role: &str| {
        let base = run.join(role);
        fs::create_dir(&base).unwrap();
        let roots = WritableRoots {
            output: base.join("output"),
            cache: base.join("cache"),
            temporary: base.join("temporary"),
        };
        for p in [&roots.output, &roots.cache, &roots.temporary] {
            fs::create_dir(p).unwrap();
            fs::set_permissions(p, fs::Permissions::from_mode(0o700)).unwrap();
        }
        let font = project.join(format!("{role}-fonts.conf"));
        let xml_escape = |p: &std::path::Path| {
            p.to_str()
                .unwrap()
                .replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('>', "&gt;")
        };
        fs::write(&font,format!("<?xml version=\"1.0\"?><!DOCTYPE fontconfig SYSTEM \"urn:fontconfig:fonts.dtd\"><fontconfig><dir>{}</dir><cachedir>{}</cachedir></fontconfig>",xml_escape(&prefix.join("texmf-dist/fonts/opentype/public/lm")),xml_escape(&roots.cache.join("fontconfig")))).unwrap();
        HostExecutionConfig {
            trusted: true,
            project_root: project.clone(),
            project_inputs: vec![
                project.join("main.tex"),
                project.join("szöveg.tex"),
                font.clone(),
            ],
            read_only_roots: vec![prefix.clone()],
            writable: roots.clone(),
            command: HostCommand {
                executable: "/usr/bin/perl".into(),
                arguments: vec![
                    bin.join("latexmk").into_os_string(),
                    "-norc".into(),
                    "-xelatex".into(),
                    "-interaction=nonstopmode".into(),
                    "-halt-on-error".into(),
                    "-file-line-error".into(),
                    "-latexoption=-no-shell-escape".into(),
                    "-synctex=1".into(),
                    "-recorder".into(),
                    "-g".into(),
                    format!("-outdir={}", roots.output.display()).into(),
                    project.join("main.tex").into_os_string(),
                ],
            },
            executable_allowlist: vec![
                "/usr/bin/perl".into(),
                "/bin/sh".into(),
                "/bin/bash".into(),
                bin.join("xelatex"),
                bin.join("xdvipdfmx"),
                bin.join("kpsewhich"),
            ],
            environment: ExecutionEnvironment::TexLive {
                prefix: prefix.clone(),
                font_config: font,
            },
            sandbox_executable: "/usr/bin/sandbox-exec".into(),
            timeout: Duration::from_secs(30),
        }
    };
    let control_config = make_config("control");
    let control_adapter = CompilerSandbox::new(control_config.clone()).unwrap();
    let blueprint = control_adapter
        .prepared
        .command(&control_adapter.prepared.profile)
        .unwrap();
    let mut control = Command::new(&control_config.command.executable);
    control
        .args(&control_config.command.arguments)
        .current_dir(&project)
        .env_clear();
    for (key, value) in blueprint.get_envs() {
        if let Some(value) = value {
            control.env(key, value);
        }
    }
    record_command(&control, &run.join("control.argv.nul"));
    assert!(!control_config.writable.output.join("main.pdf").exists());
    run_positive_control(control, &control_config.writable.output);
    assert!(control_config.writable.output.join("main.pdf").is_file());

    let protected_config = make_config("protected");
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let port = listener.local_addr().unwrap().port().to_string();
    let mut probe_config = protected_config.clone();
    probe_config.command.arguments = vec![
        "-e".into(),
        BOUNDARY_PROBE.into(),
        "--".into(),
        project.clone().into_os_string(),
        outside.clone().into_os_string(),
        protected_config.writable.output.clone().into_os_string(),
        port.into(),
        "parent".into(),
        BOUNDARY_PROBE.into(),
    ];
    let probe = CompilerSandbox::new(probe_config).unwrap();
    record_command(
        &probe.prepared.command(&probe.prepared.profile).unwrap(),
        &run.join("protected-probe.argv.nul"),
    );
    let probe_result = probe.run(&AtomicBool::new(false)).unwrap();
    assert_eq!(
        probe_result.state,
        ExecutionState::Succeeded,
        "protected preflight failed; logs {:?}",
        probe_result.stderr_log
    );
    for role in ["parent", "child"] {
        assert_eq!(
            fs::read_to_string(
                protected_config
                    .writable
                    .output
                    .join(format!("{role}.receipt"))
            )
            .unwrap(),
            "inside=allowed\nread=denied\nwrite=denied\nnetwork=denied\nsynthetic_env=absent\n"
        );
    }
    assert_eq!(
        fs::read(outside.join("sentinel")).unwrap(),
        b"own outside sentinel"
    );
    let archive = run.join("protected-probe-evidence");
    fs::create_dir(&archive).unwrap();
    for name in [
        "isolation.profile.sb",
        "compiler.stdout.log",
        "compiler.stderr.log",
        "parent.receipt",
        "child.receipt",
    ] {
        fs::rename(
            protected_config.writable.output.join(name),
            archive.join(name),
        )
        .unwrap();
    }
    let protected = CompilerSandbox::new(protected_config.clone()).unwrap();
    let blueprint = protected
        .prepared
        .command(&protected.prepared.profile)
        .unwrap();
    record_command(&blueprint, &run.join("protected-tex.argv.nul"));
    let mut envfile = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(run.join("protected.env.nul"))
        .unwrap();
    use std::os::unix::ffi::OsStrExt;
    for (key, value) in blueprint.get_envs() {
        if let Some(value) = value {
            envfile.write_all(key.as_bytes()).unwrap();
            envfile.write_all(&[0]).unwrap();
            envfile.write_all(value.as_bytes()).unwrap();
            envfile.write_all(&[0]).unwrap();
        }
    }
    assert!(!protected_config.writable.output.join("main.pdf").exists());
    let result = protected.run(&AtomicBool::new(false)).unwrap();
    assert_eq!(
        result.state,
        ExecutionState::Succeeded,
        "protected TeX failed; preserved logs at {:?}",
        result.stderr_log
    );
    assert_eq!(result.exit_code, Some(0));
    assert!(protected_config.writable.output.join("main.pdf").is_file());
    assert_eq!(
        fs::read(archive.join("isolation.profile.sb")).unwrap(),
        fs::read(result.profile.unwrap()).unwrap(),
        "probe and TeX must consume byte-identical profiles"
    );
    for role in ["control", "protected"] {
        let log = fs::read_to_string(run.join(role).join("output/main.log")).unwrap();
        assert!(
            !log.contains("undefined references") && !log.contains("LaTeX Warning: Reference"),
            "cross-reference unresolved"
        );
    }
    println!("P18_PHASE2_TEX_ARTIFACTS={}", run.display());
    println!(
        "Fresh control/protected PDFs exist; parser/render validation is a separate required step, not inferred from exit0 or prefix"
    );
}
