//! Состояние, общее для контекстов: продолжение абсолютно позиционированных потомков через
//! фрагментаинеры (`OofResume`) и бюджет строк `line-clamp` (`ClampBudget`).

use super::TokenId;

/// Абсолютно позиционированный потомок, продолжающийся в следующем
/// фрагментаинере (css-break-3 §2 «absolutely-positioned boxes … are
/// fragmented»; Blink `OofBlockStartOffset`/`OofInlineStartOffset`, Gecko
/// overflow containers `nsContainerFrame` для abspos через страницы).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct OofResume {
    /// Номер потомка в списке OOF содержащего блока.
    pub(crate) idx: u32,
    /// Остаток блочного начала: большая вставка `top`, ещё не «съеденная»
    /// фрагментаинерами, либо постоянное начало повторяемого `fixed`.
    pub(crate) block_start: f32,
    /// Строчное начало: статическую позицию посреди строки не пересчитать.
    pub(crate) inline_start: f32,
    /// `position: fixed` при печати повторяется на каждой странице
    /// (css-position-3 §3.4).
    pub(crate) repeat_fixed: bool,
    /// Токен самого потомка, если он уже начат.
    pub(crate) token: Option<TokenId>,
}

/// Бюджет строк `line-clamp` (css-overflow-4 §5 `max-lines`, `continue`):
/// счёт строк сквозной для всех фрагментаинеров и вложенных BFC.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ClampBudget {
    /// Сколько считаемых строк ещё можно показать.
    pub(crate) lines_left: u32,
    /// `continue: discard` — содержимое после точки среза выбрасывается
    /// (иначе `collapse`, `line-clamp` по умолчанию).
    pub(crate) discard: bool,
}

impl ClampBudget {
    /// Израсходовать до `lines` строк: сколько реально взято и исчерпан ли
    /// бюджет (последняя взятая строка несёт знак обрыва).
    pub(crate) fn take(&mut self, lines: u32) -> (u32, bool) {
        let taken = lines.min(self.lines_left);
        self.lines_left -= taken;
        (taken, self.lines_left == 0)
    }
}
