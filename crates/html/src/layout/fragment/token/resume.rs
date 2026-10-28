//! Состояние, общее для контекстов: продолжение абсолютно позиционированных потомков через
//! фрагментаинеры (`OofResume`). Счёт строк `line-clamp` в токен не входит: clamp-контейнер
//! раскладывается целиком своим хостом (`text::clamp::ClampFlow`), и фрагменты режут эту одну
//! раскладку, не начиная счёт заново.

use super::TokenId;

pub(crate) mod oof;

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
