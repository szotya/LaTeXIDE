//! Own synthetic descriptor controls; no user/session descriptors are inspected.
use super::*;
use std::{
    fs::{File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    os::fd::{AsRawFd, FromRawFd, OwnedFd},
    process::Command,
};

unsafe extern "C" {
    fn fcntl(fd: i32, command: i32, ...) -> i32;
    fn pipe(fds: *mut i32) -> i32;
}

const PROBE: &str = r#"
use POSIX (); $|=1; use Errno qw(EBADF EPERM EACCES);
my ($read,$write,$path,$source,$role)=@ARGV;
sub status { my ($n)=@_; return "allowed:$n" if defined($n); return 'closed' if $! == EBADF; die "unexpected fd errno: $!"; }
my $r=status(POSIX::read($read,my $buffer,12));
my $w=status(POSIX::write($write,'OWN_FD_PROBE',12));
my $p=open(my $outside,'>>',$path); my $path_result=$p ? 'allowed' : ($! == EPERM || $! == EACCES ? 'denied' : die "unexpected path errno $!"); close($outside) if $p;
print "$role read=$r write=$w path=$path_result\n";
if ($role eq 'parent') { my $pid=fork(); defined($pid) or die $!; if (!$pid) { exec('/usr/bin/perl','-e',$source,'--',$read,$write,$path,$source,'descendant'); die $!; } waitpid($pid,0); die 'descendant failed' if $?; }
"#;

struct OwnDescriptors {
    read: File,
    write: File,
    flags: [(i32, i32); 2],
    is_pipe: bool,
}
impl OwnDescriptors {
    fn new(f: &Fixture, is_pipe: bool, high: bool, cloexec: bool) -> Self {
        let (read, write) = if is_pipe {
            let mut fds = [-1; 2];
            assert_eq!(unsafe { pipe(fds.as_mut_ptr()) }, 0);
            unsafe { (File::from_raw_fd(fds[0]), File::from_raw_fd(fds[1])) }
        } else {
            fs::write(f.outside.join("fd-read"), b"OWN_READ_FD!").unwrap();
            fs::write(f.outside.join("fd-write"), b"own sentinel").unwrap();
            (
                File::open(f.outside.join("fd-read")).unwrap(),
                OpenOptions::new()
                    .append(true)
                    .open(f.outside.join("fd-write"))
                    .unwrap(),
            )
        };
        let duplicate = |file: File, minimum: i32| {
            if high {
                let fd = unsafe { fcntl(file.as_raw_fd(), 0, minimum) }; // F_DUPFD
                assert!(
                    fd >= minimum,
                    "own high duplicate failed: {}",
                    std::io::Error::last_os_error()
                );
                File::from(unsafe { OwnedFd::from_raw_fd(fd) })
            } else {
                file
            }
        };
        let read = duplicate(read, 4096);
        let mut write = duplicate(write, 8192);
        for file in [&read, &write] {
            assert_eq!(unsafe { fcntl(file.as_raw_fd(), 2, i32::from(cloexec)) }, 0); // F_SETFD
            if is_pipe {
                let flags = unsafe { fcntl(file.as_raw_fd(), 3) }; // F_GETFL
                assert!(flags >= 0);
                assert_eq!(unsafe { fcntl(file.as_raw_fd(), 4, flags | 4) }, 0); // O_NONBLOCK
            }
        }
        if is_pipe {
            write.write_all(b"OWN_PIPE_IN!").unwrap();
        }
        let flags = [&read, &write]
            .map(|file| unsafe { (fcntl(file.as_raw_fd(), 1), fcntl(file.as_raw_fd(), 3)) });
        Self {
            read,
            write,
            flags,
            is_pipe,
        }
    }
    fn arguments(&self, f: &Fixture) -> Vec<std::ffi::OsString> {
        vec![
            "-e".into(),
            PROBE.into(),
            "--".into(),
            self.read.as_raw_fd().to_string().into(),
            self.write.as_raw_fd().to_string().into(),
            f.outside.join("sentinel").into_os_string(),
            PROBE.into(),
            "parent".into(),
        ]
    }
    fn assert_parent_unchanged(mut self, f: &Fixture) {
        for (file, expected) in [&self.read, &self.write].into_iter().zip(self.flags) {
            assert_eq!(
                unsafe { (fcntl(file.as_raw_fd(), 1), fcntl(file.as_raw_fd(), 3)) },
                expected,
                "parent descriptor flags changed"
            );
        }
        if self.is_pipe {
            let mut bytes = [0; 12];
            self.read.read_exact(&mut bytes).unwrap();
            assert_eq!(
                &bytes, b"OWN_PIPE_IN!",
                "child consumed or modified parent pipe"
            );
            assert!(self.read.read(&mut [0; 1]).is_err());
            self.write.write_all(b"parent alive").unwrap();
            self.read.read_exact(&mut bytes).unwrap();
            assert_eq!(&bytes, b"parent alive");
        } else {
            assert_eq!(
                self.read.stream_position().unwrap(),
                0,
                "child advanced parent offset"
            );
            assert_eq!(self.write.stream_position().unwrap(), 0);
            assert_eq!(
                fs::read(f.outside.join("fd-read")).unwrap(),
                b"OWN_READ_FD!"
            );
            assert_eq!(
                fs::read(f.outside.join("fd-write")).unwrap(),
                b"own sentinel"
            );
            let mut bytes = [0; 12];
            self.read.read_exact(&mut bytes).unwrap();
            assert_eq!(&bytes, b"OWN_READ_FD!");
            self.write.write_all(b"parent alive").unwrap();
            assert_eq!(
                fs::read(f.outside.join("fd-write")).unwrap(),
                b"own sentinelparent alive"
            );
            self.read.seek(SeekFrom::Start(0)).unwrap();
        }
        assert_eq!(
            fs::read(f.outside.join("sentinel")).unwrap(),
            b"own outside sentinel"
        );
    }
}

fn boundary(is_pipe: bool, high: bool, cloexec: bool) {
    // Explicit control on a different own fixture. Never a product fallback.
    if !cloexec {
        let f = Fixture::new();
        let descriptors = OwnDescriptors::new(&f, is_pipe, high, false);
        let output = Command::new("/usr/bin/perl")
            .args(descriptors.arguments(&f))
            .env_clear()
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let text = String::from_utf8(output.stdout).unwrap();
        assert!(
            text.contains("parent read=allowed:12 write=allowed:12 path=allowed"),
            "control: {text}"
        );
        assert!(
            text.contains("descendant write=allowed:12")
                || text.contains("write=allowed:12 path=allowed")
        );
        println!("P18_PHASE4_CONTROL pipe={is_pipe} high={high}: {text}");
    }
    let f = Fixture::new();
    let descriptors = OwnDescriptors::new(&f, is_pipe, high, cloexec);
    let fd_numbers = (descriptors.read.as_raw_fd(), descriptors.write.as_raw_fd());
    let mut config = f.config(PROBE);
    config.command.arguments = descriptors.arguments(&f);
    let result = CompilerSandbox::new(config)
        .unwrap()
        .run(&AtomicBool::new(false))
        .unwrap();
    let stdout = fs::read_to_string(result.stdout_log.as_ref().unwrap()).unwrap();
    let stderr = fs::read_to_string(result.stderr_log.as_ref().unwrap()).unwrap();
    let retained = f.base.clone();
    println!(
        "P18_PHASE4_PROTECTED pipe={is_pipe} high={high} cloexec={cloexec} fds={fd_numbers:?} state={:?} raw={retained:?}\n{stdout}\n{stderr}",
        result.state
    );
    // Retain raw files/logs both on RED and GREEN in the explicit own TMPDIR.
    let holder = std::mem::ManuallyDrop::new(f);
    assert_eq!(result.state, ExecutionState::Succeeded, "{stderr}");
    assert_eq!(
        stdout,
        "parent read=closed write=closed path=denied\ndescendant read=closed write=closed path=denied\n",
        "outside descriptor crossed exec"
    );
    descriptors.assert_parent_unchanged(&holder);
}

#[test]
fn phase4_regular_non_cloexec_fd_is_not_inherited() {
    boundary(false, false, false);
}
#[test]
fn phase4_high_duplicated_non_cloexec_fd_is_not_inherited() {
    boundary(false, true, false);
}
#[test]
fn phase4_high_pipe_non_cloexec_fd_is_not_inherited() {
    boundary(true, true, false);
}
#[test]
fn phase4_existing_cloexec_fd_control() {
    boundary(false, true, true);
}

#[test]
fn phase4_standard_io_and_compiler_created_exec_pipe_work() {
    let f = Fixture::new();
    let config = f.config(
        r#"
        my $n=sysread(STDIN,my $empty,1); defined($n) && $n==0 or die 'stdin';
        print STDOUT "own stdout\n"; print STDERR "own stderr\n"; $|=1;
        pipe(my $read,my $write) or die $!;
        my $pid=fork(); defined($pid) or die $!;
        if (!$pid) { close($read); open(STDOUT,'>&',$write) or die $!; close($write);
            exec('/usr/bin/perl','-e','print STDOUT "own compiler pipe";'); die $!; }
        close($write); local $/; my $bytes=<$read>; close($read);
        waitpid($pid,0); die 'pipe child' if $?;
        $bytes eq 'own compiler pipe' or die 'pipe bytes';
        print STDOUT "pipe=$bytes\n";
    "#,
    );
    let result = CompilerSandbox::new(config)
        .unwrap()
        .run(&AtomicBool::new(false))
        .unwrap();
    assert_eq!(result.state, ExecutionState::Succeeded);
    assert_eq!(
        fs::read_to_string(result.stdout_log.unwrap()).unwrap(),
        "own stdout\npipe=own compiler pipe\n"
    );
    assert_eq!(
        fs::read_to_string(result.stderr_log.unwrap()).unwrap(),
        "own stderr\n"
    );
}

#[test]
fn phase4_exec_failure_and_pre_exec_error_reach_parent_without_hanging() {
    use std::{os::unix::process::CommandExt, process::Stdio};
    let f = Fixture::new();
    let descriptors = OwnDescriptors::new(&f, false, true, false);
    let invalid = f.outside.join("own-invalid-executable");
    fs::write(&invalid, b"own invalid executable format\n").unwrap();
    fs::set_permissions(&invalid, fs::Permissions::from_mode(0o700)).unwrap();
    for (path, expected, pre_error) in [
        (f.outside.join("missing"), 2, false),
        (invalid, 8, false),
        (PathBuf::from("/usr/bin/perl"), 13, true),
    ] {
        let mut command = Command::new(&path);
        command
            .env_clear()
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .process_group(0);
        if pre_error {
            unsafe {
                command.pre_exec(|| Err(std::io::Error::from_raw_os_error(13)));
            }
        }
        super::super::child_exec::install(&mut command).unwrap();
        let start = std::time::Instant::now();
        let result = command.spawn();
        println!(
            "P18_PHASE4_SPAWN_FAILURE path={path:?} errno={:?} elapsed={:?}",
            result.as_ref().err().and_then(|e| e.raw_os_error()),
            start.elapsed()
        );
        assert!(start.elapsed() < Duration::from_secs(2));
        assert_eq!(result.unwrap_err().raw_os_error(), Some(expected));
    }
    descriptors.assert_parent_unchanged(&f);
}

#[test]
fn phase4_unreaped_tree_drop_terminates_own_ordinary_descendant() {
    use std::os::unix::process::CommandExt;
    let f = Fixture::new();
    let pulse = f.roots.output.join("own-drop.pulse");
    let mut command = Command::new("/usr/bin/perl");
    command.env_clear().args(["-e",r#"my $pid=fork(); defined($pid) or die $!; if (!$pid) { open(my $f,'>>',$ARGV[0]) or die $!; $f->autoflush(1); while(1) { print $f 'x'; select(undef,undef,undef,0.01); } } sleep(10);"#,"--"]).arg(&pulse).process_group(0);
    super::super::child_exec::install(&mut command).unwrap();
    let child = command.spawn().unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    while !fs::metadata(&pulse).is_ok_and(|m| m.len() > 0) {
        if std::time::Instant::now() >= deadline {
            super::super::macos::drop_owned_child_for_test(child);
            panic!("own descendant did not start");
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    super::super::macos::drop_owned_child_for_test(child);
    let size = fs::metadata(&pulse).unwrap().len();
    std::thread::sleep(Duration::from_millis(80));
    assert_eq!(fs::metadata(pulse).unwrap().len(), size);
}
