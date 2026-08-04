//! Поля `RootView`, вынесенные группой `term`
//! (`plan/100-refactor-250.md`).

#[derive(Default)]
pub struct TerminalState {
    /// Локальные терминалы (portable-pty + alacritty grid), мульти-шелл;
    /// Focus для ввода; term_menu_open — «+» дропдаун профилей.
    pub terminals: Vec<crate::term::TermSession>,
    pub term_active: usize,
    pub term_menu_open: bool,
    /// Persist-дефолтный шелл («звёздочка» в пикере, им открываются новые).
    pub term_default_shell: Option<String>,
    /// Идёт mouse-выделение в терминале (drag от mouse-down).
    pub term_selecting: bool,
    /// Окно видимых табов терминала: индекс первого (overflow-шевроны).
    /// Горизонтальный скролл полосы табов терминала (`overflow-x: auto`).
    pub term_tab_scroll: gpui::ScrollHandle,
}
