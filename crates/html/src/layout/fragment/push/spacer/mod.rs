//! Распорки перед перенесённым и рост перенесённой коробки; поля абзаца.

use crate::document::state::ValueCx;
use crate::dom::whitespace::is_blank;
use crate::dom::{Element, Node};
use crate::layout::fragment::breaks::edge_break;
use crate::layout::fragment::grid_bands::grid_stack;
use crate::style::computed::Display;
use crate::style::values::value::Len;
mod grow;
pub(crate) use grow::avoid_only_monolith;
pub(crate) use grow::grow_pushed;

/// Распорка-КОРОБКА перед коробкой `id`: пустой блок высотой `grow`.
/// Нужна принудительному разрыву. Точка `forced` в мере стоит ПЕРЕД
/// схлопнутым полем (`shape_full`: `cuts.push((y, y + lead))`, потом
/// `forced.push(y)`), поэтому `margin-top` её не сдвигает — сдвигает только
/// новый поточный сосед.
pub(super) fn spacer_before(c: &mut Element, id: u64, grow: f32, vcx: ValueCx<'_>) -> bool {
    insert_spacer(c, id, grow, vcx, true)
}

/// Распорка по РАЗЛОЖЕННОЙ копии (`push::Regrow`): заданная высота хозяина не
/// растёт — прирост от фрагментации засчитывается в неё (css-break-3
/// §box-splitting, примечание), содержимое за ней — его переполнение.
pub(super) fn spacer_before_laid(c: &mut Element, id: u64, grow: f32, vcx: ValueCx<'_>) -> bool {
    insert_spacer(c, id, grow, vcx, false)
}

