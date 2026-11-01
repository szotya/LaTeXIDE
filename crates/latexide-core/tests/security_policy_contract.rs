//! Tests the actual P18 provider through the frozen port, without lib.rs registration.
#[path = "../src/services/security/mod.rs"]
mod security_service;

use latexide_contracts::security::{ExecutionPolicy, ResourceRequest, SecurityError};
#[cfg(unix)]
use latexide_contracts::security::{ResourceGrant, SecurityPort};
use security_service::{ProjectSecurity, authorize_resource};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(1);

struct Fixture {
    base: PathBuf,
    project: PathBuf,
    #[cfg(unix)]
    outside: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let base = std::env::temp_dir().join(format!(
            "latexide-p18-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        // create_dir (not create_dir_all) refuses a pre-existing fixture or symlink.
        fs::create_dir(&base).unwrap();
        let base = base.canonicalize().unwrap();
        let project = base.join("project");
        let outside = base.join("project-neighbor");
        fs::create_dir(&project).unwrap();
        fs::create_dir(&outside).unwrap();
        fs::create_dir(project.join("output")).unwrap();
        fs::write(project.join("main.tex"), b"\xef\xbb\xbfcontent\r\n").unwrap();
        fs::write(
            outside.join("sentinel.txt"),
            b"own synthetic outside sentinel",
        )
        .unwrap();
        Self {
            base,
            project,
            #[cfg(unix)]
            outside,
        }
    }

    fn policy(&self) -> ExecutionPolicy {
        ExecutionPolicy {
            project_root: self.project.to_str().unwrap().into(),
            writable_roots: vec![self.project.join("output").to_str().unwrap().into()],
            network: false,
            trusted: true,
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.base);
    }
}

fn request(path: impl Into<String>, write: bool, network: bool) -> ResourceRequest {
    ResourceRequest {
        path: path.into(),
        write,
        network,
    }
}

#[test]
#[cfg(any(target_os = "macos", target_os = "linux"))]
fn contract_v1_first_slice() {
    // A policy decision is not OS enforcement; the opt-in subprocess probe is separate.
    let f = Fixture::new();
    let policy = f.policy();
    let service = ProjectSecurity::new(&policy).unwrap();
    let port: &dyn SecurityPort = &service;
    let outside = f.outside.join("sentinel.txt").to_str().unwrap().to_owned();
    assert_eq!(
        port.authorize_resource(&policy, request(outside.clone(), true, false)),
        Err(SecurityError::OutsideRoot)
    );
    assert_eq!(
        port.authorize_resource(&policy, request(outside, false, false)),
        Err(SecurityError::OutsideRoot)
    );
    assert_eq!(
        port.authorize_resource(&policy, request("main.tex", false, true)),
        Err(SecurityError::NetworkDenied)
    );
    assert_eq!(
        port.authorize_resource(&policy, request("main.tex", false, false)),
        Ok(ResourceGrant {
            canonical_path: f.project.join("main.tex").to_str().unwrap().into(),
        })
    );
    assert_eq!(
        fs::read(f.outside.join("sentinel.txt")).unwrap(),
        b"own synthetic outside sentinel"
    );
    assert_eq!(
        fs::read(f.project.join("main.tex")).unwrap(),
        b"\xef\xbb\xbfcontent\r\n"
    );
}

#[test]
#[cfg(any(target_os = "macos", target_os = "linux"))]
fn project_read_permission_does_not_implicitly_grant_write_permission() {
    let f = Fixture::new();
    let policy = f.policy();
    let service = ProjectSecurity::new(&policy).unwrap();
    assert_eq!(
        service.authorize_resource(&policy, request("main.tex", true, false)),
        Err(SecurityError::OutsideRoot)
    );
    let target = f.project.join("output/new.pdf");
    assert_eq!(
        service.authorize_resource(&policy, request("output/new.pdf", true, false)),
        Ok(ResourceGrant {
            canonical_path: target.to_str().unwrap().into(),
        })
    );
    assert!(
        !target.exists(),
        "policy authorization must not create an output"
    );
    assert_eq!(
        service.authorize_resource(&policy, request("output/missing/child.pdf", true, false)),
        Err(SecurityError::IdentityChanged)
    );
}

#[test]
#[cfg(any(target_os = "macos", target_os = "linux"))]
fn explicit_external_output_root_is_bound_to_the_trusted_policy() {
    let f = Fixture::new();
    let mut policy = f.policy();
    policy
        .writable_roots
        .push(f.outside.to_str().unwrap().into());
    let service = ProjectSecurity::new(&policy).unwrap();
    assert_eq!(
        service.authorize_resource(
            &policy,
            request(f.outside.join("new.pdf").to_str().unwrap(), true, false)
        ),
        Ok(ResourceGrant {
            canonical_path: f.outside.join("new.pdf").to_str().unwrap().into(),
        })
    );
    let mut escalated = policy.clone();
    escalated
        .writable_roots
        .push(f.base.to_str().unwrap().into());
    assert_eq!(
        service.authorize_resource(&escalated, request("main.tex", false, false)),
        Err(SecurityError::Untrusted)
    );
}

#[test]
#[cfg(any(target_os = "macos", target_os = "linux"))]
fn untrusted_projects_and_caller_policy_escalation_are_rejected() {
    let f = Fixture::new();
    let mut policy = f.policy();
    policy.trusted = false;
    assert!(matches!(
        ProjectSecurity::new(&policy),
        Err(SecurityError::Untrusted)
    ));
    assert_eq!(
        authorize_resource(&policy, request("main.tex", false, false)),
        Err(SecurityError::Untrusted)
    );
    policy.trusted = true;
    let service = ProjectSecurity::new(&policy).unwrap();
    let mut escalated = policy.clone();
    escalated.network = true;
    assert_eq!(
        service.authorize_resource(&escalated, request("main.tex", false, true)),
        Err(SecurityError::Untrusted)
    );
}

#[test]
#[cfg(any(target_os = "macos", target_os = "linux"))]
fn explicit_network_permission_still_requires_an_authorized_local_resource() {
    let f = Fixture::new();
    let mut policy = f.policy();
    policy.network = true;
    let service = ProjectSecurity::new(&policy).unwrap();
    assert!(
        service
            .authorize_resource(&policy, request("main.tex", false, true))
            .is_ok()
    );
    assert_eq!(
        service.authorize_resource(
            &policy,
            request(
                f.outside.join("sentinel.txt").to_str().unwrap(),
                false,
                true
            )
        ),
        Err(SecurityError::OutsideRoot)
    );
    assert_eq!(
        service.authorize_resource(&policy, request("https://example.invalid/", false, true)),
        Err(SecurityError::OutsideRoot)
    );
}

#[test]
#[cfg(any(target_os = "macos", target_os = "linux"))]
fn traversal_malformed_paths_and_missing_reads_fail_closed() {
    let f = Fixture::new();
    let policy = f.policy();
    let service = ProjectSecurity::new(&policy).unwrap();
    for path in [
        "",
        "../project-neighbor/sentinel.txt",
        "output/../../project-neighbor/sentinel.txt",
        "output/../main.tex",
        "C:\\project\\main.tex",
        "\\\\server\\share",
        "main.tex\0suffix",
        "main.tex\n",
    ] {
        assert_eq!(
            service.authorize_resource(&policy, request(path, false, false)),
            Err(SecurityError::OutsideRoot),
            "malformed/escaping request should fail closed"
        );
    }
    assert_eq!(
        service.authorize_resource(&policy, request("absent.tex", false, false)),
        Err(SecurityError::IdentityChanged)
    );
}

#[test]
#[cfg(any(target_os = "macos", target_os = "linux"))]
fn malformed_or_unavailable_policy_roots_never_fall_back_to_the_current_directory() {
    let f = Fixture::new();
    for root in [
        "",
        ".",
        "missing",
        "https://example.invalid/",
        f.project.join("main.tex").to_str().unwrap(),
    ] {
        let mut policy = f.policy();
        policy.project_root = root.into();
        assert!(ProjectSecurity::new(&policy).is_err());
    }
    let mut policy = f.policy();
    policy
        .writable_roots
        .push(f.base.join("missing").to_str().unwrap().into());
    assert!(matches!(
        ProjectSecurity::new(&policy),
        Err(SecurityError::IdentityChanged)
    ));
}

#[cfg(unix)]
#[test]
fn review_focus_r1() {
    let f = Fixture::new();
    let mut policy = f.policy();
    // Isolate the project anchor: a replaced writable root must not mask a
    // missing project-root identity check in this regression.
    policy.writable_roots.clear();
    let service = ProjectSecurity::new(&policy).unwrap();
    assert!(
        service
            .authorize_resource(&policy, request("main.tex", false, false))
            .is_ok()
    );
    fs::rename(&f.project, f.base.join("original-project")).unwrap();
    fs::create_dir(&f.project).unwrap();
    fs::create_dir(f.project.join("output")).unwrap();
    fs::write(
        f.project.join("main.tex"),
        b"replacement at the same canonical path",
    )
    .unwrap();
    assert_eq!(
        service.authorize_resource(&policy, request("main.tex", false, false)),
        Err(SecurityError::IdentityChanged)
    );
    assert_eq!(
        fs::read(f.base.join("original-project/main.tex")).unwrap(),
        b"\xef\xbb\xbfcontent\r\n"
    );
}

#[cfg(unix)]
#[test]
fn writable_root_replacement_is_detected_even_when_the_project_is_unchanged() {
    let f = Fixture::new();
    let policy = f.policy();
    let service = ProjectSecurity::new(&policy).unwrap();
    fs::rename(f.project.join("output"), f.project.join("original-output")).unwrap();
    fs::create_dir(f.project.join("output")).unwrap();
    assert_eq!(
        service.authorize_resource(&policy, request("output/new.pdf", true, false)),
        Err(SecurityError::IdentityChanged)
    );
}

#[cfg(unix)]
#[test]
fn hard_links_cannot_make_an_outside_sentinel_a_project_owned_resource() {
    let f = Fixture::new();
    let policy = f.policy();
    let service = ProjectSecurity::new(&policy).unwrap();
    fs::hard_link(
        f.outside.join("sentinel.txt"),
        f.project.join("output/alias.txt"),
    )
    .unwrap();
    assert_eq!(
        service.authorize_resource(&policy, request("output/alias.txt", true, false)),
        Err(SecurityError::IdentityChanged)
    );
    assert_eq!(
        service.authorize_resource(&policy, request("output/alias.txt", false, false)),
        Err(SecurityError::IdentityChanged)
    );
    assert_eq!(
        fs::read(f.outside.join("sentinel.txt")).unwrap(),
        b"own synthetic outside sentinel"
    );
}

#[cfg(unix)]
#[test]
fn symlinks_and_retargeted_intermediate_directories_are_rejected() {
    use std::os::unix::fs::symlink;
    let f = Fixture::new();
    let policy = f.policy();
    let service = ProjectSecurity::new(&policy).unwrap();
    symlink(
        f.project.join("main.tex"),
        f.project.join("inside-link.tex"),
    )
    .unwrap();
    symlink(
        f.outside.join("sentinel.txt"),
        f.project.join("outside-link.tex"),
    )
    .unwrap();
    symlink(f.base.join("absent"), f.project.join("dangling-link.tex")).unwrap();
    for path in ["inside-link.tex", "outside-link.tex", "dangling-link.tex"] {
        assert_eq!(
            service.authorize_resource(&policy, request(path, false, false)),
            Err(SecurityError::IdentityChanged)
        );
    }
    fs::create_dir(f.project.join("assets")).unwrap();
    fs::write(f.project.join("assets/sentinel.txt"), b"own inside content").unwrap();
    assert!(
        service
            .authorize_resource(&policy, request("assets/sentinel.txt", false, false))
            .is_ok()
    );
    fs::rename(f.project.join("assets"), f.project.join("original-assets")).unwrap();
    symlink(&f.outside, f.project.join("assets")).unwrap();
    assert_eq!(
        service.authorize_resource(&policy, request("assets/sentinel.txt", false, false)),
        Err(SecurityError::IdentityChanged)
    );
}

#[cfg(unix)]
#[test]
fn configured_root_alias_retargeting_is_not_silently_accepted() {
    use std::os::unix::fs::symlink;
    let f = Fixture::new();
    let alias = f.base.join("project-alias");
    symlink(&f.project, &alias).unwrap();
    let mut policy = f.policy();
    policy.project_root = alias.to_str().unwrap().into();
    let service = ProjectSecurity::new(&policy).unwrap();
    assert!(
        service
            .authorize_resource(&policy, request("main.tex", false, false))
            .is_ok()
    );
    fs::remove_file(&alias).unwrap();
    symlink(&f.outside, &alias).unwrap();
    assert_eq!(
        service.authorize_resource(&policy, request("sentinel.txt", false, false)),
        Err(SecurityError::IdentityChanged)
    );
}

#[cfg(unix)]
#[test]
fn retained_resource_identity_detects_same_path_same_size_same_mtime_replacement() {
    let f = Fixture::new();
    let policy = f.policy();
    let service = ProjectSecurity::new(&policy).unwrap();
    let checked = service
        .check_resource(&policy, request("main.tex", false, false))
        .unwrap();
    let old_time = fs::metadata(f.project.join("main.tex"))
        .unwrap()
        .modified()
        .unwrap();
    fs::rename(f.project.join("main.tex"), f.project.join("original.tex")).unwrap();
    fs::write(f.project.join("main.tex"), b"\xef\xbb\xbfreplace\r\n").unwrap();
    fs::File::options()
        .write(true)
        .open(f.project.join("main.tex"))
        .unwrap()
        .set_times(fs::FileTimes::new().set_modified(old_time))
        .unwrap();
    assert_eq!(
        fs::metadata(f.project.join("main.tex")).unwrap().len(),
        fs::metadata(f.project.join("original.tex")).unwrap().len()
    );
    assert_eq!(
        fs::metadata(f.project.join("main.tex"))
            .unwrap()
            .modified()
            .unwrap(),
        old_time
    );
    assert_eq!(checked.revalidate(), Err(SecurityError::IdentityChanged));
    // A fresh decision may observe a legitimate atomic rewrite; v1 has no file token.
    assert!(
        service
            .authorize_resource(&policy, request("main.tex", false, false))
            .is_ok()
    );
}

#[cfg(unix)]
#[test]
fn retained_parent_identity_detects_directory_replacement() {
    let f = Fixture::new();
    let policy = f.policy();
    fs::create_dir(f.project.join("assets")).unwrap();
    fs::write(f.project.join("assets/item.tex"), b"old").unwrap();
    let service = ProjectSecurity::new(&policy).unwrap();
    let checked = service
        .check_resource(&policy, request("assets/item.tex", false, false))
        .unwrap();
    fs::rename(f.project.join("assets"), f.project.join("original-assets")).unwrap();
    fs::create_dir(f.project.join("assets")).unwrap();
    fs::write(f.project.join("assets/item.tex"), b"new").unwrap();
    assert_eq!(checked.revalidate(), Err(SecurityError::IdentityChanged));
}

#[cfg(unix)]
#[test]
fn missing_write_leaf_must_still_be_absent_when_a_check_is_revalidated() {
    use std::os::unix::fs::symlink;
    let f = Fixture::new();
    let policy = f.policy();
    let service = ProjectSecurity::new(&policy).unwrap();
    let checked = service
        .check_resource(&policy, request("output/new.pdf", true, false))
        .unwrap();
    assert!(checked.revalidate().is_ok());
    symlink(
        f.outside.join("sentinel.txt"),
        f.project.join("output/new.pdf"),
    )
    .unwrap();
    assert_eq!(checked.revalidate(), Err(SecurityError::IdentityChanged));
    assert_eq!(
        fs::read(f.outside.join("sentinel.txt")).unwrap(),
        b"own synthetic outside sentinel"
    );
}

#[cfg(unix)]
#[test]
fn a_new_hardlink_invalidates_an_already_checked_file() {
    let f = Fixture::new();
    let policy = f.policy();
    let service = ProjectSecurity::new(&policy).unwrap();
    let checked = service
        .check_resource(&policy, request("main.tex", false, false))
        .unwrap();
    fs::hard_link(f.project.join("main.tex"), f.outside.join("late-alias.tex")).unwrap();
    assert_eq!(checked.revalidate(), Err(SecurityError::IdentityChanged));
}

#[test]
fn review_focus_r4() {
    // Narrow scope: policy errors cannot echo this synthetic input into diagnostics.
    // This does not exercise application logs, exports, or assistant context.
    let f = Fixture::new();
    let sentinel = "SYNTHETIC_P18_TOKEN_DO_NOT_ECHO_4d6e";
    let error = authorize_resource(&f.policy(), request(format!("../{sentinel}"), true, false))
        .unwrap_err();
    assert!(!format!("{error:?}").contains(sentinel));
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
#[test]
fn canonical_grants_follow_filesystem_spelling_without_inventing_case_or_unicode_rules() {
    let f = Fixture::new();
    fs::write(f.project.join("Árvíz 🧪.tex"), b"own unicode file").unwrap();
    let policy = f.policy();
    let provider = ProjectSecurity::new(&policy).unwrap();
    for name in ["Árvíz 🧪.tex", "MAIN.TEX"] {
        let path = f.project.join(name);
        match path.canonicalize() {
            Ok(canonical) => assert_eq!(
                provider.authorize_resource(&policy, request(name, false, false)),
                Ok(ResourceGrant {
                    canonical_path: canonical.to_str().unwrap().into(),
                })
            ),
            Err(_) => assert_eq!(
                provider.authorize_resource(&policy, request(name, false, false)),
                Err(SecurityError::IdentityChanged)
            ),
        }
    }
}

#[cfg(target_os = "macos")]
#[test]
fn p18_rev_01_case_variant_writable_parent_uses_os_identity() {
    use std::os::unix::fs::MetadataExt;
    let f = Fixture::new();
    let policy = f.policy();
    let service = ProjectSecurity::new(&policy).unwrap();
    let alternate = f.project.join("OUTPUT");
    let canonical = fs::metadata(f.project.join("output")).unwrap();
    let alias = fs::metadata(&alternate).unwrap();
    assert_eq!(
        (canonical.dev(), canonical.ino()),
        (alias.dev(), alias.ino()),
        "filesystem case-alias capability precondition"
    );
    assert_eq!(
        service.authorize_resource(&policy, request("OUTPUT/new.pdf", true, false)),
        Ok(ResourceGrant {
            canonical_path: f.project.join("output/new.pdf").to_str().unwrap().into()
        })
    );
    assert!(!f.project.join("output/new.pdf").exists());
}

#[cfg(target_os = "macos")]
#[test]
fn p18_rev_01_absolute_project_case_variant_uses_os_identity() {
    let f = Fixture::new();
    let policy = f.policy();
    let service = ProjectSecurity::new(&policy).unwrap();
    let alternate = f.base.join("PROJECT/output");
    assert_eq!(alternate.canonicalize().unwrap(), f.project.join("output"));
    assert_eq!(
        service.authorize_resource(
            &policy,
            request(alternate.join("new.pdf").to_str().unwrap(), true, false)
        ),
        Ok(ResourceGrant {
            canonical_path: f.project.join("output/new.pdf").to_str().unwrap().into()
        })
    );
}

#[cfg(target_os = "macos")]
#[test]
fn p18_rev_01_unicode_equivalent_writable_parent_uses_os_identity() {
    let f = Fixture::new();
    fs::create_dir(f.project.join("Árvíz")).unwrap();
    let mut policy = f.policy();
    policy
        .writable_roots
        .push(f.project.join("Árvíz").to_str().unwrap().into());
    let service = ProjectSecurity::new(&policy).unwrap();
    assert_eq!(
        f.project
            .join("A\u{301}rvi\u{301}z")
            .canonicalize()
            .unwrap(),
        f.project.join("Árvíz")
    );
    assert_eq!(
        service.authorize_resource(&policy, request("A\u{301}rvi\u{301}z/new.pdf", true, false)),
        Ok(ResourceGrant {
            canonical_path: f.project.join("Árvíz/new.pdf").to_str().unwrap().into()
        })
    );
}

#[cfg(target_os = "macos")]
#[test]
fn p18_rev_01_spelling_resolution_does_not_authorize_symlink_aliases() {
    use std::os::unix::fs::symlink;
    let f = Fixture::new();
    let policy = f.policy();
    let service = ProjectSecurity::new(&policy).unwrap();
    symlink(f.project.join("output"), f.project.join("output-alias")).unwrap();
    symlink(&f.project, f.base.join("project-alias")).unwrap();
    assert!(
        service
            .authorize_resource(&policy, request("OUTPUT-ALIAS/new.pdf", true, false))
            .is_err()
    );
    assert!(
        service
            .authorize_resource(
                &policy,
                request(
                    f.base
                        .join("project-alias/output/new.pdf")
                        .to_str()
                        .unwrap(),
                    true,
                    false
                )
            )
            .is_err()
    );
    assert!(!f.project.join("output/new.pdf").exists());
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
#[test]
fn unqualified_filesystem_adapter_fails_closed() {
    let f = Fixture::new();
    // A native path may be rejected before the unavailable identity backend is
    // reached. The portable contract is denial, not error-stage precedence.
    assert!(ProjectSecurity::new(&f.policy()).is_err());
    let mut policy = f.policy();
    policy.trusted = false;
    assert!(matches!(
        ProjectSecurity::new(&policy),
        Err(SecurityError::Untrusted)
    ));
}

// These opt-in tests are OS evidence using a synthetic Rust child. The profile
// below is a test fixture, NOT a TeX/compiler isolation implementation.
#[cfg(target_os = "macos")]
mod os_probe {
    use super::*;
    use std::{
        io,
        net::SocketAddr,
        process::{Command, Output, Stdio},
        time::{Duration, Instant},
    };

    pub fn command(
        program: &std::path::Path,
        f: &Fixture,
        receipt: &str,
        address: SocketAddr,
    ) -> Command {
        let mut command = Command::new(program);
        command
            .env_clear()
            .current_dir(&f.project)
            .env("LATEXIDE_P18_PROBE_BASE", &f.base)
            .env("LATEXIDE_P18_PROBE_RECEIPT", receipt)
            .env("LATEXIDE_P18_PROBE_ADDRESS", address.to_string());
        command
    }

    pub fn child_arguments(command: &mut Command) {
        command.args([
            "--exact",
            "p18_os_probe_child",
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ]);
    }

    pub fn run(mut command: Command) -> io::Result<Output> {
        let mut child = command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        let deadline = Instant::now() + Duration::from_secs(5);
        while child.try_wait()?.is_none() {
            if Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "own OS probe timed out",
                ));
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        child.wait_with_output()
    }

    pub fn literal(path: &std::path::Path) -> String {
        let value = path.to_str().unwrap();
        assert!(!value.chars().any(char::is_control));
        format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
    }

    pub fn status<T>(result: io::Result<T>) -> &'static str {
        match result {
            Ok(_) => "allowed",
            Err(error) if error.kind() == io::ErrorKind::PermissionDenied => "denied",
            Err(error) => panic!("probe has an unexpected error kind: {:?}", error.kind()),
        }
    }
}

#[cfg(target_os = "macos")]
#[test]
#[ignore = "private subprocess entry; run only via os_subprocess_boundary_probe"]
fn p18_os_probe_child() {
    use std::{
        io::Write,
        net::{SocketAddr, TcpStream},
        time::Duration,
    };
    let base = PathBuf::from(
        std::env::var_os("LATEXIDE_P18_PROBE_BASE").expect("parent-owned fixture required"),
    );
    assert!(
        base.file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("latexide-p18-")
    );
    let receipt = std::env::var("LATEXIDE_P18_PROBE_RECEIPT").unwrap();
    assert!(["control", "sandbox", "invalid", "missing"].contains(&receipt.as_str()));
    let address: SocketAddr = std::env::var("LATEXIDE_P18_PROBE_ADDRESS")
        .unwrap()
        .parse()
        .unwrap();
    assert!(
        address.ip().is_loopback(),
        "external network probes are forbidden"
    );
    let project_read = os_probe::status(fs::read(base.join("project/main.tex")));
    let output_write = os_probe::status(fs::write(
        base.join("project/output/allowed.txt"),
        b"own output",
    ));
    let outside_read = os_probe::status(fs::read(base.join("project-neighbor/sentinel.txt")));
    let outside_write = os_probe::status(
        fs::OpenOptions::new()
            .append(true)
            .open(base.join("project-neighbor/sentinel.txt"))
            .and_then(|mut file| file.write_all(b" own probe write")),
    );
    let network = os_probe::status(TcpStream::connect_timeout(
        &address,
        Duration::from_millis(250),
    ));
    let result = format!(
        "project_read={project_read}\noutput_write={output_write}\noutside_read={outside_read}\noutside_write={outside_write}\nnetwork={network}\n"
    );
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(base.join(format!("project/output/{receipt}.receipt")))
        .unwrap();
    file.write_all(result.as_bytes()).unwrap();
}

#[cfg(target_os = "macos")]
#[test]
#[ignore = "opt-in: own sentinels and loopback only; not compiler integration"]
fn os_subprocess_boundary_probe() {
    use std::net::TcpListener;
    let f = Fixture::new();
    let policy = f.policy();
    let provider = ProjectSecurity::new(&policy).unwrap();
    assert!(
        provider
            .authorize_resource(&policy, request("main.tex", false, false))
            .is_ok()
    );
    assert!(
        provider
            .authorize_resource(&policy, request("output/allowed.txt", true, false))
            .is_ok()
    );
    assert_eq!(
        provider.authorize_resource(&policy, request("main.tex", false, true)),
        Err(SecurityError::NetworkDenied)
    );
    let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).unwrap();
    let address = listener.local_addr().unwrap();
    let executable = std::env::current_exe().unwrap().canonicalize().unwrap();

