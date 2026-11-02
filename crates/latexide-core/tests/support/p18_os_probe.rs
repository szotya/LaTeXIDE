//! Private synthetic OS probe. This executable is not a libtest case.
#[cfg(target_os = "macos")]
use std::{fs, path::PathBuf};

#[cfg(target_os = "macos")]
fn status<T>(result: std::io::Result<T>) -> &'static str {
    match result {
        Ok(_) => "allowed",
        Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => "denied",
        Err(error) => panic!("probe has an unexpected error kind: {:?}", error.kind()),
    }
}

#[cfg(not(target_os = "macos"))]
fn main() {
    panic!("P18 OS probe requires macOS");
}

#[cfg(target_os = "macos")]
fn main() {
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
    let project_read = status(fs::read(base.join("project/main.tex")));
    let output_write = status(fs::write(
        base.join("project/output/allowed.txt"),
        b"own output",
    ));
    let outside_read = status(fs::read(base.join("project-neighbor/sentinel.txt")));
    let outside_write = status(
        fs::OpenOptions::new()
            .append(true)
            .open(base.join("project-neighbor/sentinel.txt"))
            .and_then(|mut file| file.write_all(b" own probe write")),
    );
    let network = status(TcpStream::connect_timeout(
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
