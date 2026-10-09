//! Orthogonal horizontal children keep percentages and native intrinsic sizing.
use crate::render::in_flow;
use crate::computed::{Align, Computed, Display};
use crate::dom::Node;
use crate::value::Len;

pub(crate) fn orthogonal_children(
    children: Vec<Node>,
    container: &Computed,
    icb_w: f32,
) -> Vec<Node> {
    let mut out = children;
    let inline_size = match container.height {
        Some(Len::Px(v)) => Some(v),
        _ => None,
    };
    for node in out.iter_mut() {
        let Node::Element(ch) = node else { continue };
        if ch.inline || ch.style.vertical != Some(false) {
            continue;
        }
        // Внепоточные не зажимаются: абсолютный элемент меряется от своего
        // содержащего блока, а не от потока (available-size-003: зажатый
        // абсолютный маркер вылезал красным).
        if !in_flow(&ch.style) {
            continue;
        }
        // Writing Modes 4 §7.3: the orthogonal child's auto block size fits
        // its content. The flex row used to implement vertical block flow
        // must not stretch that physical height to a parallel sibling's line.
        if matches!(
            container.display,
            None | Some(Display::Block | Display::InlineBlock | Display::TableCell)
        ) && matches!(ch.style.height, None | Some(Len::Auto))
            && ch.style.align_self.is_none()
            && !ch.style.float.is_some_and(|side| side != 0)
        {
            let from_bottom = (container.sideways == Some(true)
                && container.vertical_rl != Some(true))
                != (container.rtl == Some(true));
            ch.style.align_self = Some(if from_bottom {
                Align::End
            } else {
                Align::Start
            });
        }
        if let Some(il) = inline_size {
            for side in [
                &mut ch.style.margin.top,
                &mut ch.style.margin.right,
                &mut ch.style.margin.bottom,
                &mut ch.style.margin.left,
            ] {
                if let Some(Len::Pct(k)) = side {
                    *side = Some(Len::Px(*k * il));
                }
            }
        }
        // Доступное место ортогонального потока (css-writing-modes-3
        // §7.3.1): фиксированный размер контейнера, а без него — НАЧАЛЬНЫЙ
        // содержащий блок. Процентная ширина htb-ребёнка в вертикальном
        // контейнере без размера считалась от сжатого по содержимому
        // родителя (two-levels-of-orthogonal-flows-percentage: 50% от
        // трёх букв вместо половины окна).
        if let Some(Len::Pct(k)) = ch.style.width {
            let base = match container.width {
                Some(Len::Px(w)) => w,
                _ => icb_w,
            };
            ch.style.width = Some(Len::Px(k * base));
        }
        // `width: auto`, записанный ЯВНО, разбор отдаёт как `Some(Len::Auto)`
        // (`value.rs`: `Len::parse("auto")`), и `is_none()` читал его как
        // заданную ширину — предел ортогонального потока не ставился вовсе.
        // Этой записью открывается каждый тест `sizing-orthog-htb-in-v*`.
        if crate::render::orthogonal_horizontal::size_auto(ch, container, icb_w) {
            continue;
        }
        let explicit_auto = matches!(ch.style.width, Some(Len::Auto));
        if (ch.style.width.is_none() || explicit_auto) && ch.style.max_width.is_none() {
            let side = |l: Option<Len>| match l {
                Some(Len::Px(v)) => v,
                _ => 0.0,
            };
            let margins = side(ch.style.margin.left) + side(ch.style.margin.right);
            match container.width {
                // Заданный размер контейнера — доступное место потока
                // целиком: авто-размер блочного ортогонального ребёнка
                // РАСТЯГИВАЕТСЯ на него (stretch-fit, css-sizing-3 §5), а не
                // жмётся к содержимому (two-levels-of-orthogonal-flows-fixed:
                // жёлтый ребёнок обязан накрыть красный контейнер 10em).
                // Явный `auto` при ЗАДАННОМ контейнере — shrink-to-fit
                // (css-writing-modes-4 §7.3.2: «min(max-content, max(min-content,
                // constraint))», constraint — размер контейнера): потолок в
                // размер контейнера, а не растяжка. Без потолка длинная строка
                // шла одной линией на всю max-content-ширину
                // (`sizing-orthog-htb-in-v{lr,rl}-010/022`). Пол min-content
                // потолком не выразить: длинное слово упрётся в предел и
                // вылезет — его эталоны (`-011/-023`) сходятся в пределах
                // допуска. Неявной ширине по-прежнему растяжка: потолок
                // вместо неё замерен и откачен (E1).
                Some(Len::Px(w)) if explicit_auto => {
                    let b = ch.style.borders();
                    let extra = side(ch.style.padding.left)
                        + side(ch.style.padding.right)
                        + side(b.left)
                        + side(b.right);
                    ch.style.max_width = Some(Len::Px((w - margins - extra).max(0.0)));
                }
                Some(Len::Px(w)) => {
                    // Ширина здесь — то, что коробке отдаст раскладка, а
                    // рендер к ЗАДАННОЙ ширине добавит отступы и рамку (как
                    // общий разбор): их доля вычитается заранее, иначе
                    // ребёнок вылезал из контейнера на их толщину.
                    let b = ch.style.borders();
                    let extra = side(ch.style.padding.left)
                        + side(ch.style.padding.right)
                        + side(b.left)
                        + side(b.right);
                    ch.style.width = Some(Len::Px((w - margins - extra).max(0.0)));
                }
                _ => {
                    // css-writing-modes-4 §7.3.1: без фиксированного размера
                    // контейнера предел ортогонального потока — начальный
                    // содержащий блок за вычетом полей ребёнка (Blink
                    // `space_utils.cc` fallback = ICB). Доля от сжатого по
                    // содержимому родителя давала бесконечную строку
                    // (`sizing-orthog-htb-in-vrl-*`).
                    // Предел §7.3.2 — «stretch fit into» ICB, то есть по
                    // ВНЕШНЕМУ краю; раскладке отдаётся размер содержимого
                    // (рендер доложит к нему рамку и отбивки), поэтому их доля
                    // вычитается заранее (`htb-in-vlr-016`: рамка 35).
                    let b = ch.style.borders();
                    let extra = side(ch.style.padding.left)
                        + side(ch.style.padding.right)
                        + side(b.left)
                        + side(b.right);
                    ch.style.max_width = Some(Len::Px((icb_w - margins - extra).max(0.0)));
                }
            }
        }
    }
    out
}
