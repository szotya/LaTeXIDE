use crate::runtime::Runtime;
use interprocess::local_socket::{prelude::*, ListenerOptions, Stream};
use latexide_core::CoreError;
use latexide_protocol::{Envelope, PROTOCOL_VERSION, Reply, SessionDescriptor, ipc};
use std::{fs::{self, OpenOptions}, io::{self, Write}, path::PathBuf,
    sync::{Arc, Mutex, atomic::{AtomicBool, Ordering}}, thread::{self, JoinHandle}};

pub struct Session { pub descriptor_path: PathBuf, endpoint: String, stop: Arc<AtomicBool>, thread: Option<JoinHandle<()>> }

fn random_hex(length: usize) -> io::Result<String> {
    let mut bytes = vec![0; length];
    getrandom::fill(&mut bytes).map_err(|error| io::Error::other(error.to_string()))?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn runtime_directory() -> io::Result<PathBuf> {
    #[cfg(unix)] {
        use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
        unsafe extern "C" { fn geteuid() -> u32; }
        let user = unsafe { geteuid() };
        let path = PathBuf::from(format!("/private/tmp/latexide-{user}"));
        match fs::DirBuilder::new().mode(0o700).create(&path) {
            Ok(()) => (),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => (),
            Err(error) => return Err(error),
        }
        let metadata = fs::symlink_metadata(&path)?;
        if !metadata.is_dir() || metadata.uid() != user || metadata.permissions().mode() & 0o077 != 0 {
            return Err(io::Error::new(io::ErrorKind::PermissionDenied, "Unsafe IDE runtime directory"));
        }
        Ok(path)
    }
    #[cfg(windows)] {
        let root = std::env::var_os("LOCALAPPDATA").ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "LOCALAPPDATA unavailable"))?;
        let path = PathBuf::from(root).join("LatexIDE/runtime");
        fs::create_dir_all(&path)?;
        Ok(path)
    }
}

impl Session {
    pub fn start(state: Arc<Mutex<Runtime>>) -> io::Result<Self> {
        let directory = runtime_directory()?;
        let session_id = format!("{}-{}", std::process::id(), random_hex(8)?);
        #[cfg(unix)]
        let endpoint = directory.join(format!("{session_id}.sock")).to_string_lossy().into_owned();
        #[cfg(windows)]
        let endpoint = format!("latexide-{session_id}");
        let options = ListenerOptions::new().name(ipc::endpoint_name(&endpoint)?);
        #[cfg(windows)]
        let options = {
            use interprocess::os::windows::{local_socket::ListenerOptionsExt, security_descriptor::SecurityDescriptor};
            let sddl = widestring::U16CString::from_str("D:P(A;;GA;;;OW)(A;;GA;;;SY)").map_err(io::Error::other)?;
            options.security_descriptor(SecurityDescriptor::deserialize(&sddl)?)
        };
        let listener = options.create_sync()?;
        let descriptor_path = directory.join(format!("{session_id}.json"));
        let descriptor = SessionDescriptor { version: PROTOCOL_VERSION, session_id,
            endpoint: endpoint.clone(), token: random_hex(32)?,
            project_root: state.lock().map_err(|_| io::Error::other("Runtime lock poisoned"))?.workspace.root().to_string_lossy().into_owned() };
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)] { use std::os::unix::fs::OpenOptionsExt; options.mode(0o600); }
        let mut output = options.open(&descriptor_path)?;
        output.write_all(&serde_json::to_vec(&descriptor)?)?;
        output.sync_all()?;
        let stop = Arc::new(AtomicBool::new(false));
        let stopping = Arc::clone(&stop);
        let thread = thread::Builder::new().name("latexide-ipc".into()).spawn(move || {
            while !stopping.load(Ordering::Acquire) {
                let connection = match listener.accept() { Ok(connection) => connection, Err(_) => break };
                if stopping.load(Ordering::Acquire) { break; }
                let reply = match ipc::read_frame(&connection).and_then(|frame|
                    serde_json::from_slice::<Envelope>(&frame).map_err(io::Error::other)) {
                    Ok(envelope) if envelope.version != PROTOCOL_VERSION => Reply::failure(CoreError::new("protocol_mismatch", "Eltérő session-protokoll.")),
                    Ok(envelope) if envelope.token != descriptor.token => Reply::failure(CoreError::new("unauthorized", "Érvénytelen IDE-session.")),
                    Ok(envelope) => match state.lock() {
                        Ok(mut runtime) => runtime.dispatch(envelope.request, "agent"),
                        Err(_) => Reply::failure(CoreError::new("runtime_unavailable", "A dokumentummotor nem érhető el.")),
                    },
                    Err(error) => Reply::failure(CoreError::new("invalid_request", error.to_string())),
                };
                let _ = ipc::write_frame(&connection, &reply.encode());
            }
        })?;
        Ok(Self { descriptor_path, endpoint, stop, thread: Some(thread) })
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Ok(name) = ipc::endpoint_name(&self.endpoint) {
            // Connecting alone wakes accept(); writing races the stopping server's close.
            let _ = Stream::connect(name);
        }
        if let Some(thread) = self.thread.take() { let _ = thread.join(); }
        let _ = fs::remove_file(&self.descriptor_path);
    }
}
