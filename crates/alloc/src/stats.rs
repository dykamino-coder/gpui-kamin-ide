//! Память самого аллокатора: сколько mimalloc держит у системы. Вход —
//! статистика процесса `mi_stats_get`; выход — байты резерва и commit арен.
//! Разница между commit и живой кучей — то, что аллокатор держит сверх
//! запрошенного: свободные страницы до отложенной очистки и округление.
//! Инвариант: размер и версия структуры сверяет сам mimalloc; при смене
//! раскладки чтение даёт `None`, а не чужие поля (проверяет тест).

/// Размер `mi_stats_t` в словах и её версия (`mimalloc-stats.h`, mimalloc v3).
const WORDS: usize = 546;
const VERSION: u64 = 5;
/// Слова полей `current`/`peak`/`total` в порядке `MI_STAT_FIELDS`.
const RESERVED: usize = 7;
const COMMITTED_PEAK: usize = 9;
const COMMITTED: usize = 10;
const PURGED: usize = 12;
const PAGE_COMMITTED: usize = 15;

#[repr(C, align(8))]
struct Stats([i64; WORDS]);

unsafe extern "C" {
    fn mi_stats_get(stats: *mut Stats) -> bool;
}

/// Память аллокатора процесса, байты.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Memory {
    /// Адресное пространство арен.
    pub reserved: u64,
    /// Commit арен сейчас и его максимум за жизнь процесса.
    pub committed: u64,
    pub peak_committed: u64,
    /// Commit внутри страниц mimalloc (без свободных диапазонов арен).
    pub page_committed: u64,
    /// Всего отдано системе очисткой.
    pub purged: u64,
}

/// Снимок памяти аллокатора; `None`, если раскладка статистики другая.
pub fn memory() -> Option<Memory> {
    let mut stats = Stats([0; WORDS]);
    stats.0[0] = std::mem::size_of::<Stats>() as i64;
    stats.0[1] = VERSION as i64;
    // SAFETY: буфер имеет размер и версию, которые mimalloc проверяет до записи.
    if !unsafe { mi_stats_get(&raw mut stats) } {
        return None;
    }
    let word = |index: usize| u64::try_from(stats.0[index]).unwrap_or(0);
    Some(Memory {
        reserved: word(RESERVED),
        committed: word(COMMITTED),
        peak_committed: word(COMMITTED_PEAK),
        page_committed: word(PAGE_COMMITTED),
        purged: word(PURGED),
    })
}