    let mut control = os_probe::command(&executable, &f, "control", address);
    os_probe::child_arguments(&mut control);
    let control = os_probe::run(control).unwrap();
    assert!(
        control.status.success(),
        "control failed: {}",
        String::from_utf8_lossy(&control.stderr)
    );
    assert_eq!(
        fs::read_to_string(f.project.join("output/control.receipt")).unwrap(),
        "project_read=allowed\noutput_write=allowed\noutside_read=allowed\noutside_write=allowed\nnetwork=allowed\n"
    );
    fs::write(
        f.outside.join("sentinel.txt"),
        b"own synthetic outside sentinel",
    )
    .unwrap();

    // Deny-first fixture; no project config, environment or user data supplies SBPL.
    let profile = format!(
        "(version 1) (deny default) (import \"dyld-support.sb\") \
         (allow process-exec (literal {})) \
         (allow sysctl-read) \
         (allow file-read* (subpath \"/System\") (subpath \"/usr/lib\") \
             (literal \"/dev/urandom\") (literal \"/dev/null\") (literal {}) (subpath {})) \
         (allow file-write* (subpath {}))",
        os_probe::literal(&executable),
        os_probe::literal(&executable),
        os_probe::literal(&f.project),
        os_probe::literal(&f.project.join("output"))
    );
    let mut isolated = os_probe::command(
        std::path::Path::new("/usr/bin/sandbox-exec"),
        &f,
        "sandbox",
        address,
    );
    isolated.args(["-p", &profile]).arg(&executable);
    os_probe::child_arguments(&mut isolated);
    let isolated = os_probe::run(isolated).unwrap();
    assert!(
        isolated.status.success(),
        "sandbox probe failed: status={}; stdout={}; stderr={}",
        isolated.status,
        String::from_utf8_lossy(&isolated.stdout),
        String::from_utf8_lossy(&isolated.stderr)
    );
    assert_eq!(
        fs::read_to_string(f.project.join("output/sandbox.receipt")).unwrap(),
        "project_read=allowed\noutput_write=allowed\noutside_read=denied\noutside_write=denied\nnetwork=denied\n"
    );
    assert_eq!(
        fs::read(f.outside.join("sentinel.txt")).unwrap(),
        b"own synthetic outside sentinel"
    );
    assert_eq!(
        fs::read(f.project.join("main.tex")).unwrap(),
        b"\xef\xbb\xbfcontent\r\n"
    );
    println!(
        "P18 OS probe: positive control allowed all five operations; deny-first profile allowed project read/output write and denied outside read/write/loopback connect (PermissionDenied)"
    );
}

