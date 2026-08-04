//! Общий Job Object процесса: дети живут и умирают вместе с главным.
//!
//! Две причины. Диспетчер задач собирает процессы одного Job под строкой
//! приложения — после переименования детей CEF в `kaminide-web.exe` они
//! расползлись по «Фоновым процессам» (жалоба пользователя). И главное:
//! `KILL_ON_JOB_CLOSE` гарантирует, что при любой смерти главного процесса
//! (краш, Stop-Process) дети не осиротеют — раньше после жёсткого убийства
//! у следующего запуска падал GPU-процесс CEF.

/// Завести Job и вступить в него. Зовётся ИЗ `main` ДО инициализации CEF:
/// дети наследуют Job родителя. Хэндл намеренно утекает — Job обязан жить
/// до конца процесса, его закрытие и есть сигнал «убить всех».
#[cfg(windows)]
pub fn adopt_children() {
    use windows::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_BREAKAWAY_OK,
        JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        JobObjectExtendedLimitInformation, SetInformationJobObject,
    };
    use windows::Win32::System::Threading::GetCurrentProcess;

    unsafe {
        let Ok(job) = CreateJobObjectW(None, None) else {
            eprintln!("[job] не создался — дети переживут главный процесс");
            return;
        };
        let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        // BREAKAWAY_OK: без него спавн инсталлера самообновления с
        // CREATE_BREAKAWAY_FROM_JOB падал, fallback сажал инсталлер в наш Job,
        // и выход приложения убивал установку молча («скачал до 100% и
        // закрылся»). Дети CEF breakaway не просят — остаются в Job.
        info.BasicLimitInformation.LimitFlags =
            JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE | JOB_OBJECT_LIMIT_BREAKAWAY_OK;
        let ok = SetInformationJobObject(
            job,
            JobObjectExtendedLimitInformation,
            &info as *const _ as *const _,
            std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        );
        if ok.is_err() || AssignProcessToJobObject(job, GetCurrentProcess()).is_err() {
            eprintln!("[job] вступить не вышло — дети переживут главный процесс");
        }
        // Хэндл не закрываем: см. док-коммент.
    }
}

#[cfg(not(windows))]
pub fn adopt_children() {}
