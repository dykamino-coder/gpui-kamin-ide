//! Тождество удерживаемых групп: ключ кэша не смешивает float-значения по PartialEq.
use super::*;

impl Computed {
    pub(crate) fn shares_storage(&self, other: &Self) -> bool {
        self.block_tag == other.block_tag
            && self.plain_block_box == other.plain_block_box
            && std::rc::Rc::ptr_eq(&self.inh.0, &other.inh.0)
            && self.reset_text_box.ptr_eq(&other.reset_text_box)
            && self.reset_cascade.ptr_eq(&other.reset_cascade)
            && self.reset_box_layout.ptr_eq(&other.reset_box_layout)
            && self.reset_flex.ptr_eq(&other.reset_flex)
            && self.reset_border.ptr_eq(&other.reset_border)
            && self.reset_grid.ptr_eq(&other.reset_grid)
            && self.reset_context.ptr_eq(&other.reset_context)
            && self.reset_generated.ptr_eq(&other.reset_generated)
            && self.reset_svg_math.ptr_eq(&other.reset_svg_math)
            && self.reset_positioned.ptr_eq(&other.reset_positioned)
            && self.reset_effects.ptr_eq(&other.reset_effects)
            && self.reset_background.ptr_eq(&other.reset_background)
            && self.reset_mask.ptr_eq(&other.reset_mask)
            && self.reset_shapes.ptr_eq(&other.reset_shapes)
            && self.reset_fragmentation.ptr_eq(&other.reset_fragmentation)
            && self.reset_transforms.ptr_eq(&other.reset_transforms)
    }
}

/// Владельцы групп стиля без заголовка `Computed`: ключ и значение кэшей, которым
/// нужны только группы (копия заголовка — отдельная вычисленная копия).
pub(crate) struct Storage {
    block_tag: bool,
    plain_block_box: bool,
    inh: InheritedGroup,
    reset_text_box: StyleGroup<ResetTextBox>,
    reset_cascade: StyleGroup<ResetCascade>,
    reset_box_layout: StyleGroup<ResetBoxLayout>,
    reset_flex: StyleGroup<ResetFlex>,
    reset_border: StyleGroup<ResetBorder>,
    reset_grid: StyleGroup<ResetGrid>,
    reset_context: StyleGroup<ResetContext>,
    reset_generated: StyleGroup<ResetGenerated>,
    reset_svg_math: StyleGroup<ResetSvgMath>,
    reset_positioned: StyleGroup<ResetPositioned>,
    reset_effects: StyleGroup<ResetEffects>,
    reset_background: StyleGroup<ResetBackground>,
    reset_mask: StyleGroup<ResetMask>,
    reset_shapes: StyleGroup<ResetShapes>,
    reset_fragmentation: StyleGroup<ResetFragmentation>,
    reset_transforms: StyleGroup<ResetTransforms>,
}

macro_rules! storage {
    ($($group:ident),*) => {
        impl Storage {
            pub(crate) fn of(c: &Computed) -> Self {
                Self {
                    block_tag: c.block_tag,
                    plain_block_box: c.plain_block_box,
                    inh: c.inh.clone(),
                    $($group: c.$group.clone(),)*
                }
            }

            /// Те же владельцы групп, что у `c` (как `Computed::shares_storage`).
            pub(crate) fn matches(&self, c: &Computed) -> bool {
                self.block_tag == c.block_tag
                    && self.plain_block_box == c.plain_block_box
                    && std::rc::Rc::ptr_eq(&self.inh.0, &c.inh.0)
                    $(&& self.$group.ptr_eq(&c.$group))*
            }

            /// Хоть одну группу `c` держит только `c`: другой элемент с теми же группами
            /// не встретится, а запись входа в кэш заставила бы копировать её при правке.
            pub(crate) fn any_unique(c: &Computed) -> bool {
                std::rc::Rc::strong_count(&c.inh.0) == 1 $(|| c.$group.is_unique())*
            }

            /// Поставить эти группы в `c`; собственные поля элемента остаются.
            pub(crate) fn share_into(&self, c: &mut Computed) {
                c.inh = self.inh.clone();
                $(c.$group = self.$group.clone();)*
            }
        }
    };
}

storage!(
    reset_text_box,
    reset_cascade,
    reset_box_layout,
    reset_flex,
    reset_border,
    reset_grid,
    reset_context,
    reset_generated,
    reset_svg_math,
    reset_positioned,
    reset_effects,
    reset_background,
    reset_mask,
    reset_shapes,
    reset_fragmentation,
    reset_transforms
);
