//! Клоны коробки по фрагментам: декорация клона (clone_dec) и сборка фрагмента (clone_fragment).

use super::solid_box;
use crate::dom::whitespace::is_blank;
use crate::dom::{Element, Node};
use crate::layout::box_kind::out_of_flow;
use crate::layout::fragment::table_bands::table_box;
use crate::layout::multicol::spanner::multicol_container;
use crate::style::computed::{Computed, Display};
use crate::style::values::value::Len;

/// `box-decoration-break: clone` (css-break-4 §break-decoration): блочное
/// украшение `(верх, низ)` — рамка с отбивкой, повторяемые в КАЖДОМ
/// фрагменте. Поле не входит: §break-margins «Cloned margins are always
/// truncated to zero». Длины не в точках — ноль, как в `shape_full`
/// (нестрогий `px_or`). `None` — прежний путь `slice` до последней строки.
/// Ворота — всё, что пара «украшение + поднятое тело» выразить не может:
/// строчная и монолит (свой путь), таблица (своя копия), `border-box`
/// (Blink делит заданную высоту на N·украшение — `clone-007`),
/// многоколоночник с детьми (вложенная стопка), графические эффекты (тело —
/// вложенный клон, эффект лёг бы дважды), позиционированная коробка и
/// внепоточный потомок (`clone-005.tentative`: содержащим блоком стало бы
/// поднятое тело без отбивки).
pub(crate) fn clone_dec(c: &Element) -> Option<(f32, f32)> {
    use crate::style::computed::Position;
    if !c.style.computed().bdb_clone {
        return None;
    }
    fn oof_inside(e: &Element) -> bool {
        e.children.iter().any(
            |n| matches!(n, Node::Element(k) if out_of_flow(&k.style.computed()) || oof_inside(k)),
        )
    }
    if c.inline
        || solid_box(c)
        || table_box(c)
        || c.style.computed().border_box == Some(true)
        || (multicol_container(&c.style.computed()) && c.children.iter().any(|n| !is_blank(n)))
        || c.style.computed().transform.is_some()
        || c.style.computed().filter.is_some()
        || c.style.computed().mask_image.is_some()
        || c.style.computed().opacity.is_some_and(|o| o < 1.0)
        || matches!(
            c.style.computed().position,
            Some(Position::Sticky) | Some(Position::Absolute) | Some(Position::Fixed)
        )
        || oof_inside(c)
    {
        return None;
    }
    let px_of = |l: &Option<Len>| crate::style::values::value::px_lossy_zero(*l);
    let b = c.style.computed().borders();
    let dt = px_of(&c.style.computed().padding.top) + px_of(&b.top);
    let db = px_of(&c.style.computed().padding.bottom) + px_of(&b.bottom);
    // Нулевое украшение тоже клонируется, если фрагменту есть что рисовать
    // своё: «'box-shadow' … applied to each fragment independently», «A
    // no-repeat background image will thus be rendered once in each
    // fragment» (§break-decoration). `clone-009`: у каждого квадрата свои тень
    // и контур; маска `slice` срезала бы их у обоих.
    let paints = !c.style.computed().shadows.is_empty()
        || c.style.computed().outline.is_some()
        || c.style.computed().bg_image.is_some()
        || c.style.computed().gradient.is_some();
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
    body.style = c.style.computed().paint_off().into();
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
        children: vec![Node::Element(body)],
        attrs: Vec::new(),
        inline: false,
    };
    let mut deco = c.clone();
    deco.style.used_mut().display = Some(Display::Block);
    deco.style.used_mut().height = Some(Len::Px((fh - dt - db).max(0.0)));
    deco.style.used_mut().min_height = None;
    deco.style.used_mut().max_height = None;
    deco.style.used_mut().column_count = None;
    deco.style.used_mut().column_width = None;
    deco.style.used_mut().column_height = None;
    deco.first_letter = None;
    deco.first_line = None;
    deco.children = vec![Node::Element(clip_box)];
    deco
}
