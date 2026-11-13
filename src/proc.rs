//! Gestione dei processi figli: su Windows ogni download gira in un Job Object,
//! così annullare/pausa termina yt-dlp *e* i processi ffmpeg che ha avviato.

use std::process::Child;
use std::sync::Mutex;

pub struct ProcessGroup {
    #[cfg(windows)]
    job: Mutex<Option<win::Job>>,
    #[cfg(not(windows))]
    pid: Mutex<Option<u32>>,
}

impl ProcessGroup {
    pub fn new() -> Self {
        Self {
            #[cfg(windows)]
            job: Mutex::new(None),
            #[cfg(not(windows))]
            pid: Mutex::new(None),
        }
    }

    /// Associa il processo appena avviato al gruppo.
    pub fn attach(&self, child: &Child) {
        #[cfg(windows)]
        {
            if let Some(job) = win::Job::for_child(child) {
                *self.job.lock().unwrap() = Some(job);
            }
        }
        #[cfg(not(windows))]
        {
            *self.pid.lock().unwrap() = Some(child.id());
        }
    }

    /// Termina il processo e tutti i suoi discendenti.
    pub fn kill(&self) {
        #[cfg(windows)]
        if let Some(job) = self.job.lock().unwrap().as_ref() {
            job.terminate();
        }
        #[cfg(not(windows))]
        if let Some(pid) = *self.pid.lock().unwrap() {
            let _ = std::process::Command::new("kill").arg(pid.to_string()).status();
        }
    }
}

#[cfg(windows)]
mod win {
    use std::os::windows::io::AsRawHandle;
    use std::process::Child;
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{CloseHandle, HANDLE};
    use windows::Win32::System::JobObjects::*;

    pub struct Job(HANDLE);

    // L'handle di un Job Object può essere usato da qualsiasi thread.
    unsafe impl Send for Job {}
    unsafe impl Sync for Job {}

    impl Job {
        pub fn for_child(child: &Child) -> Option<Self> {
            unsafe {
                let handle = CreateJobObjectW(None, PCWSTR::null()).ok()?;
                let job = Job(handle);
                let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
                // Se l'app si chiude (anche in crash) i download vengono terminati.
                info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
                SetInformationJobObject(
                    job.0,
                    JobObjectExtendedLimitInformation,
                    &info as *const _ as *const core::ffi::c_void,
                    std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                )
                .ok()?;
                AssignProcessToJobObject(job.0, HANDLE(child.as_raw_handle())).ok()?;
                Some(job)
            }
        }

        pub fn terminate(&self) {
            unsafe {
                let _ = TerminateJobObject(self.0, 1);
            }
        }
    }

    impl Drop for Job {
        fn drop(&mut self) {
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }
}
