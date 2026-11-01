//! Признаки коробки, которые каскад наследования читает у родителя: устанавливает ли
//! коробка содержащий блок внепоточных потомков (CSS 2.1 §10.1 п.4 плюс барьеры
//! css-transforms, css-contain, css-will-change), корень подложки (filter-effects-2
//! §BackdropRoot) и явное наследование режима `unicode-bidi` (css-writing-modes-4 §2.2).

use crate::style::computed::Computed;

/// Наследование: в CSS вниз идут только текстовые свойства. Бокс-свойства
/// (отступы, фон) принадлежат самому элементу и вниз не передаются.
/// Устанавливает ли коробка содержащий блок для внепоточных потомков
/// (§10.1 п.4 плюс барьеры css-transforms и css-contain).
///
/// Список ШИРЕ, чем `position` не `static`: лишний барьер значит невынесенный
/// элемент, то есть прежнее поведение, а пропущенный — вынос из коробки,
/// которая обязана была его удержать.
pub fn establishes_cb(c: &Computed) -> bool {
    // Растворённый элемент коробки не даёт и содержащим блоком быть не может
    // (css-display-3 §3.2).
    if matches!(
        c.reset_box_layout.display,
        Some(crate::style::computed::Display::Contents)
            | Some(crate::style::computed::Display::None)
    ) {
        return false;
    }
    matches!(
        c.reset_positioned.position,
        Some(crate::style::computed::Position::Relative)
            | Some(crate::style::computed::Position::Absolute)
            | Some(crate::style::computed::Position::Fixed)
            | Some(crate::style::computed::Position::Sticky)
    ) || c.reset_transforms.transform.is_some()
        // css-transforms-2: `preserve-3d` is a containing block for all descendants.
        || c.reset_transforms.preserve_3d == Some(true)
        || c.reset_effects.filter.is_some()
        || c.reset_context.contain_paint == Some(true)
        || c.reset_context.contain_layout == Some(true)
        // css-will-change-1 §2.1: обещание свойства, которое дало бы блок,
        // даёт его уже сейчас (`will-change-abspos-cb-002/003`).
        || c.reset_effects.will_change & (crate::style::computed::wc::CB_ABS | crate::style::computed::wc::CB_FIXED) != 0
}

/// Корень подложки (filter-effects-2 §BackdropRoot) — без корня документа:
/// его «Backdrop Root Image» и есть весь кадр. Фильтр у потомков
/// наследуется (`inherit`), но они и так под корнем.
pub fn backdrop_root(c: &Computed) -> bool {
    c.reset_effects.opacity.is_some_and(|o| o < 1.0)
        || c.reset_effects.filter.is_some_and(|f| f != crate::style::computed::Filter::neutral())
        || c.reset_effects.filter_ref.is_some()
        || c.reset_mask.mask_image.is_some()
        || c.reset_mask.clip_ref.is_some()
        || c.reset_mask.clip_polygon.is_some()
        || c.reset_mask.clip_shape.is_some()
        || c.reset_effects.blend.is_some_and(|b| b != 0)
        || c.reset_effects.backdrop_blur.is_some()
        || c.reset_effects.backdrop_color.is_some()
        // Любой `backdrop-filter`, кроме `none` (Overview.bs:119; Blink
        // paint_property_tree_builder.cc:1846), и `view-transition-name`
        // (css-view-transitions-1 Overview.bs:582).
        || c.reset_effects.backdrop_filter_set
        || c.reset_effects.vt_name
        // `clip-path` ЛЮБОЙ формой (Overview.bs:118; Blink
        // paint_property_tree_builder.cc:1887 `ClipPathClip()`): `inset()`,
        // `rect()`, `xywh()` и голая коробка разбором лежат не в
        // `clip_shape`/`clip_polygon` (`backdrop-filter-backdrop-root-clip-path-2`).
        || c.reset_mask.clip_inset.is_some()
        || c.reset_mask.clip_edges.is_some()
        || c.reset_mask.clip_xywh.is_some()
        || c.reset_mask.clip_bare_box
        || c.reset_effects.will_change_root
}

/// CSS Writing Modes 4 section 2.2: only explicit inheritance copies the mode.
pub fn resolve_bidi_inheritance(parent: &Computed, child: &mut Computed) {
    if child.reset_cascade.bidi_inherit {
        child.reset_cascade.bidi_override = parent.reset_cascade.bidi_override;
        child.reset_cascade.bidi_isolate = parent.reset_cascade.bidi_isolate;
        child.reset_cascade.bidi_embed = parent.reset_cascade.bidi_embed;
        child.reset_cascade.bidi_plaintext = parent.reset_cascade.bidi_plaintext;
    }
}
