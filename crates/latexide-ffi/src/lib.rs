//! Native C ABI to the shared Rust workspace runtime.

extern crate self as latexide_ffi;
pub use latexide_contracts as contracts;

mod runtime;
mod session;
use latexide_core::{CoreError, Workspace};
use latexide_protocol::{MAX_MESSAGE_BYTES, Reply, Request};
use runtime::Runtime;
use session::Session;
use std::{ffi::{CStr, CString, c_char}, panic::{AssertUnwindSafe, catch_unwind},
    path::PathBuf, ptr, sync::{Arc, Mutex}};

pub struct LatexideRuntime { state: Arc<Mutex<Runtime>>, _session: Session }

fn encoded(reply: Reply) -> *mut c_char {
    CString::new(reply.encode()).expect("JSON escapes embedded NUL").into_raw()
}

/// # Safety
/// `project_root` must point to a valid NUL-terminated UTF-8 string for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn latexide_runtime_new(project_root: *const c_char) -> *mut LatexideRuntime {
    catch_unwind(AssertUnwindSafe(|| -> Result<*mut LatexideRuntime, CoreError> {
        if project_root.is_null() { return Err(CoreError::new("invalid_project", "Hiányzó projektgyökér.")); }
        let root = unsafe { CStr::from_ptr(project_root) }.to_str().map_err(|_| CoreError::new("invalid_path", "Nem UTF-8 projektútvonal."))?;
        let workspace = Workspace::open(PathBuf::from(root))?;
        let state = Arc::new(Mutex::new(Runtime::new(workspace)));
        let session = Session::start(Arc::clone(&state))?;
        state.lock().map_err(|_| CoreError::new("runtime_unavailable", "A dokumentummotor nem elérhető."))?.session_descriptor = session.descriptor_path.clone();
        Ok(Box::into_raw(Box::new(LatexideRuntime { state, _session: session })))
    })).ok().and_then(|result| match result { Ok(handle) => Some(handle), Err(error) => { eprintln!("IDE runtime: {error}"); None } }).unwrap_or(ptr::null_mut())
}

/// # Safety
/// The handle must be null or a live handle returned by `latexide_runtime_new`.
/// No calls may be in progress when it is freed. Do not free twice.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn latexide_runtime_free(runtime: *mut LatexideRuntime) {
    if !runtime.is_null() { let _ = catch_unwind(AssertUnwindSafe(|| drop(unsafe { Box::from_raw(runtime) }))); }
}

/// # Safety
/// The handle must remain live and `request_json` must be a valid NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn latexide_dispatch(runtime: *mut LatexideRuntime, request_json: *const c_char) -> *mut c_char {
    let result = catch_unwind(AssertUnwindSafe(|| {
        if runtime.is_null() || request_json.is_null() { return Reply::failure(CoreError::new("invalid_handle", "Hiányzó runtime vagy kérés.")); }
        let bytes = unsafe { CStr::from_ptr(request_json) }.to_bytes();
        if bytes.len() > MAX_MESSAGE_BYTES { return Reply::failure(CoreError::new("request_too_large", "A kérés meghaladja az 1 MiB határt.")); }
        let request: Request = match serde_json::from_slice(bytes) {
            Ok(request) => request, Err(error) => return Reply::failure(CoreError::new("invalid_request", error.to_string())),
        };
        let handle = unsafe { &*runtime };
        match handle.state.lock() {
            Ok(mut state) => state.dispatch(request, "user"),
            Err(_) => Reply::failure(CoreError::new("runtime_unavailable", "A dokumentummotor nem érhető el.")),
        }
    }));
    encoded(result.unwrap_or_else(|_| Reply::failure(CoreError::new("internal_error", "A dokumentummotor belső hibát jelzett."))))
}

/// # Safety
/// `reply` must be null or an owned pointer returned by `latexide_dispatch`, freed exactly once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn latexide_string_free(reply: *mut c_char) {
    if !reply.is_null() { drop(unsafe { CString::from_raw(reply) }); }
}
