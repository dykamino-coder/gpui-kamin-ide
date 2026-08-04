//! Числа и имена, общие для всего корневого вида: шрифт, лимиты, debounce.
//!
//! Значения перенесены как есть (`plan/100-refactor-250.md`); рядом с каждым
//! — место в оригинале, откуда оно взято.

/// Шрифт интерфейса.
pub const UI_FONT: &str = "Bricolage Grotesque";

/// Лимит файл-табов (как оригинал): 13-й выталкивает самый старый чистый.
pub(crate) const MAX_EDITOR_TABS: usize = 12;
/// Минимальная ширина слота табов (один чип)
pub(crate) const CHIP_W_RESERVE: f32 = 182.0;

/// Debounce ввода в оверлеях — те же значения, что в оригинале
/// (`QuickOpen.tsx:20`, `FindInFiles.tsx:22`, `WorkspaceSymbols.tsx:11`).
pub(crate) const QO_DEBOUNCE_MS: u64 = 80;
pub(crate) const FIF_DEBOUNCE_MS: u64 = 220;
pub(crate) const WS_DEBOUNCE_MS: u64 = 120;
/// Фильтр Logs/System — `FILTER_DEBOUNCE_MS` (`LogsPanel.tsx:17`).
pub(crate) const LOG_FILTER_DEBOUNCE_MS: u64 = 150;
/// `SUB_CLOSE_DELAY_MS` оригинала: грация закрытия каскада файлового меню.
pub const SUB_CLOSE_DELAY_MS: u64 = 250;

/// Ячейка терминала: JetBrains Mono 12px → advance ровно 0.6em = 7.2px.
// JetBrains Mono 13px: advance 0.6em = 7.8; строка xterm ≈ 17
pub const TERM_CELL_W: f32 = 7.8;
pub const TERM_CELL_H: f32 = 17.0;
