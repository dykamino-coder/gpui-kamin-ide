//! Используемые размеры и поля детей обычного блочного потока.

mod flex;
pub(super) use flex::flex_child;

use crate::dom::Node;
use crate::render::*;
use crate::style::computed::{Align, Computed, Display};
use crate::style::values::value::Len;

#[allow(clippy::too_many_arguments)]
pub(crate) fn flow_children(
    collapsed: Vec<Node>,
    ordered_context: bool,
    inherited: &Computed,
    _opts: &RenderOpts,
    flex_context: bool,
) -> Vec<Node> {
    let collapsed: Vec<Node> = if ordered_context {
        // Элемент гибкого контейнера сжимается по умолчанию — это его
        // начальное значение в CSS. Проставляем его явно, потому что
        // `display: inline-block` в другом месте выключает сжатие: строчная
        // коробка В СТРОКЕ и правда не жмётся, а тот же элемент В РЯДУ —
        // обязан. Без этого ряд из `<span>`-ов держал свою ширину и не
        // ужимался до минимального размера содержимого.
        collapsed
            .into_iter()
            .map(|n| match n {
                Node::Element(e) if flex_context => flex_child(e, inherited),
                other => other,
            })
            .collect()
    } else {
        collapsed
            .into_iter()
            .map(|n| match n {
                Node::Element(mut e) => {
                    if e.style.flex_shrink.is_none() {
                        e.style.flex_shrink = Some(0.0);
                    }
                    // rtl: переполняющий блок с ЗАДАННОЙ шириной прижат к
                    // правому краю и вылезает влево (csswg-drafts#5572);
                    // только горизонтальное письмо — в вертикали cross-ось
                    // иная (abs-pos-border-offset-001/002).
                    if inherited.rtl == Some(true)
                        && inherited.vertical_rl.is_none()
                        && e.style.width.is_some()
                        && e.style.align_self.is_none()
                        // Блочный по ВЫЧИСЛЕННОМУ `display`, а не по тегу:
                        // `span { display: block; width: … }` в rtl-блоке —
                        // тоже блок (замер 1393 пар с rtl/картинками: +0/−0,
                        // `block-in-inline-margins-002a/b` 0.12 -> 0.00).
                        // Эталон `flexbox-writing-mode-013-ref` держит
                        // слева другое (не этот путь).
                        && (!e.inline
                            || matches!(
                                e.style.display,
                                Some(Display::Block)
                                    | Some(Display::ListItem)
                                    | Some(Display::Flex)
                                    | Some(Display::Grid)
                                    | Some(Display::Table)
                            ))
                        && !matches!(
                            e.style.position,
                            Some(crate::style::computed::Position::Absolute)
                                | Some(crate::style::computed::Position::Fixed)
                        )
                    {
                        e.style.align_self = Some(Align::End);
                    }
                    // `sideways-lr` — единственное письмо, где строчная ось
                    // идёт СНИЗУ ВВЕРХ: таблица Abstract-Physical Mapping
                    // (css-writing-modes-4, Overview.bs:1795-1830) даёт ему
                    // `line-left` = НИЗ, всем прочим вертикальным — верх.
                    // Значит начало строчной оси содержащего блока — его
                    // нижний край, и ребёнок с ОПРЕДЕЛЁННЫМ поперечным
                    // (физически вертикальным) размером стоит там:
                    // `body { height: 9em }` под корнем `sideways-lr` прижат
                    // к низу окна (`block-flow-direction-043-ref`: стол
                    // y 412…591 из 600), квадрат `height: 100px` — в НИЖНЕМ
                    // левом углу (`wm-propagation-body-035-ref`: y 492…592).
                    // Растянутого ребёнка правило не касается: `align-self`
                    // без определённого поперечного размера снимает растяжку.
                    // ★ ЗАМЕРЕНО: срез всего вертикального письма
                    // (`target/L-wm-wide.txt`, 1798 пар) 1335 -> 1354,
                    // +20/−1. Единственная потеря — `abs-pos-border-
                    // offset-002` 0.45 -> 2.22: там 68 коробок всех
                    // сочетаний письма и направления, и порог она
                    // держала не правотой, а усреднением; статическое
                    // место абсолюта при `sideways-lr` остаётся долгом
                    // корня WM-OVERCONSTRAINED-AXIS.
                    if inherited.vertical == Some(true)
                        && inherited.sideways == Some(true)
                        && inherited.vertical_rl != Some(true)
                        && matches!(
                            e.style.height,
                            Some(Len::Px(_)) | Some(Len::Em(_)) | Some(Len::Pct(_))
                        )
                        && e.style.align_self.is_none()
                        && !e.inline
                        && matches!(e.style.display, None | Some(Display::Block))
                        && !matches!(
                            e.style.position,
                            Some(crate::style::computed::Position::Absolute)
                                | Some(crate::style::computed::Position::Fixed)
                        )
                    {
                        e.style.align_self = Some(Align::End);
                    }
                    // `vertical-lr`/`vertical-rl`/`sideways-rl` при
                    // `direction: rtl`: строчная ось идёт СНИЗУ вверх —
                    // inline-start содержащего блока у НИЖНЕГО края
                    // (css-writing-modes-4 §6.4: line-right = низ, rtl
                    // ставит start на line-right). Переполненная по строчной
                    // оси коробка стоит у inline-start и вылезает к inline-end
                    // (CSS 2.2 §10.3.3 в логических осях, §7.1) — то есть
                    // низом к низу и ВВЕРХ. Без правила тело `height: 100vh`
                    // с рамками под корнем `vertical-lr; direction: rtl`
                    // лежало от верха, и красная верхняя рамка оставалась в
                    // окне (`contain-{body,html}-t-o-*` — ломался и эталон).
                    // `sideways-lr` исключён: у него line-left = низ, и при rtl
                    // start — ВЕРХ (правило выше его не касается rtl).
                    if inherited.vertical == Some(true)
                        && inherited.rtl == Some(true)
                        && !(inherited.sideways == Some(true)
                            && inherited.vertical_rl != Some(true))
                        && matches!(
                            e.style.height,
                            Some(Len::Px(_))
                                | Some(Len::Em(_))
                                | Some(Len::Pct(_))
                                | Some(Len::Vh(_))
                                | Some(Len::Vw(_))
                        )
                        && e.style.align_self.is_none()
                        && !e.inline
                        && matches!(e.style.display, None | Some(Display::Block))
                        && !matches!(
                            e.style.position,
                            Some(crate::style::computed::Position::Absolute)
                                | Some(crate::style::computed::Position::Fixed)
                        )
                    {
                        e.style.align_self = Some(Align::End);
                    }
                    // Коробка с `aspect-ratio` при auto-ширине и определённой
                    // высоте — fit-content, а не растяжка (css-sizing-4 §5.1:
                    // «automatic sizes are calculated the same as for a replaced
                    // element with a natural aspect ratio»; Blink length_utils
                    // `may_apply_aspect_ratio` → FitContent). Блок у нас —
                    // колонка flex, и `stretch` тянул ширину на всю строку
                    // (`block-aspect-ratio-002/006/…`).
                    // Нулевое и бесконечное отношение — как `auto`
                    // (css-sizing-4 §5.1; `zero-or-infinity-002`).
                    let positioned_out = matches!(
                        e.style.position,
                        Some(crate::style::computed::Position::Absolute)
                            | Some(crate::style::computed::Position::Fixed)
                    );
                    let ratio_ok = e
                        .style
                        .aspect_ratio
                        .is_some_and(|r| r.is_finite() && r > 0.0);
                    if ratio_ok
                        && !ordered_context
                        && matches!(e.style.width, None | Some(Len::Auto))
                        // Доля высоты от блока с высотой в точках — тоже
                        // определённая высота (CSS 2.1 §10.5), и ширина
                        // идёт из соотношения, а не растяжкой
                        // (`percentage-resolution-005`: 50×100 вместо 100×100).
                        && (matches!(e.style.height, Some(Len::Px(_)))
                            || (matches!(e.style.height, Some(Len::Pct(_)))
                                && matches!(inherited.height, Some(Len::Px(_)))))
                        && e.style.align_self.is_none()
                        && inherited.vertical.is_none()
                        && !e.inline
                        && !positioned_out
                    {
                        e.style.align_self = Some(Align::Start);
                    }
                    Node::Element(e)
                }
                other => other,
            })
            .collect()
    };
    collapsed
}
