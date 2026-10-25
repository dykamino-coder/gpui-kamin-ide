//! Модель break tokens: всё, что переносится из фрагментаинера N в N+1 (`Token`), и
//! фрагментаинер, в который коробка продолжается (`Space`). Ядро вне `gpui::Element` и без
//! `Window`: один контракт `resume(коробка, Space, Token)` для колонок, страниц, вложенных
//! рядов и печатного драйвера. Геометрию даёт реальная раскладка, токен — только состояние
//! продолжения; вложенные токены живут в `TokenStore` и адресуются `TokenId`.

pub(crate) mod cursor;
mod error;
pub(crate) mod fc;
pub(crate) use error::Error;
pub(crate) mod resume;
pub(crate) mod space;

use fc::FcState;
use resume::{ClampBudget, OofResume};
pub(crate) use space::Space;

/// Ссылка на токен в `TokenStore`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct TokenId(u32);

/// Как возник разрыв (Blink `BreakToken` bits; Gecko `nsReflowStatus`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct TokenFlags {
    /// Разрыв ПЕРЕД коробкой: в этом фрагментаинере от неё нет фрагмента
    /// (Blink `IsBreakBefore`, Gecko `IsInlineBreakBefore`).
    pub(crate) break_before: bool,
    /// Принудительный разрыв (css-break-3 §3.1): поля после него не усекаются.
    pub(crate) forced: bool,
    /// Коробка не рвалась, а повторяется в следующем фрагментаинере
    /// (Blink `IsRepeated`: шапка таблицы, `fixed` при печати).
    pub(crate) repeated: bool,
    /// Разрыв вызван спаннером многоколоночника (Blink `IsCausedByColumnSpanner`).
    pub(crate) by_spanner: bool,
    /// Все дети уже начаты; остались только продолжения (Blink `HasSeenAllChildren`).
    pub(crate) seen_all_children: bool,
}

/// Состояние продолжения коробки на границе фрагментаинера (Blink
/// `BlockBreakToken`; у Gecko то же лежит в continuation-фрейме и его
/// overflow lines). Отсутствие токена — коробка ещё не начата.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Token {
    /// Блочный размер коробки, уже израсходованный прежними фрагментами
    /// (Blink `ConsumedBlockSize`).
    pub(crate) consumed: f32,
    /// Номер следующего фрагмента коробки (Blink `SequenceNumber`).
    pub(crate) seq: u32,
    pub(crate) fc: FcState,
    /// Абсолютно позиционированные потомки, продолжающиеся через фрагментаинеры.
    pub(crate) oof: Vec<OofResume>,
    /// Бюджет строк `line-clamp`, общий для всех фрагментаинеров.
    pub(crate) clamp: Option<ClampBudget>,
    /// Содержимое кончилось, остались padding/border конца коробки или
    /// параллельные потоки (Blink `IsAtBlockEnd`).
    pub(crate) at_block_end: bool,
    /// Переполнение монолита за край страницы: столько следующей страницы
    /// уже занято (Blink `MonolithicOverflow`, только печать).
    pub(crate) monolithic_overflow: f32,
    pub(crate) flags: TokenFlags,
}

impl Token {
    /// Токен после первого фрагмента коробки.
    pub(crate) fn new(fc: FcState) -> Self {
        Token {
            consumed: 0.0,
            seq: 1,
            fc,
            oof: Vec::new(),
            clamp: None,
            at_block_end: false,
            monolithic_overflow: 0.0,
            flags: TokenFlags::default(),
        }
    }

    /// Токен после ещё одного фрагмента блочного размера `fragment_block`
    /// с новым состоянием контекста: прочее состояние наследуется.
    pub(crate) fn advance(&self, fragment_block: f32, fc: FcState) -> Result<Self, Error> {
        if !fragment_block.is_finite()
            || fragment_block < 0.0
            || !self.consumed.is_finite()
            || self.consumed < 0.0
        {
            return Err(Error::InvalidGeometry);
        }
        let consumed = self.consumed + fragment_block;
        if !consumed.is_finite() {
            return Err(Error::InvalidGeometry);
        }
        let seq = self.seq.checked_add(1).ok_or(Error::ResourceExhausted)?;
        let mut oof = Vec::new();
        oof.try_reserve(self.oof.len())
            .map_err(|_| Error::ResourceExhausted)?;
        oof.extend_from_slice(&self.oof);
        Ok(Token {
            consumed,
            seq,
            fc,
            oof,
            clamp: self.clamp,
            at_block_end: false,
            monolithic_overflow: 0.0,
            flags: TokenFlags::default(),
        })
    }
}

/// Хранилище токенов одного контекста фрагментации: вложенные токены
/// ссылаются сюда, а не копируют поддерево. Срок жизни arena задаёт драйвер
/// раскладки; идентичности внутри неё не переиспользуются.
#[derive(Debug, Default)]
pub(crate) struct TokenStore {
    tokens: Vec<Token>,
}

impl TokenStore {
    pub(crate) fn push(&mut self, token: Token) -> Result<TokenId, Error> {
        let id = u32::try_from(self.tokens.len()).map_err(|_| Error::ResourceExhausted)?;
        // Только уже сохранённые дети: граф продолжений не содержит циклов.
        if token
            .fc
            .child_tokens()
            .chain(token.oof.iter().filter_map(|o| o.token))
            .any(|child| self.get(child).is_none())
        {
            return Err(Error::InvalidToken);
        }
        self.tokens
            .try_reserve(1)
            .map_err(|_| Error::ResourceExhausted)?;
        self.tokens.push(token);
        Ok(TokenId(id))
    }

    pub(crate) fn get(&self, id: TokenId) -> Option<&Token> {
        self.tokens.get(id.0 as usize)
    }

    /// Токен и все его потомки в порядке обхода (родитель раньше детей).
    pub(crate) fn subtree(&self, root: TokenId) -> Vec<TokenId> {
        let mut out = vec![root];
        let mut i = 0;
        while let Some(&id) = out.get(i) {
            if let Some(t) = self.get(id) {
                out.extend(t.fc.child_tokens());
                out.extend(t.oof.iter().filter_map(|o| o.token));
            }
            i += 1;
        }
        out
    }
}

#[cfg(test)]
mod tests;
