//! Клоны коробки по фрагментам: декорация клона (clone_dec) и сборка фрагмента (clone_fragment).

use super::solid_box;
use crate::dom::whitespace::is_blank;
use crate::dom::{Element, Node};
use crate::layout::block::sizing::BoxDimensions;
use crate::layout::box_kind::out_of_flow;
use crate::layout::fragment::table_bands::table_box;
use crate::layout::multicol::spanner::multicol_container;
use crate::style::computed::{Computed, Display};
use crate::style::values::value::Len;
mod edges;

/// `box-decoration-break: clone` (css-break-4 §break-decoration): блочное
/// украшение `(верх, низ)` — рамка с отбивкой, повторяемые в КАЖДОМ
/// фрагменте. Поле не входит: §break-margins «Cloned margins are always
/// truncated to zero». Длины не в точках — ноль, как в `shape_full`
/// (нестрогий `px_or`). `None` — прежний путь `slice` до последней строки.
/// Ворота — всё, что пара «украшение + поднятое тело» выразить не может:
/// строчная и монолит (свой путь), таблица (своя копия),
/// многоколоночник с детьми (вложенная стопка), графические эффекты (тело —
/// вложенный клон, эффект лёг бы дважды), позиционированная коробка и
/// внепоточный потомок (`clone-005.tentative`: содержащим блоком стало бы
/// поднятое тело без отбивки).
pub(crate) fn clone_dec(c: &Element) -> Option<(f32, f32)> {
    use crate::style::computed::Position;
    if !c.style.used().bdb_clone {
        return None;
    }
    fn oof_inside(e: &Element) -> bool {
        e.children
            .iter()
            .any(|n| matches!(n, Node::Element(k) if out_of_flow(&k.style.used()) || oof_inside(k)))
    }
    if c.inline
        || solid_box(c)
        || table_box(c)
        || (multicol_container(&c.style.used()) && c.children.iter().any(|n| !is_blank(n)))
        || c.style.used().transform.is_some()
        || c.style.used().filter.is_some()
        || c.style.used().mask_image.is_some()
        || c.style.used().opacity.is_some_and(|o| o < 1.0)
        || matches!(
            c.style.used().position,
            Some(Position::Sticky) | Some(Position::Absolute) | Some(Position::Fixed)
        )
        || oof_inside(c)
    {
        return None;
    }
    let px_of = |l: &Option<Len>| crate::style::values::value::px_lossy_zero(*l);
    let b = c.style.used().borders();
    let dt = px_of(&c.style.used().padding.top) + px_of(&b.top);
    let db = px_of(&c.style.used().padding.bottom) + px_of(&b.bottom);
    // Нулевое украшение тоже клонируется, если фрагменту есть что рисовать
    // своё: «'box-shadow' … applied to each fragment independently», «A
    // no-repeat background image will thus be rendered once in each
    // fragment» (§break-decoration). `clone-009`: у каждого квадрата свои тень
    // и контур; маска `slice` срезала бы их у обоих.
    let paints = !c.style.used().shadows.is_empty()
        || c.style.used().outline.is_some()
        || c.style.used().bg_image.is_some()
        || c.style.used().gradient.is_some();
    (dt + db > 0.01 || paints).then_some((dt, db))
}

