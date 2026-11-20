//! Компактный набор разделяемых групп computed values (CSS Cascade §4).
use super::*;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Computed {
    /// Идентичность украшающей коробки; не CSS и не часть разделяемых групп.
    pub decor_box: u64,
    /// Роль тега для анонимной строчной коробки, отдельно от CSS-операндов.
    pub block_tag: bool,
    /// Применимость CSS-свойств к коробке; факт DOM, а не computed value.
    pub plain_block_box: bool,
    pub clone_audit: CloneAudit<false>,
    pub reset_text_box: StyleGroup<ResetTextBox>,
    pub reset_cascade: StyleGroup<ResetCascade>,
    pub reset_box_layout: StyleGroup<ResetBoxLayout>,
    pub reset_flex: StyleGroup<ResetFlex>,
    pub reset_border: StyleGroup<ResetBorder>,
    pub reset_grid: StyleGroup<ResetGrid>,
    pub reset_context: StyleGroup<ResetContext>,
    pub reset_generated: StyleGroup<ResetGenerated>,
    pub reset_svg_math: StyleGroup<ResetSvgMath>,
    pub reset_positioned: StyleGroup<ResetPositioned>,
    pub reset_effects: StyleGroup<ResetEffects>,
    pub reset_background: StyleGroup<ResetBackground>,
    pub reset_mask: StyleGroup<ResetMask>,
    pub reset_shapes: StyleGroup<ResetShapes>,
    pub reset_fragmentation: StyleGroup<ResetFragmentation>,
    pub reset_transforms: StyleGroup<ResetTransforms>,
    pub inh: InheritedGroup,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct InheritedStyle {
    pub inherited_clone_audit: CloneAudit<true>,
    pub ui: StyleGroup<InheritedUi>,
    pub font: StyleGroup<InheritedFont>,
    pub text_flow: StyleGroup<InheritedTextFlow>,
    pub table_list: StyleGroup<InheritedTableList>,
    pub text_ink: StyleGroup<InheritedTextInk>,
    pub writing: StyleGroup<InheritedWriting>,
}

/// Наследуемые группы отделяются только при изменении потомка.
#[derive(Clone, Debug, Default)]
pub struct InheritedGroup(pub std::rc::Rc<InheritedStyle>);
impl InheritedGroup {
    pub fn new(value: InheritedStyle) -> Self {
        Self(std::rc::Rc::new(value))
    }
}
impl PartialEq for InheritedGroup {
    fn eq(&self, other: &Self) -> bool {
        std::rc::Rc::ptr_eq(&self.0, &other.0) || *self.0 == *other.0
    }
}
impl std::ops::Deref for InheritedGroup {
    type Target = InheritedStyle;
    fn deref(&self) -> &InheritedStyle {
        &self.0
    }
}
impl std::ops::DerefMut for InheritedGroup {
    fn deref_mut(&mut self) -> &mut InheritedStyle {
        std::rc::Rc::make_mut(&mut self.0)
    }
}

/// A group a write detached but left equal to its origin shares the origin's
/// again: the copy lives only until this check. Blink compares in every
/// `ComputedStyle` setter; here one check per style covers every write through
/// `&mut`, including a declaration of the initial value (`box-sizing: initial`).
macro_rules! share_unchanged {
    ($self:ident, $origin:ident: $($group:ident),+) => {
        $(if !$self.$group.ptr_eq(&$origin.$group) && *$self.$group == *$origin.$group {
            $self.$group = $origin.$group.clone();
        })+
    };
}

impl Computed {
    /// Each group equal to the one of `origin` shares `origin`'s; the style does
    /// not change.
    pub fn share_unchanged(&mut self, origin: &Computed) {
        share_unchanged!(self, origin: reset_text_box, reset_cascade, reset_box_layout,
            reset_flex, reset_border, reset_grid, reset_context, reset_generated,
            reset_svg_math, reset_positioned, reset_effects, reset_background, reset_mask,
            reset_shapes, reset_fragmentation, reset_transforms);
        self.inh.share_unchanged(&origin.inh);
    }
}

impl InheritedGroup {
    /// The whole group of `origin` when equal; otherwise each equal subgroup is
    /// `origin`'s (a child that sets only its font keeps its parent's text groups).
    pub fn share_unchanged(&mut self, origin: &InheritedGroup) {
        if std::rc::Rc::ptr_eq(&self.0, &origin.0) {
            return;
        }
        if *self == *origin {
            *self = origin.clone();
        } else if let Some(own) = std::rc::Rc::get_mut(&mut self.0) {
            share_unchanged!(own, origin: ui, font, text_flow, table_list, text_ink, writing);
        }
    }
}
