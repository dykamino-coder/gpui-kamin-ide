//! Пометка `hug_inline` детям вертикального контейнера, чья строчная ось по содержимому:
//! сетка с неопределёнными строчными дорожками или невытягивающим `justify-*` меряет текст
//! ребёнка до растяжки (css-grid-1 §6.6, css-align-3 §6.1), а цепочка потоковых блоков с
//! авто-высотой наследует shrink-to-fit заявки контейнера.
use crate::dom::Node;
use crate::style::computed::{Align, Computed, Display};
use crate::style::values::value::Len;

/// Строчная ось вертикального контейнера по СОДЕРЖИМОМУ: кому из детей
/// повёрнутый абзац обязан заявить высоту строкой (`hug_inline`).
///
/// Высоту повёрнутый абзац не заявляет (`VerticalText::request_layout`):
/// длину строки решает родитель. Когда родитель сам размером в содержимое по
/// строчной оси, решать некому — коробка схлопывалась в свои рамки, а глиф
/// висел ниже (проба руками, случай 2). Такой родитель — вертикальная
/// СЕТКА с невытягивающим `justify-self`/`justify-items`: строчная ось
/// элемента — по содержимому (css-grid-1 §6.6, css-align-3 §6.1 —
/// растягивает только `stretch`/`normal`). Пометка идёт и вниз по цепочке
/// потоковых блоков с `auto` высотой (`merged.hug_inline` — собственный флаг
/// контейнера, `style::cascade::inherit` начинает с `own.clone()`): их строчный
/// размер — тот же shrink-to-fit. Вертикальный флоат (случай 4) сюда не
/// доходит: его строит хост полос, и `float` до сборщика детей не доезжает.
pub(crate) fn children(children: Vec<Node>, own: &Computed, merged: &Computed) -> Vec<Node> {
    let auto_inline = |c: &Computed| matches!(c.reset_box_layout.height, None | Some(Len::Auto));
    let grid = matches!(
        own.reset_box_layout.display,
        Some(Display::Grid) | Some(Display::InlineGrid)
    );
    let shrink = merged.reset_context.hug_claim && auto_inline(own);
    if !grid && !shrink {
        return children;
    }
    let mut out = children;
    for node in out.iter_mut() {
        let Node::Element(ch) = node else { continue };
        // Ортогональный ребёнок (своё горизонтальное письмо) — не наш случай:
        // его строчная ось горизонтальна.
        if ch.inline
            || ch.style.cascaded().vertical_decl() == Some(false)
            || !crate::layout::box_kind::in_flow(&ch.style.used())
            || !auto_inline(&ch.style.used())
        {
            continue;
        }
        let hug = if grid {
            // Auto inline tracks need a contribution before stretch can resolve their size.
            auto_inline(own)
                || matches!(
                    ch.style
                        .used()
                        .reset_flex
                        .justify_self
                        .or(own.reset_flex.justify_items),
                    Some(Align::Start)
                        | Some(Align::Center)
                        | Some(Align::End)
                        | Some(Align::Baseline)
                )
        } else {
            !matches!(
                ch.style.used().reset_box_layout.display,
                Some(Display::Flex) | Some(Display::Grid) | Some(Display::Table)
            )
        };
        if hug {
            ch.style.used_mut().reset_context.hug_inline = true;
            ch.style.used_mut().reset_context.hug_claim = true;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn auto_grid_tracks_claim_intrinsics_but_definite_tracks_keep_stretch() {
        let mut parent = {
            let mut with_inherited = Computed {
                reset_box_layout: crate::style::computed::StyleGroup::new(
                    crate::style::computed::ResetBoxLayout {
                        display: Some(Display::Grid),
                        ..Default::default()
                    },
                ),
                ..Computed::default()
            };
            with_inherited.inh.writing.writing_mode = Some(
                crate::style::computed::writing_mode::WritingModeKw::new(true, false, false),
            );
            with_inherited
        };
        let child = crate::layout::table::anon::anon_element("div", vec![]);
        let claimed = |child, parent: &Computed| {
            let out = children(vec![Node::Element(child)], parent, parent);
            let Node::Element(child) = &out[0] else {
                unreachable!()
            };
            (
                child.style.used().reset_context.hug_inline,
                child.style.used().reset_context.hug_claim,
            )
        };
        assert_eq!(claimed(child.clone(), &parent), (true, true));
        parent.reset_box_layout.height = Some(Len::Px(50.0));
        assert_eq!(claimed(child.clone(), &parent), (false, false));
        parent.reset_flex.justify_items = Some(Align::Start);
        assert_eq!(claimed(child.clone(), &parent), (true, true));
        parent.reset_box_layout.height = None;
        let mut fixed = child.clone();
        fixed.style.used_mut().reset_box_layout.height = Some(Len::Px(50.0));
        assert_eq!(claimed(fixed, &parent), (false, false));
        let mut horizontal = child.clone();
        horizontal.style.used_mut().inh.writing.writing_mode =
            Some(crate::style::computed::writing_mode::WritingModeKw::new(
                false,
                horizontal.style.cascaded().wm().block_rl(),
                horizontal.style.cascaded().wm().sideways(),
            ));
        assert_eq!(claimed(horizontal, &parent), (false, false));
        let mut positioned = child;
        positioned.style.used_mut().reset_positioned.position =
            Some(crate::style::computed::Position::Absolute);
        assert_eq!(claimed(positioned, &parent), (false, false));
    }
}
