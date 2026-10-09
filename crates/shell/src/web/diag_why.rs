//! Frame-source attribution sampled at most once per second in the native log.

use super::diag_activity::Counts;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

#[derive(Default)]
struct Stats {
    frames: Counts,
    events: Counts,
    sampled: Option<Instant>,
    cpu_ms: Option<u64>,
    policy: Option<String>,
}
static STATS: LazyLock<Mutex<Stats>> = LazyLock::new(|| Mutex::new(Stats::default()));

pub(crate) fn tick(name: &'static str) {
    if let Ok(mut stats) = STATS.lock() {
        stats.events.add(name);
    }
}
pub(super) fn frame(id: &str, source: &str) {
    if let Ok(mut stats) = STATS.lock() {
        stats.frames.add(&format!("{id}/{source}"));
    }
    super::diag_views::frame(id);
}
pub(super) fn post(id: &str) {
    tick("wake/host-post");
    super::diag_views::post(id);
}

pub(super) fn report() {
    let now = Instant::now();
    let Some((interval, line, policy)) = STATS.lock().ok().and_then(|mut stats| {
        if !sample_due(stats.sampled, now) {
            return None;
        }
        let interval = stats
            .sampled
            .map(|last| now.duration_since(last).as_millis());
        stats.sampled = Some(now);
        let cpu = cpu_ms();
        let delta = cpu
            .zip(stats.cpu_ms)
            .map(|(next, last)| next.saturating_sub(last));
        stats.cpu_ms = cpu;
        let frames = stats.frames.take_top(5);
        let ticks = stats.events.take_top(32);
        let line = super::diag_activity::line(delta, &frames, &ticks);
        let next = policy();
        let policy = super::diag_activity::changed(&mut stats.policy, next);
        Some((interval, line, policy))
    }) else {
        return;
    };
    if let Some(policy) = policy {
        super::diag::emit_line(policy);
    }
    if let Some(line) = line {
        super::diag::emit_line(format!("{line} interval_ms={interval:?}"));
    }
    super::diag_views::report(now);
}

fn flag(name: &str) -> Option<bool> {
    match std::env::var(name).ok().as_deref() {
        Some("1") => Some(true),
        Some("0") => Some(false),
        _ => None,
    }
}
pub(super) fn frame_rate(id: &str) -> i32 {
    let rate = if crate::win_integration::reduce_motion() {
        60
    } else {
        120
    };
    super::diag::emit_line(format!(
        "[boot] view={id:?} windowless_frame_rate={rate} cef_software={}",
        super::sw_mode()
    ));
    rate
}
fn policy() -> String {
    let reduce = crate::win_integration::reduce_motion();
    let forced = flag("KAMIN_REDUCE_MOTION");
    let (remote, animation) = system_policy();
    format!(
        "[motion] reduce={reduce} source={} rdp={remote:?} client_area_animation={animation:?} force_reduce={forced:?} force_sw_render={:?} cef_force_sw={:?} prepaint_prof={:?}",
        if forced.is_some() { "env" } else { "rdp" },
        flag("KAMIN_FORCE_SW_RENDER"),
        flag("KAMIN_CEF_FORCE_SW"),
        flag("KAMIN_PREPAINT_PROF")
    )
}

#[cfg(windows)]
fn system_policy() -> (Option<bool>, Option<bool>) {
    use windows::Win32::UI::WindowsAndMessaging::*;
    let mut enabled = windows::core::BOOL::default();
    unsafe {
        let animation = SystemParametersInfoW(
            SPI_GETCLIENTAREAANIMATION,
            0,
            Some((&mut enabled as *mut windows::core::BOOL).cast()),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
        .ok()
        .map(|_| enabled.as_bool());
        (Some(GetSystemMetrics(SM_REMOTESESSION) != 0), animation)
    }
}
#[cfg(not(windows))]
fn system_policy() -> (Option<bool>, Option<bool>) {
    (None, None)
}

#[cfg(windows)]
fn cpu_ms() -> Option<u64> {
    use windows::Win32::Foundation::FILETIME;
    use windows::Win32::System::Threading::{GetCurrentProcess, GetProcessTimes};
    let (mut creation, mut exit, mut kernel, mut user) = (
        FILETIME::default(),
        FILETIME::default(),
        FILETIME::default(),
        FILETIME::default(),
    );
    unsafe {
        GetProcessTimes(
            GetCurrentProcess(),
            &mut creation,
            &mut exit,
            &mut kernel,
            &mut user,
        )
        .ok()?;
    }
    let ticks = |time: FILETIME| ((time.dwHighDateTime as u64) << 32) | time.dwLowDateTime as u64;
    Some((ticks(kernel) + ticks(user)) / 10_000)
}
#[cfg(not(windows))]
fn cpu_ms() -> Option<u64> {
    None
}

fn sample_due(last: Option<Instant>, now: Instant) -> bool {
    last.is_none_or(|at| now.saturating_duration_since(at) >= Duration::from_secs(1))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sampling_waits_a_full_second_even_when_the_pump_is_woken_repeatedly() {
        let at = Instant::now();
        assert!(sample_due(None, at));
        assert!(!sample_due(Some(at), at));
        assert!(!sample_due(Some(at), at + Duration::from_millis(999)));
        assert!(sample_due(Some(at), at + Duration::from_secs(1)));
        assert!(sample_due(Some(at), at + Duration::from_secs(3)));
    }
}
