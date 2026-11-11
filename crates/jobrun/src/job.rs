//! The Job Object the runner and its command live in.

use windows::Win32::Foundation::HANDLE;
use windows::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT,
    JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
    JobObjectExtendedLimitInformation, SetInformationJobObject,
};
use windows::Win32::System::Threading::GetCurrentProcess;

/// The only handle of the job. It is not inheritable and is never closed
/// explicitly: the end of the runner process closes it.
pub struct Job(HANDLE);

impl Job {
    /// Create a job that ends its processes when its last handle closes, and
    /// put the current process into it. Every process started from here on
    /// joins the job. The job nests inside the jobs the runner already
    /// belongs to (the application's and the Node host's).
    ///
    /// The job does not allow breakaway. The MSYS runtime (Git Bash) asks
    /// which limits its job has and starts every child with
    /// `CREATE_BREAKAWAY_FROM_JOB` when breakaway is allowed, so a job that
    /// allowed it would lose exactly the processes it exists to stop. The
    /// cost: a program that requests breakaway unconditionally fails to start
    /// under the runner with access denied.
    pub fn enter() -> windows::core::Result<Self> {
        // SAFETY: plain Win32 calls on a handle this function has just
        // created and on the current-process pseudo handle.
        unsafe {
            let job = Self(CreateJobObjectW(None, None)?);
            job.set_limits(JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE)?;
            AssignProcessToJobObject(job.0, GetCurrentProcess())?;
            Ok(job)
        }
    }

    /// The command has finished by itself: whatever it left running stays.
    pub fn keep_survivors(&self) {
        // A failure leaves kill-on-close in force. That only ends leftover
        // processes of a finished command, so there is nothing to report.
        let _ = self.set_limits(JOB_OBJECT_LIMIT::default());
    }

    fn set_limits(&self, flags: JOB_OBJECT_LIMIT) -> windows::core::Result<()> {
        let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        info.BasicLimitInformation.LimitFlags = flags;
        let size =
            u32::try_from(std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>()).unwrap_or(0);
        // SAFETY: `info` is a live, fully initialised structure of the class
        // named in the call, and `size` is its size.
        unsafe {
            SetInformationJobObject(
                self.0,
                JobObjectExtendedLimitInformation,
                std::ptr::from_ref(&info).cast(),
                size,
            )
        }
    }
}
