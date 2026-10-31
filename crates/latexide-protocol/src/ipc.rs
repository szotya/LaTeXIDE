use crate::{Envelope, MAX_MESSAGE_BYTES, PROTOCOL_VERSION, Reply, Request, SessionDescriptor};
use interprocess::local_socket::{prelude::*, Name, Stream};
#[cfg(unix)]
use interprocess::local_socket::GenericFilePath;
#[cfg(windows)]
use interprocess::local_socket::GenericNamespaced;
use std::{fs, io::{self, Read, Write}, path::Path, time::{Duration, Instant}};

pub fn endpoint_name(endpoint: &str) -> io::Result<Name<'_>> {
    #[cfg(unix)] { endpoint.to_fs_name::<GenericFilePath>() }
    #[cfg(windows)] { endpoint.to_ns_name::<GenericNamespaced>() }
}

pub fn read_frame(stream: &Stream) -> io::Result<Vec<u8>> {
    stream.set_recv_timeout(Some(Duration::from_millis(200)))?;
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut incoming = stream;
    let mut frame = Vec::with_capacity(1024);
    let mut chunk = [0u8; 8192];
    loop {
        if Instant::now() >= deadline { return Err(io::Error::new(io::ErrorKind::TimedOut, "IPC request deadline exceeded")); }
        match incoming.read(&mut chunk) {
            Ok(0) => return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "IPC frame ended without newline")),
            Ok(length) => {
                let end = chunk[..length].iter().position(|byte| *byte == b'\n');
                frame.extend_from_slice(&chunk[..end.unwrap_or(length)]);
                if frame.len() > MAX_MESSAGE_BYTES { return Err(io::Error::new(io::ErrorKind::InvalidData, "IPC frame exceeds 1 MiB")); }
                if end.is_some() { return Ok(frame); }
            }
            Err(error) if matches!(error.kind(), io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut | io::ErrorKind::Interrupted) => continue,
            Err(error) => return Err(error),
        }
    }
}

pub fn write_frame(stream: &Stream, frame: &[u8]) -> io::Result<()> {
    if frame.len() > MAX_MESSAGE_BYTES { return Err(io::Error::new(io::ErrorKind::InvalidInput, "IPC frame exceeds 1 MiB")); }
    // An FFI host (Swift/C) need not inherit Rust main's SIGPIPE policy.
    // Keep a disconnected local peer from terminating the whole native IDE.
    #[cfg(target_os = "macos")] {
        use std::os::fd::AsRawFd;
        let Stream::UdSocket(socket) = stream;
        let enabled: libc::c_int = 1;
        let result = unsafe { libc::setsockopt(socket.inner().as_raw_fd(), libc::SOL_SOCKET,
            libc::SO_NOSIGPIPE, &enabled as *const _ as *const libc::c_void,
            std::mem::size_of_val(&enabled) as libc::socklen_t) };
        if result != 0 { return Err(io::Error::last_os_error()); }
    }
    stream.set_send_timeout(Some(Duration::from_secs(3)))?;
    let mut outgoing = stream;
    outgoing.write_all(frame)?;
    outgoing.write_all(b"\n")
}

pub fn read_descriptor(path: &Path) -> io::Result<SessionDescriptor> {
    let metadata = fs::metadata(path)?;
    if metadata.len() > 64 * 1024 { return Err(io::Error::new(io::ErrorKind::InvalidData, "Session descriptor too large")); }
    #[cfg(unix)] {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        unsafe extern "C" { fn geteuid() -> u32; }
        if metadata.uid() != unsafe { geteuid() } || metadata.permissions().mode() & 0o077 != 0 {
            return Err(io::Error::new(io::ErrorKind::PermissionDenied, "Session descriptor must be private to the current user"));
        }
    }
    let descriptor: SessionDescriptor = serde_json::from_slice(&fs::read(path)?)?;
    if descriptor.version != PROTOCOL_VERSION { return Err(io::Error::new(io::ErrorKind::InvalidData, "Unsupported session protocol version")); }
    Ok(descriptor)
}

pub fn call(descriptor: &SessionDescriptor, request: Request) -> io::Result<Reply> {
    let stream = Stream::connect(endpoint_name(&descriptor.endpoint)?)?;
    let envelope = Envelope { version: PROTOCOL_VERSION, token: descriptor.token.clone(), request };
    write_frame(&stream, &serde_json::to_vec(&envelope)?)?;
    Ok(serde_json::from_slice(&read_frame(&stream)?)?)
}
