//! Покраска: фон, рамки, двойная рамка, слой рамки (apply_paint).

use crate::style::apply::*;
use crate::style::computed::{Computed, Display, Position};
use crate::style::values::value::Len;
use gpui::{Div, Styled, px};

mod borders;
mod shadows;
use borders::paint_border_colors;
pub(crate) use borders::{border_layer, double_border};
use shadows::paint_shadows;

pub(super) fn apply_paint(mut d: Div, c: &Computed) -> Div {
    d.style().css_border_snap = Some(true);
    // Atomic paint (CSS 2.1 Appendix E step 7.2.1.4 for inline-blocks, step 5
    // for floats; css-flexbox-1 §5.4 and css-grid-1 §9: flex and grid items
    // "paint exactly the same as inline blocks"): the box's own line content
    // is painted with it, not after later siblings' backgrounds
    // (`grid-lanes` items with overlapping negative margins).
    if c.parent_flex_grid
        || c.float.is_some_and(|f| f != 0)
        || matches!(
            c.display,
            Some(Display::InlineBlock) | Some(Display::InlineFlex) | Some(Display::InlineGrid)
        )
    {
        d.style().paint_atomic = Some(true);
    }
    // An integer `z-index` on a positioned box (or a flex/grid item, css-flexbox-1
    // §5.4) makes it a stacking context (CSS 2.1 §9.9.1): its line content stays
    // inside it — a `z-index: -1` box's text no longer paints above later flow
    // backgrounds (`line-breaking-ic-001`).
    if c.z_index.is_some()
        && (c.parent_flex_grid
            || matches!(
                c.position,
                Some(Position::Relative)
                    | Some(Position::Absolute)
                    | Some(Position::Fixed)
                    | Some(Position::Sticky)
            ))
    {
        d.style().paint_stacking = Some(true);
    }
    // Смешивание больше не живёт на заливке: раньше блендер знал четыре
    // формулы и красил только фон узла, а CSS смешивает ВСЁ поддерево целиком.
    // Теперь оно считается при сборке буфера группы (см. `render::grouped`).
    // Фон, обрезанный внутренним краем (`background-clip`), красит не сама
    // коробка, а отдельный слой внутри неё (`render::clip_layer`): коробка в
    // раскладке красится целиком, вместе с рамкой и полями.
    if c.color_clip().is_none() {
        if let Some(g) = &c.gradient {
            // Градиенту с размером/повтором/позицией нужна механика плитки —
            // его рисует слой-картинка (см. render::decorations), заливка
            // красила бы всю коробку.
            if !c.gradient_as_tile() {
                d = d.bg(fill(g));
            }
            if let Some(bg) = c.background {
                d = d.bg(gpui::Background::from(bg.to_hsla()));
            }
        } else if let Some(bg) = c.background {
            d = d.bg(gpui::Background::from(bg.to_hsla()));
        }
    }
    d = paint_border_colors(d, c);
    if c.border_dashed == Some(true) {
        d = d.border_dashed();
    }
    if c.border_dotted == Some(true) {
        d.style().border_style = Some(gpui::BorderStyle::Dotted);
    }
    if let Some(o) = c.opacity {
        d = d.opacity(o);
    }
    // `visibility: hidden` — элемент занимает своё место, но не рисуется.
    if c.hidden == Some(true) {
        d.style().visibility = Some(gpui::Visibility::Hidden);
    }
    // Элемент, не ловящий курсор, не меняет и его форму.
    if let Some(name) = c
        .cursor
        .as_ref()
        .filter(|_| c.pointer_events_none != Some(true))
    {
        // Набор GPUI совпадает с CSS почти буква в букву; неизвестное имя
        // оставляем без изменений, а не подменяем стрелкой.
        let style = match name.as_str() {
            "pointer" => Some(gpui::CursorStyle::PointingHand),
            "text" | "vertical-text" => Some(gpui::CursorStyle::IBeam),
            "crosshair" => Some(gpui::CursorStyle::Crosshair),
            "grab" => Some(gpui::CursorStyle::OpenHand),
            "grabbing" | "move" | "all-scroll" => Some(gpui::CursorStyle::ClosedHand),
            "default" | "auto" => Some(gpui::CursorStyle::Arrow),
            "not-allowed" | "no-drop" => Some(gpui::CursorStyle::OperationNotAllowed),
            "context-menu" => Some(gpui::CursorStyle::ContextualMenu),
            "copy" => Some(gpui::CursorStyle::DragCopy),
            "alias" => Some(gpui::CursorStyle::DragLink),
            "ew-resize" | "col-resize" => Some(gpui::CursorStyle::ResizeLeftRight),
            "ns-resize" | "row-resize" => Some(gpui::CursorStyle::ResizeUpDown),
            "e-resize" => Some(gpui::CursorStyle::ResizeRight),
            "w-resize" => Some(gpui::CursorStyle::ResizeLeft),
            "n-resize" => Some(gpui::CursorStyle::ResizeUp),
            "s-resize" => Some(gpui::CursorStyle::ResizeDown),
            "nwse-resize" | "nw-resize" | "se-resize" => {
                Some(gpui::CursorStyle::ResizeUpLeftDownRight)
            }
            "nesw-resize" | "ne-resize" | "sw-resize" => {
                Some(gpui::CursorStyle::ResizeUpRightDownLeft)
            }
            _ => None,
        };
        if let Some(st) = style {
            d.style().mouse_cursor = Some(st);
        }
    }
    d = paint_shadows(d, c);
    d
}
