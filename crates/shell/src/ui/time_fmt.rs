//! Формат времени — порт `renderer/utils/relative-time.ts`.
//!
//! У оригинала обе формы лежат в ОДНОМ модуле и используются вместе: строка
//! показывает относительное время, а подсказкой к ней — абсолютное. У нас они
//! были продублированы в панели логов и в списке сессий, причём подсказки в
//! сессиях не было вовсе (ревью ц.35).

/// `relativeTime`: now / 5m / 3h / 46d.
pub fn relative_secs(secs_ago: u64) -> String {
    if secs_ago < 60 {
        "now".into()
    } else if secs_ago < 3_600 {
        format!("{}m", secs_ago / 60)
    } else if secs_ago < 86_400 {
        format!("{}h", secs_ago / 3_600)
    } else {
        format!("{}d", secs_ago / 86_400)
    }
}

/// `relativeTime` от unix-миллисекунд (сессии отдают `lastOpened` в мс).
pub fn relative_ms(unix_ms: f64) -> String {
    relative_secs(secs_since(unix_ms))
}

/// `relativeTime` от `SystemTime` (записи системного лога).
pub fn relative_at(at: std::time::SystemTime) -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(at)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    relative_secs(secs)
}

/// `absoluteTime` (`dateStyle: medium, timeStyle: short` без локали) —
/// «YYYY-MM-DD HH:MM» от unix-секунд.
pub fn absolute_unix(secs: u64) -> String {
    // Гражданский календарь из дней эпохи (Howard Hinnant, civil_from_days)
    let days = (secs / 86_400) as i64;
    let tod = secs % 86_400;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!(
        "{y:04}-{m:02}-{d:02} {:02}:{:02}",
        tod / 3600,
        (tod % 3600) / 60
    )
}

/// `absoluteTime` от `SystemTime`.
pub fn absolute_at(at: std::time::SystemTime) -> String {
    let secs = at
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    absolute_unix(secs)
}

/// `absoluteTime` от unix-миллисекунд.
pub fn absolute_ms(unix_ms: f64) -> String {
    absolute_unix((unix_ms.max(0.0) / 1000.0) as u64)
}

/// Сколько секунд прошло с момента unix-мс (0, если он в будущем).
fn secs_since(unix_ms: f64) -> u64 {
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as f64)
        .unwrap_or(0.0);
    ((now_ms - unix_ms).max(0.0) / 1000.0) as u64
}
