//! Перенос ряда сетки в следующий фрагментаинер распоркой в дереве копии (css-grid-1 §12.1:
//! ряд, начатый в следующем фрагментаинере, сдвигается вместе с последующими, высота сетки
//! `auto` растёт на сдвиг; Blink `ShiftBreakpointIntoNextFragmentainer` сдвигает смещения
//! рядов, не меняя размеров элементов). Сдвиг — нижнее поле элементов, кончающихся в
//! предыдущем ряду: дорожка `auto` растёт с их вкладом, растянутые элементы сохраняют размер
//! (поле съедает прирост дорожки); явная дорожка в точках растёт сама.

use crate::dom::{Element, Node};
use crate::layout::fragment::flow::blocks::RowPush;
use crate::style::computed::{Track, TrackSize};
use crate::style::values::value::Len;

/// Сдвинуть ряд `row.track + 1` сетки `row.grid` на `grow`. Ложно, когда
/// сдвиг не выражается полями (поле не в точках) или сетки нет.
pub(super) fn push_row(dom: &mut Element, row: &RowPush, grow: f32) -> bool {
    let Some(grid) = find_mut(dom, row.grid) else {
        return false;
    };
    let items = || {
        grid.children.iter().filter_map(|n| match n {
            Node::Element(k) if row.items.contains(&k.node_id) => Some(k),
            _ => None,
        })
    };
    if items().any(|k| !matches!(k.style.computed().margin.bottom, None | Some(Len::Px(_)))) {
        return false;
    }
    for n in grid.children.iter_mut() {
        if let Node::Element(k) = n
            && row.items.contains(&k.node_id)
        {
            let old = crate::style::values::value::px_lossy_zero(k.style.computed().margin.bottom);
            k.style.used_mut().margin.bottom = Some(Len::Px(old + grow));
        }
    }
    if let Some(TrackSize::Single(Track::Px(v))) = grid
        .style
        .used_mut()
        .grid_rows
        .as_mut()
        .and_then(|r| r.get_mut(row.track))
    {
        *v += grow;
    }
    true
}

fn find_mut(c: &mut Element, id: u64) -> Option<&mut Element> {
    if c.node_id == id {
        return Some(c);
    }
    c.children.iter_mut().find_map(|n| match n {
        Node::Element(k) => find_mut(k, id),
        _ => None,
    })
}