/// Копия ОДНОГО фрагмента коробки с `box-decoration-break: clone`
/// (css-break-4 §break-decoration: «Each box fragment is independently
/// wrapped with the border, padding … The background is drawn independently
/// in each fragment»). Три коробки:
/// * `deco` — исходная коробка блоком высоты ЭТОГО фрагмента `fh`: рамка,
///   отбивка, фон, тень, скругление — свои у фрагмента;
/// * обёртка `overflow-y: clip` высотой `clip` — «сколько содержимого съел
///   этот фрагмент» (у последнего — до конца видимого переполнения:
///   Blink `fragmentation_utils.cc:435` «child content may overflow it»,
///   `clone-002`); по строчной оси не режет (`clone-012` кладёт содержимое
///   второй колонки в первую отрицательным полем);
/// * `body` — исходная коробка без краски и без рамки/отбивки/полей/ширины,
///   поднятая на `from` СДВИГОМ (`position: relative`), а не полем: у обёртки
///   без рамки поле тела схлопнулось бы сквозь неё и увезло обрезку
///   (taffy `block.rs:186`, `Clip` — не скролл-контейнер).
///
/// Blink устроен так же: фрагмент — свой `PhysicalBoxFragment` со всеми
/// сторонами, краска общим путём (`box_fragment_painter.cc:2322` разводит
/// только `slice`).
pub(crate) fn clone_fragment(
    c: &Element,
    dt: f32,
    db: f32,
    from: f32,
    fh: f32,
    clip: f32,
) -> Element {
    use crate::style::computed::{Overflow, Position, Sides};
    let mut body = c.clone();
    body.style = c.style.used().paint_off().into();
    // Исходный border-box уже вычтен из normal content size. После снятия
    // краёв тело сохраняет эту величину, иначе padding добавился бы дважды.
    if BoxDimensions::for_element(&body, &body.style.used()).border_box {
        let style = body.style.used_mut();
        for length in [
            &mut style.height,
            &mut style.min_height,
            &mut style.max_height,
        ] {
            if let Some(Len::Px(value)) = length {
                *value = (*value - dt - db).max(0.0);
            }
        }
    }
    body.style.used_mut().border_box = Some(false);
    body.style.used_mut().padding = Sides::default();
    body.style.used_mut().border_width = Sides::default();
    body.style.used_mut().border_visible = [Some(false); 4];
    body.style.used_mut().margin = Sides::default();
    body.style.used_mut().width = None;
    body.style.used_mut().min_width = None;
    body.style.used_mut().max_width = None;
    body.style.used_mut().position = Some(Position::Relative);
    body.style.used_mut().inset = Sides::default();
    body.style.used_mut().inset.top = Some(Len::Px(-from));
    body.style.used_mut().bdb_clone = false;
    body.hover = None;
    body.anim = None;
    body.list_item = None;
    let clip_box = Element {
        marks: Default::default(),
        list_item: None,
        node_id: c.node_id ^ 0x0BDB_C10E_0000_0001,
        anim: None,
        tag: "div".to_string(),
        style: Computed {
            display: Some(Display::Block),
            height: Some(Len::Px(clip.max(0.0))),
            overflow_y: Some(Overflow::Clip),
            ..Computed::default()
        }
        .into(),
        hover: None,
        first_letter: None,
        first_line: None,
        children: (vec![Node::Element(body)]).into(),
        attrs: Vec::new(),
        inline: false,
    };
    let mut deco = c.clone();
    let (dt, db) = edges::truncate(deco.style.used_mut(), dt, db, fh, clip);
    deco.style.used_mut().display = Some(Display::Block);
    deco.style.used_mut().height = Some(Len::Px((fh - dt - db).max(0.0)));
    // Фрагменту задан content size после вычитания его собственных краёв;
    // исходный border-box уже учтён при вычислении normal size.
    deco.style.used_mut().border_box = Some(false);
    deco.style.used_mut().min_height = None;
    deco.style.used_mut().max_height = None;
    deco.style.used_mut().column_count = None;
    deco.style.used_mut().column_width = None;
    deco.style.used_mut().column_height = None;
    deco.first_letter = None;
    deco.first_line = None;
    deco.children = vec![Node::Element(clip_box)].into();
    deco
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn border_box_clone_subtracts_edges_once_and_does_not_reapply_minimum() {
        let tree = crate::dom::parse(
            "<div style='height:100px;min-height:80px;max-height:120px;box-sizing:border-box;padding:10px;box-decoration-break:clone'></div>",
            "",
        );
        fn find(nodes: &[Node]) -> Option<&Element> {
            nodes.iter().find_map(|n| match n {
                Node::Element(e) if e.tag == "div" => Some(e),
                Node::Element(e) => find(&e.children),
                _ => None,
            })
        }
        let fragment = clone_fragment(find(&tree).unwrap(), 10.0, 10.0, 30.0, 50.0, 30.0);
        assert_eq!(fragment.style.used().height, Some(Len::Px(30.0)));
        assert_eq!(fragment.style.used().min_height, None);
        let Node::Element(clip) = &fragment.children[0] else {
            panic!("clip box")
        };
        let Node::Element(body) = &clip.children[0] else {
            panic!("body box")
        };
        let style = body.style.used();
        assert_eq!(style.height, Some(Len::Px(80.0)));
        assert_eq!(style.min_height, Some(Len::Px(60.0)));
        assert_eq!(style.max_height, Some(Len::Px(100.0)));
        assert_eq!(style.inset.top, Some(Len::Px(-30.0)));
        assert_eq!(style.border_box, Some(false));
    }
}
