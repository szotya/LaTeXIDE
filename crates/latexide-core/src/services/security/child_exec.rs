//! Darwin exec boundary. Build all storage/actions in the parent; replace only
//! the already-forked Command child, with an explicit standard-I/O allowlist.
use std::{
    ffi::{CString, c_char, c_void},
    io,
    os::unix::{ffi::OsStrExt, process::CommandExt},
    process::Command,
};

unsafe extern "C" {
    fn posix_spawnattr_init(attr: *mut *mut c_void) -> i32;
    fn posix_spawnattr_setflags(attr: *mut *mut c_void, flags: i16) -> i32;
    fn posix_spawnattr_destroy(attr: *mut *mut c_void) -> i32;
    fn posix_spawn_file_actions_init(actions: *mut *mut c_void) -> i32;
    fn posix_spawn_file_actions_addinherit_np(actions: *mut *mut c_void, fd: i32) -> i32;
    fn posix_spawn_file_actions_destroy(actions: *mut *mut c_void) -> i32;
    fn posix_spawn(
        pid: *mut i32,
        path: *const c_char,
        actions: *const *mut c_void,
        attributes: *const *mut c_void,
        argv: *const *mut c_char,
        env: *const *mut c_char,
    ) -> i32;
}

fn checked(error: i32) -> io::Result<()> {
    if error == 0 {
        Ok(())
    } else {
        Err(io::Error::from_raw_os_error(error))
    }
}

struct Attributes(*mut c_void);
impl Drop for Attributes {
    fn drop(&mut self) {
        unsafe {
            posix_spawnattr_destroy(&mut self.0);
        }
    }
}
struct Actions(*mut c_void);
impl Drop for Actions {
    fn drop(&mut self) {
        unsafe {
            posix_spawn_file_actions_destroy(&mut self.0);
        }
    }
}
struct Strings {
    _storage: Vec<CString>,
    pointers: Vec<*mut c_char>,
}
impl Strings {
    fn new(storage: Vec<CString>) -> Self {
        let mut pointers: Vec<_> = storage.iter().map(|s| s.as_ptr().cast_mut()).collect();
        pointers.push(std::ptr::null_mut());
        Self {
            _storage: storage,
            pointers,
        }
    }
}
struct Boundary {
    attributes: Attributes,
    actions: Actions,
    path: CString,
    arguments: Strings,
    environment: Strings,
}
// After construction these opaque objects and pointed-to CString allocations
// are immutable. The closure owns them for Command's entire lifetime. Darwin
// reads them during SETEXEC in the forked child; no shared mutation/free occurs.
unsafe impl Send for Boundary {}
unsafe impl Sync for Boundary {}

impl Boundary {
    fn exec(&self) -> io::Result<()> {
        let mut unused_pid = 0;
        let error = unsafe {
            posix_spawn(
                &mut unused_pid,
                self.path.as_ptr(),
                &self.actions.0,
                &self.attributes.0,
                self.arguments.pointers.as_ptr(),
                self.environment.pointers.as_ptr(),
            )
        };
        // SETEXEC succeeds by replacing this child (same PID/group), never by
        // returning. Propagate failure via Rust's still-open exec-error pipe.
        // Even an unexpected zero must not fall through to an unrestricted exec.
        Err(io::Error::from_raw_os_error(if error == 0 {
            5
        } else {
            error
        }))
    }
}

pub(super) fn install(command: &mut Command) -> io::Result<()> {
    let cstring = |value: &[u8]| CString::new(value).map_err(|_| io::Error::from_raw_os_error(22));
    let path = cstring(command.get_program().as_bytes())?;
    let mut arguments = vec![path.clone()];
    for argument in command.get_args() {
        arguments.push(cstring(argument.as_bytes())?);
    }
    let mut environment = Vec::new();
    // This adapter always calls env_clear and supplies a fixed environment.
    // Construct the identical explicit env before fork; never call getenv here.
    for (key, value) in command.get_envs() {
        if let Some(value) = value {
            let mut entry = key.as_bytes().to_vec();
            entry.push(b'=');
            entry.extend_from_slice(value.as_bytes());
            environment.push(cstring(&entry)?);
        }
    }
    let mut raw = std::ptr::null_mut();
    checked(unsafe { posix_spawnattr_init(&mut raw) })?;
    let mut attributes = Attributes(raw);
    // Darwin SDK sys/spawn.h: SETEXEC=0x0040, CLOEXEC_DEFAULT=0x4000.
    checked(unsafe { posix_spawnattr_setflags(&mut attributes.0, 0x0040 | 0x4000) })?;
    let mut raw = std::ptr::null_mut();
    checked(unsafe { posix_spawn_file_actions_init(&mut raw) })?;
    let mut actions = Actions(raw);
    for fd in 0..=2 {
        checked(unsafe { posix_spawn_file_actions_addinherit_np(&mut actions.0, fd) })?;
    }
    let boundary = Boundary {
        attributes,
        actions,
        path,
        arguments: Strings::new(arguments),
        environment: Strings::new(environment),
    };
    // All allocation, CString conversion and attribute/action mutation happened
    // above, in the parent. The child calls only Darwin's prepared spawn/exec
    // primitive. Rust has already installed stdio/cwd/group and the preceding
    // pre_exec has applied resource limits. No descriptor-number scan or parent
    // FD flag/offset mutation is used, including for FDs above child NOFILE=128.
    unsafe {
        command.pre_exec(move || boundary.exec());
    }
    Ok(())
}
