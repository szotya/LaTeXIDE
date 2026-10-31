use std::{io, process::{Child, Command}};

pub fn environment(command: &mut Command, compiler: &std::path::Path, cache: &std::path::Path) -> io::Result<()> {
    command.env_clear();
    for key in ["HOME", "USERPROFILE", "SystemRoot", "WINDIR", "COMSPEC", "TEMP", "TMP", "TMPDIR",
                "LANG", "LC_ALL", "LC_CTYPE", "LOCALAPPDATA", "APPDATA", "FONTCONFIG_PATH", "FONTCONFIG_FILE"] {
        if let Some(value) = std::env::var_os(key) { command.env(key, value); }
    }
    let mut paths = vec![compiler.parent().unwrap_or_else(|| std::path::Path::new(".")).to_path_buf()];
    #[cfg(unix)] {
        paths.extend(["/usr/bin", "/bin", "/usr/sbin", "/sbin"].map(std::path::PathBuf::from));
    }
    #[cfg(windows)] {
        if let Some(root) = compiler.parent().and_then(std::path::Path::parent).and_then(std::path::Path::parent) {
            let perl = root.join("tlpkg/tlperl/bin");
            if perl.is_dir() { paths.push(perl); }
        }
        if let Some(root) = std::env::var_os("SystemRoot") { paths.push(std::path::PathBuf::from(root).join("System32")); }
    }
    command.env("PATH", std::env::join_paths(paths).map_err(io::Error::other)?);
    for (variable, directory) in [("TEXMFHOME", "texmf-home"), ("TEXMFVAR", "texmf-var"), ("TEXMFCONFIG", "texmf-config")] {
        let path = cache.join(directory);
        std::fs::create_dir_all(&path)?;
        command.env(variable, path);
    }
    command.env("MKTEXFMT", "0");
    Ok(())
}

#[cfg(unix)]
pub fn prepare(command: &mut Command) {
    use std::os::unix::process::CommandExt;
    command.process_group(0);
}
#[cfg(windows)]
pub fn prepare(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
}

pub struct Tree {
    pub child: Child,
    #[cfg(unix)] group: i32,
    #[cfg(windows)] job: windows_sys::Win32::Foundation::HANDLE,
}
impl Tree {
    pub fn attach(child: Child) -> io::Result<Self> {
        #[cfg(unix)] { Ok(Self { group: child.id() as i32, child }) }
        #[cfg(windows)] {
            let mut child = child;
            use std::os::windows::io::AsRawHandle;
            use windows_sys::Win32::{Foundation::CloseHandle, System::JobObjects::*};
            let job = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
            if job.is_null() { let error = io::Error::last_os_error(); let _ = child.kill(); let _ = child.wait(); return Err(error); }
            let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { std::mem::zeroed() };
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            let configured = unsafe { SetInformationJobObject(job, JobObjectExtendedLimitInformation,
                &info as *const _ as *const _, std::mem::size_of_val(&info) as u32) };
            let assigned = configured != 0 && unsafe { AssignProcessToJobObject(job, child.as_raw_handle()) } != 0;
            if !assigned { let error = io::Error::last_os_error(); let _ = child.kill(); let _ = child.wait(); unsafe { CloseHandle(job); } return Err(error); }
            Ok(Self { job, child })
        }
    }

    pub fn terminate(&mut self) {
        #[cfg(unix)] {
            unsafe extern "C" { fn kill(pid: i32, signal: i32) -> i32; }
            let _ = unsafe { kill(-self.group, 9) };
        }
        #[cfg(windows)] { let _ = unsafe { windows_sys::Win32::System::JobObjects::TerminateJobObject(self.job, 1) }; }
        let _ = self.child.kill();
    }
}
impl Drop for Tree {
    fn drop(&mut self) {
        self.terminate();
        let _ = self.child.wait();
        #[cfg(windows)] { unsafe { windows_sys::Win32::Foundation::CloseHandle(self.job); } }
    }
}
