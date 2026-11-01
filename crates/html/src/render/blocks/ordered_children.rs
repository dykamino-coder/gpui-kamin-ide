//! Подготовка детей flex/grid: collapsed struts и используемые размеры.

use crate::dom::whitespace::is_blank;
use crate::layout::options::RenderOpts;
mod element;
pub(super) use element::ordered_element;

use crate::dom::Node;
use crate::style::computed::{BoxStyle, FlexDir};
use crate::style::values::value::Len;

#[allow(clippy::too_many_arguments)]
pub(crate) fn ordered_children(
    collapsed: Vec<Node>,
    ordered_context: bool,
    inherited: &BoxStyle,
    opts: &RenderOpts,
) -> Vec<Node> {
    let collapsed: Vec<Node> = if ordered_context {
        collapsed
            .into_iter()
            // `visibility: collapse` на элементе гибкого контейнера убирает
            // его из строки, НО оставляет РАСПОРКУ (strut, css-flexbox §4.4):
            // поперечный размер и базовая линия ряда меряются как при нём
            // (flexbox-collapsed-item-baseline-001). Распорка — тот же
            // элемент с нулевой ГЛАВНОЙ осью и невидимой краской.
            .map(|n| match n {
                Node::Element(mut e) if e.style.used().reset_context.collapsed == Some(true) => {
                    match inherited.reset_flex.flex_dir {
                        Some(FlexDir::Col) | Some(FlexDir::ColReverse) => {
                            e.style.used_mut().reset_box_layout.height = Some(Len::Px(0.0));
                            e.style.used_mut().reset_box_layout.max_height = Some(Len::Px(0.0));
                            e.style.used_mut().reset_box_layout.min_height = Some(Len::Px(0.0));
                            e.style.used_mut().reset_box_layout.margin.top = Some(Len::Px(0.0));
                            e.style.used_mut().reset_box_layout.margin.bottom = Some(Len::Px(0.0));
                            e.style.used_mut().reset_box_layout.padding.top = Some(Len::Px(0.0));
                            e.style.used_mut().reset_box_layout.padding.bottom = Some(Len::Px(0.0));
                            e.style.used_mut().reset_border.border_width.top = Some(Len::Px(0.0));
                            e.style.used_mut().reset_border.border_width.bottom =
                                Some(Len::Px(0.0));
                        }
                        // Распорка в главной оси — ноль ЦЕЛИКОМ: элемент «as
                        // if display:none» (css-flexbox-1 §4.4), значит и его
                        // поля, отбивки и рамки по главной оси соседей не
                        // раздвигают (`flexbox_visibility-collapse`: между
                        // соседями только их собственные поля).
                        _ => {
                            e.style.used_mut().reset_box_layout.width = Some(Len::Px(0.0));
                            e.style.used_mut().reset_box_layout.max_width = Some(Len::Px(0.0));
                            e.style.used_mut().reset_box_layout.min_width = Some(Len::Px(0.0));
                            e.style.used_mut().reset_box_layout.margin.left = Some(Len::Px(0.0));
                            e.style.used_mut().reset_box_layout.margin.right = Some(Len::Px(0.0));
                            e.style.used_mut().reset_box_layout.padding.left = Some(Len::Px(0.0));
                            e.style.used_mut().reset_box_layout.padding.right = Some(Len::Px(0.0));
                            e.style.used_mut().reset_border.border_width.left = Some(Len::Px(0.0));
                            e.style.used_mut().reset_border.border_width.right = Some(Len::Px(0.0));
                        }
                    }
                    e.style.used_mut().inh.ui.hidden = Some(true);
                    Node::Element(e)
                }
                other => other,
            })
            // ПРОБЕЛЬНЫЙ текст между детьми ряда/сетки не рождает анонимный
            // элемент (css-flexbox §4): переводы строк разметки давали
            // лишние 2-3px между коробками
            // (flexbox-baseline-align-self-baseline-horiz-001: тест дышит
            // щелями, эталон написан слитно).
            .filter(|n| !matches!(n, Node::Text(_)) || !is_blank(n))
            .map(|n| match n {
                Node::Element(e) => ordered_element(e, inherited, opts),
                other => other,
            })
            .collect()
    } else {
        collapsed
    };
    collapsed
}