fn insert_spacer(c: &mut Element, id: u64, grow: f32, vcx: ValueCx<'_>, fixed_grows: bool) -> bool {
    // Сетка-стопка: между рядами стоит `row-gap` (`shape_full`:
    // `lead = prev_mb + row_gap + kmt`), и вставка ряда добавляет ЛИШНИЙ
    // зазор. Ряд flex без переноса: высота ряда — `tallest` по детям, а
    // распорка встала бы соседом БОК О БОК и подняла бы весь ряд. Оба
    // случая забирает подъём разрыва, а не рост.
    let is_flex = matches!(
        c.style.used().display,
        Some(Display::Flex) | Some(Display::InlineFlex)
    ) || c.style.used().webkit_box == Some(true);
    let row_nowrap = is_flex
        && matches!(
            c.style.used().flex_dir,
            None | Some(crate::style::computed::FlexDir::Row)
                | Some(crate::style::computed::FlexDir::RowReverse)
        )
        && c.style.used().flex_wrap != Some(true)
        && c.style.cascaded().webkit_box_vertical != Some(true);
    // Зазор гибкой стопки (`row-gap`) встал бы и перед распоркой — лишний
    // зазор, как у сетки-стопки; такой разрыв остаётся без роста.
    let flex_gapped = is_flex
        && c.style.used().webkit_box != Some(true)
        && matches!(c.style.used().gap, Some((Some(Len::Px(v)), _)) if v > 0.0);
    let at = if grid_stack(c) || row_nowrap || flex_gapped {
        None
    } else {
        c.children
            .iter()
            .position(|n| matches!(n, Node::Element(k) if k.node_id == id))
    };
    if let Some(i) = at {
        // Оценка формы видит распорку отдельным соседом и сохраняет break-after
        // перед ней. Измеренная стопка хранит исходных соседей и переносит
        // границу под распорку по STRUT_ID, поэтому принимает оба края.
        if fixed_grows && !matches!(&c.children[i], Node::Element(k) if edge_break(k, false, vcx)) {
            return false;
        }
        // Нижнее поле предыдущего соседа переносится на распорку. Иначе
        // `lead` схлопывается ДВАЖДЫ — перед распоркой (`prev_mb.max(0)`) и
        // перед коробкой (`0.max(kmt)`), — и точка разрыва уезжает на
        // `prev_mb` НИЖЕ края колонки (`trailing-child-margin-000`, `-002`:
        // `margin-bottom: 50px`, обе зелёные).
        let pi = c.children[..i].iter().rposition(|n| !is_blank(n));
        let prev_mb = match pi.map(|j| &c.children[j]) {
            Some(Node::Element(k)) => k.style.used().margin.bottom,
            _ => None,
        };
        if prev_mb.is_some()
            && let Some(Node::Element(k)) = pi.map(|j| &mut c.children[j])
        {
            k.style.used_mut().margin.bottom = None;
        }
        let mut style = crate::style::computed::Computed {
            height: Some(Len::Px(grow)),
            ..Default::default()
        };
        style.margin.bottom = prev_mb;
        // Распорка в гибком хозяине — сама элемент: при переносе по строкам
        // она обязана занять СВОЮ строку (иначе встаёт рядом с предыдущим
        // элементом и строку не растит), не сжиматься в контейнере с заданной
        // высотой и стоять в визуальном порядке рядом со своей коробкой
        // (`order`, `reorder` в `blocks()`).
        if matches!(
            c.style.used().display,
            Some(Display::Flex) | Some(Display::InlineFlex)
        ) {
            style.width = Some(Len::Pct(1.0));
            style.flex_shrink = Some(0.0);
            if let Node::Element(k) = &c.children[i] {
                style.order = k.style.used().order;
            }
        }
        c.children.insert(
            i,
            Node::Element(Element {
                marks: Default::default(),
                list_item: None,
                // Свой устойчивый номер: анимации у распорки нет, но номер
                // обязан быть уникальным — иначе GPUI склеит её состояние с
                // коробкой, перед которой она стоит.
                node_id: id ^ crate::layout::fragment::flow::blocks::STRUT_ID,
                anim: None,
                tag: "div".to_string(),
                style: style.into(),
                hover: None,
                first_letter: None,
                first_line: None,
                children: (Vec::new()).into(),
                attrs: Vec::new(),
                inline: false,
            }),
        );
        // Заданная высота хозяина СТАРШЕ содержимого (`shape_full`: ветка
        // `c.style.height` возвращает `v + top + bot`), и распорка внутри неё
        // меры не меняет — `changed` не взводится, цикл `grow_pushed` встаёт
        // на первом заходе. Проба `p2-single-line-column-flex-fragmentation-
        // 037` осталась красной именно поэтому, а `p3-…` с высотой 100 → 150
        // сняла 4/5 площади (3906 → 756 точек).
        if fixed_grows && let Some(Len::Px(h)) = c.style.used().height {
            c.style.used_mut().height = Some(Len::Px(h + grow));
        }
        return true;
    }
    for n in c.children.iter_mut() {
        if let Node::Element(k) = n
            && insert_spacer(k, id, grow, vcx, fixed_grows)
        {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::fragment::{
        ShapeCx,
        flow::blocks::{BlockBox, Laid, STRUT_ID},
    };

    #[test]
    fn measured_forced_break_after_moves_below_its_growth_strut() {
        let tree = crate::dom::parse(
            "<section><div style='height:20px;break-after:page'></div><div style='height:30px'></div></section>",
            "",
        );
        fn find(nodes: &[Node]) -> Option<&Element> {
            nodes.iter().find_map(|n| match n {
                Node::Element(e) if e.tag == "section" => Some(e),
                Node::Element(e) => find(&e.children),
                _ => None,
            })
        }
        let mut root = find(&tree).unwrap().clone();
        let blocks = BlockBox::collect(&root, ShapeCx::COLUMNS, tree.values());
        let (first, next) = (blocks.kids[0].id, blocks.kids[1].id);
        assert!(spacer_before_laid(&mut root, next, 80.0, tree.values()));
        let strut = root
            .children
            .iter()
            .find_map(|n| match n {
                Node::Element(e) if e.node_id == next ^ STRUT_ID => Some(e),
                _ => None,
            })
            .unwrap();
        assert_eq!(strut.style.used().height, Some(Len::Px(80.0)));
        let laid = Laid::of_boxes(&[
            (blocks.id, 0.0, 130.0),
            (first, 0.0, 20.0),
            (next ^ STRUT_ID, 20.0, 100.0),
            (next, 100.0, 130.0),
        ]);
        let breaks = blocks.breaks(130.0, &laid);
        assert_eq!(breaks.forced, vec![100.0]);
        assert!(breaks.cuts.contains(&(100.0, 100.0)));
    }
}
