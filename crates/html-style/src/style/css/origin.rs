//! Происхождение объявления в каскаде (css-cascade-5 §6.2 «Cascade Sorting
//! Order», §6.3 «Cascade Origins»; css-anchor-position-1 §fallback-rule для
//! `@position-try`). Числа — порядок силы обычных объявлений; важные
//! объявления переворачивают порядок (`CascadeKey`).

/// Происхождение правила или иного источника объявлений.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum Origin {
    /// Таблица агента пользователя (и тема приложения поверх неё).
    UserAgent = 1,
    /// Пользовательская таблица.
    User = 2,
    /// Представленческие подсказки HTML/MathML: между пользователем и
    /// автором; для `revert` считаются авторскими (§6.4 preshint).
    PresHint = 3,
    /// Таблицы и `style=""` документа.
    #[default]
    Author = 4,
    /// Выбранный вариант `@position-try`: выше автора, ниже анимаций.
    Try = 5,
    /// Анимации CSS (значения кадров).
    Animation = 6,
    /// Переходы: сильнее всего, включая важные объявления агента.
    Transition = 16,
}

impl Origin {
    /// Происхождение, к которому откатывает `revert` из этого
    /// (css-cascade-5 §7.2): автор, подсказки, `@position-try` и анимации —
    /// к пользователю, пользователь — к агенту, агент — ни к чему.
    pub fn revert_target(self) -> Option<Origin> {
        match self {
            Origin::UserAgent | Origin::Transition => None,
            Origin::User => Some(Origin::UserAgent),
            Origin::PresHint | Origin::Author | Origin::Try | Origin::Animation => {
                Some(Origin::User)
            }
        }
    }
}
