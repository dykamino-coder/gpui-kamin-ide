//! Определённая высота содержимого текущего блока по цепочке содержащих блоков (CSS 2 §10.5).
//! Вход при спуске `blocks()`: стиль блока, его ширина и высота области просмотра; выход —
//! thread-local `HEIGHT`, которую читает вложенная область просмотра (`iframe`) для
//! разрешения процентной высоты. `Guard` возвращает прежнее значение при выходе из блока.
//! Высота `auto` остаётся неопределённой даже при определённом `min-height`; у корня база —
//! область просмотра; вертикальное письмо и колонки цепочку обрывают.

use crate::style::computed::{Computed, Display, Position};
use crate::style::values::value::Len;

/// Definite content height of a block container with style `st` for its
/// children (`BoxConstraints::avail_h`); `outer` — its own container's, `width` —
/// its own container's available width (percentage padding/borders).
pub(crate) fn inner(
    st: &Computed,
    outer: Option<f32>,
    width: Option<f32>,
    viewport: f32,
) -> Option<f32> {
    match st.display {
        Some(Display::Contents) => return outer,
        None | Some(Display::Block | Display::ListItem | Display::InlineBlock) => {}
        _ => return None,
    }
    if st.wm().vertical() || st.column_count.is_some() || st.column_width.is_some() {
        return None;
    }
    let outer = if st.root_box { Some(viewport) } else { outer };
    if matches!(st.height, Some(Len::Pct(_)))
        && matches!(st.position, Some(Position::Absolute | Position::Fixed))
    {
        return None;
    }
    content_block_size(st, false, st.height, outer, width)
}

/// Определённый размер содержимого по блочной оси: box-sizing и пределы
/// CSS 2 §10.7 общие для содержащего блока и fragmentainer колонок.
/// `preferred` позволяет передать потолок колонок при автоматическом размере.
pub(crate) fn content_block_size(
    st: &Computed,
    vertical: bool,
    preferred: Option<Len>,
    outer: Option<f32>,
    inline: Option<f32>,
) -> Option<f32> {
    let length = |value| match value {
        Some(Len::Px(v)) => Some(v),
        Some(Len::Pct(k)) => outer.map(|v| k * v),
        _ => None,
    };
    let edges = if st.border_box == Some(true) {
        let side = |value| match value {
            None | Some(Len::Auto) => Some(0.0),
            Some(Len::Px(v)) => Some(v),
            Some(Len::Pct(k)) => inline.map(|v| k * v),
            _ => None,
        };
        let b = st.borders();
        let (a, z, ba, bz) = if vertical {
            (st.padding.left, st.padding.right, b.left, b.right)
        } else {
            (st.padding.top, st.padding.bottom, b.top, b.bottom)
        };
        side(a)? + side(z)? + side(ba)? + side(bz)?
    } else {
        0.0
    };
    // Auto height stays indefinite even when min-height is definite.
    let height = (length(preferred)? - edges).max(0.0);
    let minimum = length(if vertical {
        st.min_width
    } else {
        st.min_height
    })
    .map(|v| (v - edges).max(0.0))
    .unwrap_or(0.0);
    let maximum = length(if vertical {
        st.max_width
    } else {
        st.max_height
    })
    .map(|v| (v - edges).max(0.0))
    .unwrap_or(f32::INFINITY);
    Some(height.min(maximum).max(minimum))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn root_and_descendant_percentages_use_definite_content_height() {
        let root = Computed {
            root_box: true,
            height: Some(Len::Pct(1.0)),
            ..Computed::default()
        };
        let child = Computed {
            height: Some(Len::Pct(0.5)),
            ..Computed::default()
        };
        assert_eq!(inner(&root, None, Some(800.0), 600.0), Some(600.0));
        assert_eq!(inner(&child, Some(600.0), Some(800.0), 600.0), Some(300.0));
        assert_eq!(inner(&child, None, Some(800.0), 600.0), None);
    }

    #[test]
    fn border_box_subtracts_edges_but_min_height_does_not_make_auto_definite() {
        let mut st = Computed {
            height: Some(Len::Px(100.0)),
            border_box: Some(true),
            ..Computed::default()
        };
        st.padding.top = Some(Len::Pct(0.1));
        assert_eq!(inner(&st, None, Some(200.0), 600.0), Some(80.0));
        st.height = None;
        st.min_height = Some(Len::Px(100.0));
        assert_eq!(inner(&st, Some(600.0), Some(200.0), 600.0), None);
    }
}
