//! Политика арен глобального аллокатора (mimalloc v3) для процесса.
//!
//! Вход: вызов [`configure`] первой строкой `main`, до первой крупной
//! аллокации. Выход: размер резервирования арены mimalloc. Инвариант:
//! переменная окружения `MIMALLOC_ARENA_RESERVE` по-прежнему побеждает —
//! политика ставится как значение по умолчанию (`mi_option_set_default`),
//! а не как явная установка.
//!
//! Почему не умолчание mimalloc (одна арена 1 GiB): при замене документа
//! свободные слайсы прошлого документа остаются закоммиченными до отложенной
//! очистки, а v3 кладёт новые малые страницы в самый нижний свободный слайс
//! своего чанка. В одной большой арене это даёт рабочий набор, зависящий от
//! размещения, а не от живых данных (замеры memory4/memory5: равные занятые
//! слайсы, разница рабочего набора до 20 МБ на p03). Арены по 64 MiB
//! (mimalloc сам удваивает размер каждые 8 арен) держат свободное рядом с
//! занятым и отдают его целыми аренами. Аналоги: суперстраницы 2 MiB
//! PartitionAlloc, чанки mozjemalloc, decay jemalloc у Servo.

use std::ffi::{c_int, c_long};
use std::sync::atomic::{AtomicUsize, Ordering};

// Символы `mi_option_*` даёт статическая библиотека из `libmimalloc-sys`.
use mimalloc as _;

/// Индекс `mi_option_arena_reserve` в `mi_option_e` (mimalloc v3,
/// `include/mimalloc.h`); тест сверяет его с именем, которое печатает сам
/// mimalloc, чтобы обновление библиотеки не сдвинуло опцию молча.
const ARENA_RESERVE: c_int = 23;

/// Резервирование одной арены, KiB (единица опции `arena_reserve`).
pub const ARENA_RESERVE_KIB: c_long = 64 * 1024;

unsafe extern "C" {
    fn mi_option_set_default(option: c_int, value: c_long);
    fn mi_option_get(option: c_int) -> c_long;
    fn mi_collect(force: bool);
}

/// Ставит политику арен процесса. Повторный вызов безвреден; уже
/// зарезервированные арены не меняются, поэтому звать до первой аллокации.
pub fn configure() {
    // SAFETY: опция в пределах `_mi_option_last` (проверяет тест);
    // mimalloc читает значение только при резервировании новой арены.
    unsafe { mi_option_set_default(ARENA_RESERVE, ARENA_RESERVE_KIB) };
}

/// Действующий размер резервирования арены, KiB.
pub fn arena_reserve_kib() -> i64 {
    // SAFETY: чтение опции; инициализирует её из окружения при первом вызове.
    i64::from(unsafe { mi_option_get(ARENA_RESERVE) })
}

/// Пауза без кадров, после которой процесс отдаёт свободные страницы ОС.
/// Больше промежутков между кадрами при вводе и анимации (там кадр каждые
/// 16 мс), меньше времени, за которое пользователь оценит память простоя.
pub const IDLE_RECLAIM_QUIET: std::time::Duration = std::time::Duration::from_millis(250);

/// Отдаёт ОС все свободные страницы процесса сейчас, не дожидаясь задержки
/// очистки mimalloc. Задержка (purge_delay, 1 с) экономит повторный commit
/// между частыми освобождениями, но очищает только тогда, когда аллокатор
/// снова освобождает страницу: процесс, затихший после быстрого кадра, держал
/// бы закоммиченными слайсы, освобождённые за последнюю секунду. Поэтому
/// вызывать в простое (платформа делает это после серии кадров), а не на
/// каждом кадре. Собирает свободные блоки страниц вызывающего потока и
/// декоммитит свободные слайсы всех арен процесса; настройки не меняет.
pub fn reclaim() {
    // SAFETY: без аргументов-указателей; mimalloc синхронизирует очистку арен.
    unsafe { mi_collect(true) };
    RETAINED.reset(committed_bytes());
}

/// Наименьший рост коммита аллокатора над уровнем после прошлой очистки,
/// при котором свободное отдаётся ОС без ожидания простоя.
pub const RETAINED_FREE_CAP: usize = 8 << 20;

/// Доля уровня после очистки, которую рост может занять (1/8): большой живой
/// документ терпит пропорционально больше свободного, как `lg_dirty_mult = 3`
/// в jemalloc (грязных страниц не больше 1/8 активных).
const RETAINED_FREE_SHIFT: u32 = 3;

