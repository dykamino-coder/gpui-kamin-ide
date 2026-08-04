//! Job Object: ребёнок умирает вместе с шеллом (KILL_ON_JOB_CLOSE) —
//! никаких осиротевших node-хостов (порт sidecar.rs::assign_to_job 1:1).

#[cfg(windows)]
pub fn assign_to_job(child: &std::process::Child) {
    use std::os::windows::io::AsRawHandle as _;
    use std::sync::OnceLock;
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
        SetInformationJobObject,
    };

    static JOB: OnceLock<isize> = OnceLock::new();
    let job_raw = *JOB.get_or_init(|| unsafe {
        let Ok(job) = CreateJobObjectW(None, None) else {
            eprintln!("CreateJobObject failed; sidecar may orphan on hard-kill");
            return 0;
        };
        let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        let size =
            u32::try_from(std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>()).unwrap_or(0);
        if let Err(err) = SetInformationJobObject(
            job,
            JobObjectExtendedLimitInformation,
            std::ptr::from_ref(&info).cast(),
            size,
        ) {
            eprintln!("SetInformationJobObject failed: {err}");
        }
        job.0 as isize
    });
    if job_raw == 0 {
        return;
    }
    unsafe {
        if let Err(err) =
            AssignProcessToJobObject(HANDLE(job_raw as *mut _), HANDLE(child.as_raw_handle()))
        {
            eprintln!("AssignProcessToJobObject failed: {err}");
        }
    }
}

#[cfg(not(windows))]
pub fn assign_to_job(_child: &std::process::Child) {}
