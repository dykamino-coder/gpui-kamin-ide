//! Изменения коробок одной публикации: причина отказа отрисовки не меняет damage.
//!
//! Это результат публикации, а не очередь кадров. Один BoxId может встречаться
//! несколько раз: потребитель объединяет независимые повреждения, не теряя
//! paint при обработке layout. Снятые ключи содержат прежнее поколение арены.

use crate::dom::{BoxId, NodeKey};
use html5ever::QualName;

/// Итог пакета до записи DOM; rollback не меняет имена и связи журнала.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct SourceDelta {
    pub(crate) changes: Vec<SourceChange>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SourceChange {
    pub(crate) node: NodeKey,
    pub(crate) kind: SourceChangeKind,
    pub(crate) before_parent: Option<NodeKey>,
    pub(crate) after_parent: Option<NodeKey>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SourceChangeKind {
    Attributes(Vec<QualName>),
    CharacterData,
    Children,
    Reparent,
}

impl SourceDelta {
    pub(crate) fn changes_children(&self) -> bool {
        self.changes.iter().any(|change| {
            matches!(
                change.kind,
                SourceChangeKind::Children | SourceChangeKind::Reparent
            )
        })
    }
}

/// Изменение входов раскладки; Content учитывает внешнее ограничение intrinsic.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum LayoutDamage {
    #[default]
    None,
    Content,
    Own,
}

/// Независимые требования стадий. Структура не означает замену документа.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Damage {
    pub(crate) structure: bool,
    pub(crate) layout: LayoutDamage,
    pub(crate) paint: bool,
}

impl Damage {
    pub(crate) const PAINT: Self = Self {
        structure: false,
        layout: LayoutDamage::None,
        paint: true,
    };
    pub(crate) const CONTENT: Self = Self {
        layout: LayoutDamage::Content,
        ..Self::PAINT
    };
    pub(crate) const LAYOUT: Self = Self {
        layout: LayoutDamage::Own,
        ..Self::PAINT
    };
    pub(crate) const STRUCTURE: Self = Self {
        structure: true,
        ..Self::LAYOUT
    };
    pub(crate) const CHILDREN: Self = Self {
        structure: true,
        ..Self::CONTENT
    };
}

/// Идентификаторы не удерживают исходный DOM, адаптер или элементы GPUI.
#[derive(Debug, Default)]
pub(crate) struct BoxDelta {
    pub(crate) changed: Vec<(BoxId, Damage)>,
    pub(crate) retired: Vec<BoxId>,
}

impl BoxDelta {
    pub(crate) fn mark(&mut self, id: BoxId, damage: Damage) {
        self.changed.push((id, damage));
    }

    pub(crate) fn retire(&mut self, id: BoxId) {
        self.retired.push(id);
    }
}