#[cfg(target_os = "macos")]
#[test]
#[ignore = "opt-in: standalone OS startup refusal; production compiler hook absent"]
fn review_focus_r2() {
    let f = Fixture::new();
    let executable = std::env::current_exe().unwrap().canonicalize().unwrap();
    // The child must not run at all, so this loopback port is never contacted.
    let address = std::net::SocketAddr::from((std::net::Ipv4Addr::LOCALHOST, 1));
    let mut invalid = os_probe::command(
        std::path::Path::new("/usr/bin/sandbox-exec"),
        &f,
        "invalid",
        address,
    );
    invalid
        .args(["-p", "(version 1) (invalid-p18-form)"])
        .arg(&executable);
    os_probe::child_arguments(&mut invalid);
    assert!(!os_probe::run(invalid).unwrap().status.success());
    assert!(!f.project.join("output/invalid.receipt").exists());
    let mut missing = os_probe::command(
        &f.base.join("missing-sandbox-utility"),
        &f,
        "missing",
        address,
    );
    missing.arg(&executable);
    os_probe::child_arguments(&mut missing);
    assert!(os_probe::run(missing).is_err());
    assert!(!f.project.join("output/missing.receipt").exists());
    assert_eq!(
        fs::read(f.outside.join("sentinel.txt")).unwrap(),
        b"own synthetic outside sentinel"
    );
}
