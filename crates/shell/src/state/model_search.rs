//! Оверлеи поиска: Quick Open (Ctrl+P), Find in Files, Go to Symbol.
//!
//! Поля вынесены из `RootView` группой (`plan/100-refactor-250.md`): у трёх
//! оверлеев одинаковая механика (инпут + подписка + результаты + debounce),
//! и в общей куче полей корня она читалась как случайный набор.

use gpui::Entity;
use gpui_component::input::InputState;

#[derive(Default)]
pub struct SearchOverlays {
    /// Quick Open открыт.
    pub quickopen_open: bool,
    pub quickopen_input: Option<Entity<InputState>>,
    pub quickopen_sub: Option<gpui::Subscription>,
    pub quickopen_results: Vec<crate::ui::quick_open::FileHit>,
    /// Поколения debounce-таймеров оверлеев: каждый Change инкрементит своё,
    /// отложенный запрос уходит только если поколение не сменилось.
    pub(crate) qo_gen: u64,
    pub(crate) fif_gen: u64,
    pub(crate) ws_gen: u64,
    /// Активная строка списка в оверлеях (↑/↓; сбрасывается на новый запрос).
    pub(crate) qo_active: usize,
    pub(crate) fif_active: usize,
    pub(crate) ws_active: usize,
    /// Find in Files открыт.
    pub fif_open: bool,
    pub fif_input: Option<Entity<InputState>>,
    pub fif_sub: Option<gpui::Subscription>,
    pub fif_results: Vec<crate::ui::find_in_files::TextHit>,
    pub fif_query_len: usize,
    pub fif_busy: bool,
    /// Go to Symbol открыт.
    pub ws_open: bool,
    pub ws_input: Option<Entity<InputState>>,
    pub ws_sub: Option<gpui::Subscription>,
    pub ws_results: Vec<crate::ui::workspace_symbols::SymbolHit>,
    pub ws_query_len: usize,
}
