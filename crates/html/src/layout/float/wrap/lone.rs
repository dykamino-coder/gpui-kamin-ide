//! Одинокий флоат с хвостом: арифметический случай ряда без флекс-раскладки.

use crate::dom::{Element, Node};
use crate::layout::float::band_host::{BandPiece, band_piece};
use crate::render::{inline_level_box, is_blank, replaced_inline};
use crate::style::computed::{Align, Computed, Display};
use crate::style::values::value::Len;
use std::ops::ControlFlow;

#[allow(clippy::too_many_arguments)]
pub(super) fn wrap_lone_float(
    parent: &Computed,
    out: &mut Vec<Node>,
    i: &mut usize,
    side: i8,
    floaters: &mut Vec<Element>,
    j: usize,
    rest: &mut Vec<Node>,
    out_of_flow: &mut Vec<Node>,
) -> ControlFlow<()> {
    if rest.iter().all(is_blank) && floaters.len() == 1 {
        {
            let mut lone = floaters.remove(0);
            lone.style.flex_shrink = None;
            // ПРОБОВАЛИ И ОТКАТИЛИ: заодно делать строчный по природе тег
            // блочным при `float: right` (обещание комментария ниже, кода
            // не было). Замерено: приобретено 4, потеряно 5 —
            // `float-nowrap-*` и `border-color-006`. Возвращать вместе с
            // §9.7 целиком.
            //
            // Обтекать нечем — но сторону блок обязан держать: `float: right`
            // без соседей всё равно стоит У ПРАВОГО края. Ряда тут нет, и
            // сторону задаёт выравнивание себя в колонке родителя. Оно
            // действует только на ЭЛЕМЕНТ раскладки, поэтому строчный по
            // природе тег (картинка) здесь же делается блочным: иначе он
            // уходит в абзац, и выравнивание достаётся абзацу, а не ему.
            // Выравнивание себя действует только на ЭЛЕМЕНТ раскладки:
            // строчный по природе тег иначе уходит в абзац, и сторона
            // достаётся абзацу, а не картинке. Гейт узкий — только
            // замещаемый тег и только когда перед ним в блоке ничего нет:
            // широкий уже мерился в минус (запись выше).
            // Доля размера у замещаемого считается от содержащего блока,
            // и блокификация его подменяет: `<iframe height="50%">` теряет
            // отсчёт (`float-replaced-height-005`). Такие остаются как есть.
            let pct_size = matches!(lone.style.width, Some(Len::Pct(_)))
                || matches!(lone.style.height, Some(Len::Pct(_)));
            if replaced_inline(&lone.tag) && !pct_size && out.iter().all(is_blank) {
                lone.inline = false;
                lone.style.display = Some(Display::Block);
            }
            // Перед флоатом стоят одни АТОМЫ (замещаемые и строчные
            // блоки): они и флоат обязаны остаться в ОДНОЙ строке, а
            // сторону флоат держит сам. Выражается гибким рядом с
            // раздачей по краям — блокификация тут не годится, она
            // унесла бы флоат на свою строку
            // (`borders/border-color-001-ref`: вторая картинка обязана
            // стоять у правого края той же строки).
            // Атомом здесь считается и замещаемый тег БЕЗ заданных
            // размеров: у картинки они приходят из файла, а `band_piece`
            // требует точек.
            let atom_like = |n: &Node| match n {
                Node::Text(_) => false,
                Node::Element(e) => {
                    band_piece(n) == Some(BandPiece::Atom)
                        || (replaced_inline(&e.tag) && e.style.float.is_none())
                }
            };
            let lead_at = out
                .iter()
                .rposition(|n| !is_blank(n) && !atom_like(n))
                .map_or(0, |p| p + 1);
            let lead_atoms = out[lead_at..].iter().any(atom_like);
            // ЛЕВЫЙ флоат, перед которым в этом же блоке уже вышел
            // строчный прогон (§9.5 п.1 и п.6): его верх — верх ТЕКУЩЕЙ
            // строки, а сама строка вокруг него сужается, то есть на
            // экране он стоит ЛЕВЕЕ прогона, хотя в разметке идёт после.
            // Мы же дописывали его блоком следом, и полосы менялись
            // местами (`box-generation-001`: жёлтая «Float» уезжала на 70
            // точек вправо от оранжевой «Inline box»).
            let inline_run_like = |n: &Node| match n {
                Node::Text(_) => true,
                Node::Element(e) => e.style.float.is_none() && inline_level_box(e),
            };
            let run_at = out
                .iter()
                .rposition(|n| !is_blank(n) && !inline_run_like(n))
                .map_or(0, |p| p + 1);
            // Прогон из одних пробелов строки не образует (§16.6.1: они
            // схлопываются), и флоату сужать нечего — он остаётся
            // одиночным блоком со своей стороной ниже. Иначе ряд держал
            // `float: left` у верха колонки `sideways-lr`, где line-left —
            // низ (`shape-outside-*-026-ref`: пробелы вокруг флоата).
            if side < 0 && !lead_atoms && out[run_at..].iter().any(|n| !is_blank(n)) {
                let row: Vec<Node> = out.split_off(run_at);
                let mut children = vec![Node::Element(lone)];
                children.extend(row);
                out.push(Node::Element(Element {
                    list_item: None,
                    node_id: 0,
                    anim: None,
                    tag: "float-row".into(),
                    style: Computed {
                        display: Some(Display::Flex),
                        ..Computed::default()
                    },
                    hover: None,
                    first_letter: None,
                    first_line: None,
                    children,
                    attrs: vec![],
                    inline: false,
                }));
                out.extend(std::mem::take(rest));
                out.extend(std::mem::take(out_of_flow));
                *i = j;
                return ControlFlow::Break(());
            }
            if lead_atoms && side > 0 {
                let mut row: Vec<Node> = out.split_off(lead_at);
                lone.style.margin.left = Some(Len::Auto);
                row.push(Node::Element(lone));
                out.push(Node::Element(Element {
                    list_item: None,
                    node_id: 0,
                    anim: None,
                    tag: "float-row".into(),
                    style: Computed {
                        display: Some(Display::Flex),
                        ..Computed::default()
                    },
                    hover: None,
                    first_letter: None,
                    first_line: None,
                    children: row,
                    attrs: vec![],
                    inline: false,
                }));
                out.extend(std::mem::take(rest));
                out.extend(std::mem::take(out_of_flow));
                *i = j;
                return ControlFlow::Break(());
            }
            // `sideways-lr`: line-left — НИЗ (css-writing-modes-4 §6.3),
            // и `float: left` прижимается к нижнему краю колонки.
            let line_left_bottom = parent.vertical == Some(true)
                && parent.vertical_rl != Some(true)
                && parent.sideways == Some(true);
            lone.style.align_self = Some(if (side < 0) != line_left_bottom {
                Align::Start
            } else {
                Align::End
            });
            out.push(Node::Element(lone));
        }
        out.extend(std::mem::take(rest));
        out.extend(std::mem::take(out_of_flow));
        *i = j;
        return ControlFlow::Break(());
    }
    ControlFlow::Continue(())
}
