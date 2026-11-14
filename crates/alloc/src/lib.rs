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

mod stats;
pub use stats::{Memory, memory};

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

/// Простой без кадров, после которого освобождённая память отдаётся системе
/// ([`release_idle_memory`]). Дольше отложенной очистки mimalloc v3
/// (`purge_delay` 1 с): во время работы страницы по-прежнему чистит сам
/// mimalloc, а вызов на простое ловит то, что он не успел. Очистка mimalloc
/// ленивая: истёкший `purge_delay` (1000 мс, `src/options.c:140`) проверяет
/// только сборка кучи (`_mi_arenas_collect` → `mi_arenas_try_purge`,
/// `src/theap.c:135`, `src/arena.c:2162`), а поток заходит в неё раз в
/// `generic_collect` (10000, `src/options.c:164`) медленных аллокаций
/// (`src/page.c:1004–1007`) — простаивающий процесс не аллоцирует.
pub const IDLE_RELEASE_DELAY: std::time::Duration = std::time::Duration::from_millis(1500);

/// Отдать системе всю свободную память потока окна: пустые страницы и
/// свободные диапазоны арен (decommit без ожидания задержки). Для простоя
/// процесса — как `PurgeMemory` PartitionAlloc в Blink на idle; в горячем
/// цикле не звать: следующая аллокация закоммитит страницы заново.
pub fn release_idle_memory() {
    // SAFETY: `mi_collect` без условий на вызывающего; принудительная сборка
    // трогает только свободную память кучи текущего потока и арен процесса.
    unsafe { mi_collect(true) }
}

#[cfg(test)]
mod tests;
