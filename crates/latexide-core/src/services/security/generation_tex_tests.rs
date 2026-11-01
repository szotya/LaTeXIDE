//! Fresh host-generation TeX evidence; only own fixtures and existing tools.
use super::super::execution::{HostCommand, WritableRoots, with_test_command_capture};
use super::*;
use std::{
    fs,
    io::Write,
    net::{Ipv4Addr, TcpListener},
    os::unix::{fs::PermissionsExt, process::CommandExt},
    process::{Command, Stdio},
    sync::{Mutex, MutexGuard},
    time::{Duration, Instant},
};

struct Authority(Mutex<HostSnapshot>);
impl HostInputAuthority for Authority {
    type Lease<'a> = MutexGuard<'a, HostSnapshot>;
    fn try_lease_current(&self) -> Result<Self::Lease<'_>, GenerationError> {
        self.0
            .try_lock()
            .map_err(|_| GenerationError::HostUnavailable)
    }
}
fn private(path: &Path) {
    fs::create_dir(path).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
}
fn xml(path: &Path) -> String {
    path.to_str()
        .unwrap()
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
fn nul_fields(path: &Path, fields: &[String]) {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .unwrap();
    for field in fields {
        file.write_all(field.as_bytes()).unwrap();
        file.write_all(&[0]).unwrap();
    }
}
fn record_control(command: &Command, prefix: &Path) {
    use std::os::unix::ffi::OsStrExt;
    let mut argv = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(prefix.with_extension("argv.nul"))
        .unwrap();
    for argument in std::iter::once(command.get_program()).chain(command.get_args()) {
        argv.write_all(argument.as_bytes()).unwrap();
        argv.write_all(&[0]).unwrap();
    }
    let mut env = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(prefix.with_extension("env.nul"))
        .unwrap();
    for (key, value) in command.get_envs() {
        if let Some(value) = value {
            env.write_all(key.as_bytes()).unwrap();
            env.write_all(&[0]).unwrap();
            env.write_all(value.as_bytes()).unwrap();
            env.write_all(&[0]).unwrap();
        }
    }
}
fn control(mut command: Command, output: &Path) {
    let stdout = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output.join("compiler.stdout.log"))
        .unwrap();
    let stderr = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output.join("compiler.stderr.log"))
        .unwrap();
    command
        .stdin(Stdio::null())
        .stdout(stdout)
        .stderr(stderr)
        .process_group(0);
    struct OwnChild {
        child: std::process::Child,
        reaped: bool,
    }
    impl OwnChild {
        fn terminate(&mut self) {
            let pid = i32::try_from(self.child.id()).unwrap();
            unsafe extern "C" {
                fn kill(pid: i32, signal: i32) -> i32;
            }
            if pid > 1 {
                unsafe {
                    kill(-pid, 9);
                }
            }
            let _ = self.child.kill();
        }
    }
    impl Drop for OwnChild {
        fn drop(&mut self) {
            if !self.reaped {
                self.terminate();
                let _ = self.child.wait();
            }
        }
    }
    let mut own = OwnChild {
        child: command.spawn().unwrap(),
        reaped: false,
    };
    let start = Instant::now();
    loop {
        #[repr(C, align(16))]
        struct Info([u8; 256]);
        unsafe extern "C" {
            fn waitid(kind: i32, id: u32, info: *mut std::ffi::c_void, flags: i32) -> i32;
        }
        let mut info = Info([0; 256]);
        assert_eq!(
            unsafe { waitid(1, own.child.id(), info.0.as_mut_ptr().cast(), 1 | 4 | 32) },
            0
        );
        if i32::from_ne_bytes(info.0[..4].try_into().unwrap()) != 0 {
            own.terminate();
            let status = own.child.wait().unwrap();
            own.reaped = true;
            assert!(
                status.success(),
                "own unsandboxed positive control failed {status}"
            );
            break;
        }
        assert!(
            start.elapsed() <= Duration::from_secs(30),
            "own bounded control timeout"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}
const PROBE: &str = r#"
use Fcntl qw(O_RDONLY O_WRONLY O_APPEND O_CREAT O_EXCL);
use Socket qw(PF_INET SOCK_STREAM IPPROTO_TCP sockaddr_in);
use Errno qw(EPERM EACCES);
my ($inside,$original,$outside,$output,$port,$role,$source)=@ARGV;
sub verdict {my ($ok)=@_;return 'allowed' if $ok;my $e=0+$!;die "unexpected errno $e" unless $e==EPERM||$e==EACCES;return 'denied';}
sub readbyte {my ($p)=@_;my $r=sysopen(my $f,$p,O_RDONLY);$r&&=defined(sysread($f,my $b,1));my $v=verdict($r);close($f) if $r;return $v;}
my $input=readbyte("$inside/main.tex");my $origin=readbyte("$original/main.tex");my $read=readbyte("$outside/sentinel");
my $w=sysopen(my $f,"$outside/sentinel",O_WRONLY|O_APPEND);$w&&=defined(syswrite($f,'probe'));my $write=verdict($w);close($f) if $w;
my $n=socket(my $sock,PF_INET,SOCK_STREAM,IPPROTO_TCP);$n&&=connect($sock,sockaddr_in($port,pack('C4',127,0,0,1)));my $net=verdict($n);close($sock) if $n;
my $secret=exists($ENV{P18_SYNTHETIC_SECRET})?'present':'absent';
sysopen(my $receipt,"$output/$role.receipt",O_WRONLY|O_CREAT|O_EXCL,0600) or die $!;
print $receipt "inside=$input\noriginal=$origin\nread=$read\nwrite=$write\nnetwork=$net\nsynthetic_env=$secret\n";close($receipt) or die $!;
if ($role eq 'parent') {my $c=fork();defined($c) or die $!;if (!$c) {exec('/usr/bin/perl','-e',$source,'--',$inside,$original,$outside,$output,$port,'child',$source);die $!;}waitpid($c,0);die 'child failed' if $?;}
"#;

#[test]
#[ignore = "opt-in: real existing offline TeX through host-owned generation; retains Phase3 evidence"]
fn generation_real_tex_offline() {
    let phase =
        PathBuf::from(std::env::var_os("LATEXIDE_P18_PHASE3_ROOT").expect("own Phase3 root"))
            .canonicalize()
            .unwrap();
    let label = std::env::var("LATEXIDE_P18_TEX_RUN").unwrap();
    assert!(label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'));
    let run = phase.join("artifacts").join(label);
    private(&run);
    let project = run.join("Árvíztűrő projekt");
    private(&project);
    private(&project.join("részek"));
    let prefix = PathBuf::from(std::env::var_os("LATEXIDE_P18_TEX_PREFIX").unwrap())
        .canonicalize()
        .unwrap();
    let bin = prefix.join("bin/universal-darwin");
    let main="\u{feff}\\documentclass{article}\n\\usepackage{fontspec}\n\\setmainfont{lmroman10-regular.otf}[BoldFont=lmroman10-bold.otf,ItalicFont=lmroman10-italic.otf,BoldItalicFont=lmroman10-bolditalic.otf]\n\\usepackage{amsmath}\n\\begin{document}\n\\section{P18 Phase3}\nÁrvíztűrő tükörfúrógép: áéíóöőúüű.\n\\input{részek/szöveg}\nA képlet száma: \\eqref{eq:eredmeny}.\n\\end{document}\n".replace('\n',"\r\n").into_bytes();
    let part="\\section{Második fájl}\r\nSaját host-generáció, többfájlos offline teszt.\r\n\\begin{equation}\\label{eq:eredmeny} E=mc^2,\\quad \\alpha+\\beta=\\gamma.\\end{equation}\r\n".as_bytes().to_vec();
    fs::write(project.join("main.tex"), &main).unwrap();
    fs::write(project.join("részek/szöveg.tex"), &part).unwrap();
    let outside = run.join("outside");
    private(&outside);
    fs::write(outside.join("sentinel"), b"own outside sentinel").unwrap();
    let make = |role: &str, generation: u64| {
        let storage = run.join(format!("{role}-generation"));
        let font_name = format!("{role}-fonts.conf");
        let font_bytes=format!("<?xml version=\"1.0\"?><!DOCTYPE fontconfig SYSTEM \"urn:fontconfig:fonts.dtd\"><fontconfig><dir>{}</dir><cachedir>{}</cachedir></fontconfig>",xml(&prefix.join("texmf-dist/fonts/opentype/public/lm")),xml(&storage.join("cache/fontconfig"))).into_bytes();
        fs::write(project.join(&font_name), &font_bytes).unwrap();
        let host = HostSnapshot::new(
            [31; 16],
            generation,
            project.clone(),
            "main.tex".into(),
            vec![
                HostInput::disk("main.tex", 10, main.clone()).unwrap(),
                HostInput::disk("részek/szöveg.tex", 12, part.clone()).unwrap(),
                HostInput::disk(&font_name, 1, font_bytes).unwrap(),
            ],
        )
        .unwrap();
        let mut host_fields = vec![
            host.binding()
                .workspace()
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect(),
            host.binding().generation().to_string(),
            host.project.to_str().unwrap().into(),
            host.main.clone(),
            host.binding().digest().hex(),
            host.files.len().to_string(),
        ];
        for input in &host.files {
            host_fields.extend([
                input.relative.clone(),
                input.revision.to_string(),
                input.disk_digest.hex(),
                input.digest.hex(),
                input.bytes.len().to_string(),
            ]);
        }
        nul_fields(&run.join(format!("{role}-host-inputs.nul")), &host_fields);
        let expected = host.binding().clone();
        let authority = Authority(Mutex::new(host));
        let captured =
            CapturedInputs::capture(&authority, &expected, &storage, &AtomicBool::new(false))
                .unwrap();
        assert_eq!(
            fs::read(captured.input_root().join("main.tex")).unwrap(),
            main
        );
        let mut configuration = None;
        let prepared = captured
            .prepare(&authority, &AtomicBool::new(false), |layout| {
                let config = HostExecutionConfig {
                    trusted: true,
                    project_root: layout.input_root().into(),
                    project_inputs: layout.inputs().to_vec(),
                    read_only_roots: vec![prefix.clone()],
                    writable: WritableRoots {
                        output: layout.output_root().into(),
                        cache: layout.cache_root().into(),
                        temporary: layout.temporary_root().into(),
                    },
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
                            format!("-outdir={}", layout.output_root().display()).into(),
                            layout.main_input().as_os_str().into(),
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
                        font_config: layout.input_root().join(&font_name),
                    },
                    sandbox_executable: "/usr/bin/sandbox-exec".into(),
                    timeout: Duration::from_secs(30),
                };
                configuration = Some(config.clone());
                Ok(config)
            })
            .unwrap();
        (
            authority,
            expected,
            prepared,
            configuration.unwrap(),
            storage,
        )
    };
    let (_control_host, _control_binding, control_prepared, control_config, control_storage) =
        make("control", 20);
    let blueprint = control_prepared.runner.test_blueprint().unwrap();
    let mut positive = Command::new(&control_config.command.executable);
    positive
        .args(&control_config.command.arguments)
        .current_dir(&control_config.project_root)
        .env_clear();
    for (key, value) in blueprint.get_envs() {
        if let Some(value) = value {
            positive.env(key, value);
        }
    }
    record_control(&positive, &run.join("control"));
    control(positive, &control_config.writable.output);
    assert!(control_storage.join("output/main.pdf").is_file());

    let (authority, expected, prepared, configuration, storage) = make("protected", 21);
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let port = listener.local_addr().unwrap().port().to_string();
    let mut probe_config = configuration.clone();
    probe_config.command.arguments = vec![
        "-e".into(),
        PROBE.into(),
        "--".into(),
        configuration.project_root.as_os_str().into(),
        project.as_os_str().into(),
        outside.as_os_str().into(),
        configuration.writable.output.as_os_str().into(),
        port.into(),
        "parent".into(),
        PROBE.into(),
    ];
    let probe = CompilerSandbox::new(probe_config).unwrap();
    let probe_result = with_test_command_capture(&run.join("protected-probe"), || {
        probe.run(&AtomicBool::new(false))
    })
    .unwrap();
    assert_eq!(
        probe_result.state,
        ExecutionState::Succeeded,
        "probe failed {:?}",
        probe_result.stderr_log
    );
    for role in ["parent", "child"] {
        assert_eq!(
            fs::read_to_string(
                configuration
                    .writable
                    .output
                    .join(format!("{role}.receipt"))
            )
            .unwrap(),
            "inside=allowed\noriginal=denied\nread=denied\nwrite=denied\nnetwork=denied\nsynthetic_env=absent\n"
        );
    }
    let probe_evidence = run.join("protected-probe-evidence");
    private(&probe_evidence);
    for entry in fs::read_dir(&configuration.writable.output).unwrap() {
        let entry = entry.unwrap();
        fs::rename(entry.path(), probe_evidence.join(entry.file_name())).unwrap();
    }
    assert_eq!(
        fs::read_dir(&configuration.writable.output)
            .unwrap()
            .count(),
        0
    );
    assert_eq!(
        fs::read_dir(&configuration.writable.cache).unwrap().count(),
        0
    );
    assert_eq!(
        fs::read_dir(&configuration.writable.temporary)
            .unwrap()
            .count(),
        0
    );
    let pending = with_test_command_capture(&run.join("protected-tex"), || {
        prepared.run(&authority, &AtomicBool::new(false))
    })
    .unwrap();
    let receipt=pending.with_current_result(&authority,&AtomicBool::new(false),|artifact| {
        assert_eq!(artifact.binding(),&expected);assert!(authority.0.try_lock().is_err());
        assert_eq!(artifact.bytes(),fs::read(storage.join("output/main.pdf")).unwrap());
        format!("workspace={}\ngeneration={}\nmanifest_sha256={}\noutput_sha256={}\npublication_host_locked=true\nsource_main_sha256={}\nsource_part_sha256={}\n",artifact.binding().workspace().iter().map(|b|format!("{b:02x}")).collect::<String>(),artifact.binding().generation(),artifact.binding().digest().hex(),artifact.content_digest().hex(),Digest::of(&main).unwrap().hex(),Digest::of(&part).unwrap().hex())
    }).unwrap();
    fs::write(run.join("generation-publication.receipt"), receipt).unwrap();
    assert_eq!(fs::read(project.join("main.tex")).unwrap(), main);
    assert_eq!(fs::read(project.join("részek/szöveg.tex")).unwrap(), part);
    assert_eq!(
        fs::read(outside.join("sentinel")).unwrap(),
        b"own outside sentinel"
    );
    for output in [&control_storage, &storage] {
        let log = fs::read_to_string(output.join("output/main.log")).unwrap();
        assert!(!log.contains("There were undefined references"));
        assert!(!log.contains("Rerun to get cross-references right"));
    }
    println!("P18_PHASE3_TEX_ARTIFACTS={}", run.display());
    println!(
        "Fresh host-generation control/protected PDF bytes exist; actual parser/text/render validation is still a separate required step"
    );
}