/// Проверка в конце кадра: если с прошлой очистки коммит аллокатора вырос
/// больше чем на `max(RETAINED_FREE_CAP, уровень / 8)`, будит фоновый поток,
/// который отдаёт свободные слайсы арен ОС. UI-поток платит только чтение
/// счётчика.
///
/// Задержка очистки mimalloc (1 с) держит освобождённое кадром закоммиченным;
/// у быстрых кадров срок не истекает внутри серии, и следующий тяжёлый кадр
/// стартует со всей памятью прошлого поверх своей. Обычные кадры (анимация,
/// ввод) коммит не растят; тяжёлый кадр (новый документ, перестройка) будит
/// одну очистку после себя. Так PartitionAlloc ограничивает число пустых slot
/// span, которые держит закоммиченными, и декоммитит сверх предела, а не каждое
/// освобождение; декоммит идёт вне потока кадров.
pub fn bound_retained() {
    let committed = committed_bytes();
    if RETAINED.over_cap(committed, RETAINED_FREE_CAP) {
        // Пока поток чистит, следующие кадры не будят его снова.
        RETAINED.reset(committed);
        background::request();
    }
}

/// Фоновая очистка арен: один поток, запросы до его пробуждения сливаются.
mod background {
    use std::sync::{Condvar, Mutex, Once, PoisonError};

    static DUE: Mutex<bool> = Mutex::new(false);
    static WAKE: Condvar = Condvar::new();
    static START: Once = Once::new();

    pub(super) fn request() {
        let mut started = true;
        START.call_once(|| {
            started = std::thread::Builder::new()
                .name("kamin-alloc-reclaim".into())
                .spawn(serve)
                .is_ok();
        });
        if !started {
            // Нет потока — очищаем здесь, как в простое.
            super::reclaim();
            return;
        }
        *DUE.lock().unwrap_or_else(PoisonError::into_inner) = true;
        WAKE.notify_one();
    }

    fn serve() {
        loop {
            {
                let mut due = DUE.lock().unwrap_or_else(PoisonError::into_inner);
                while !*due {
                    due = WAKE.wait(due).unwrap_or_else(PoisonError::into_inner);
                }
                *due = false;
            }
            // Свободные слайсы всех арен процесса; страницы этого потока пусты.
            super::reclaim();
        }
    }
}

static RETAINED: RetainedBound = RetainedBound::new();

/// Нижний уровень коммита аллокатора с прошлой очистки.
struct RetainedBound {
    low: AtomicUsize,
}

impl RetainedBound {
    const fn new() -> Self {
        Self {
            low: AtomicUsize::new(usize::MAX),
        }
    }

    /// Запоминает уровень сразу после очистки.
    fn reset(&self, committed: usize) {
        self.low.store(committed, Ordering::Relaxed);
    }

    /// Коммит вырос над нижним уровнем больше чем на `max(cap, уровень / 8)`.
    /// Нижний уровень следует вниз за собственной отложенной очисткой
    /// аллокатора.
    fn over_cap(&self, committed: usize, cap: usize) -> bool {
        let low = self
            .low
            .fetch_min(committed, Ordering::Relaxed)
            .min(committed);
        committed.saturating_sub(low) > cap.max(low >> RETAINED_FREE_SHIFT)
    }
}

/// Счётчики mimalloc v3 (`mi_stats_t`, `include/mimalloc-stats.h`, версия 5):
/// заголовок, затем 15 счётчиков-объёмов и 21 счётчик событий в порядке
/// `MI_STAT_FIELDS`, резерв и массивы по размерам. Нужен только объём
/// `committed`; размер и версию сверяет сам mimalloc и иначе ничего не пишет.
#[repr(C)]
struct MiStats {
    size: usize,
    version: usize,
    pages: [i64; 3],
    reserved: [i64; 3],
    committed: [i64; 3],
    rest: [i64; MI_STATS_REST_WORDS],
}

/// Слова `mi_stats_t` после `committed`: 12 объёмов и 21 событие из
/// `MI_STAT_FIELDS`, резерв 4 объёма + 4 события, бины 74 + 74 + 6 объёмов.
const MI_STATS_REST_WORDS: usize = 12 * 3 + 21 + 4 * 3 + 4 + (74 + 74 + 6) * 3;
const MI_STAT_VERSION: usize = 5;

#[repr(C)]
#[derive(Clone, Copy)]
struct MiSubprocId(*mut std::ffi::c_void);

unsafe extern "C" {
    fn mi_subproc_main() -> MiSubprocId;
    fn mi_subproc_stats_get_exclusive(subproc: MiSubprocId, stats: *mut MiStats) -> bool;
}

/// Байты, которые аллокатор сейчас держит закоммиченными (живое и свободное).
fn committed_bytes() -> usize {
    let mut stats = MiStats {
        size: std::mem::size_of::<MiStats>(),
        version: MI_STAT_VERSION,
        pages: [0; 3],
        reserved: [0; 3],
        committed: [0; 3],
        rest: [0; MI_STATS_REST_WORDS],
    };
    // SAFETY: `stats` — структура размера и версии, которые mimalloc сверяет
    // перед копированием; субпроцесс main живёт всё время процесса.
    let copied = unsafe { mi_subproc_stats_get_exclusive(mi_subproc_main(), &raw mut stats) };
    if copied {
        usize::try_from(stats.committed[2]).unwrap_or(0)
    } else {
        0
    }
}

#[cfg(test)]
mod tests;
