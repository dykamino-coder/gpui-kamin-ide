//! Vertical grids with indefinite inline tracks measure child text before stretch.
use crate::{
    computed::{Align, Computed, Display},
    dom::Node,
    value::Len,
};

/// Строчная ось вертикального контейнера по СОДЕРЖИМОМУ: кому из детей
/// повёрнутый абзац обязан заявить высоту строкой (`hug_inline`).
///
/// Высоту повёрнутый абзац не заявляет (`VerticalText::request_layout`):
/// длину строки решает родитель. Когда родитель сам размером в содержимое по
/// строчной оси, решать некому — коробка схлопывалась в свои рамки, а глиф
/// висел ниже (`target/mt/gr.html`, случай 2). Такой родитель — вертикальная
/// СЕТКА с невытягивающим `justify-self`/`justify-items`: строчная ось
/// элемента — по содержимому (css-grid-1 §6.6, css-align-3 §6.1 —
/// растягивает только `stretch`/`normal`). Пометка идёт и вниз по цепочке
/// потоковых блоков с `auto` высотой (`merged.hug_inline` — собственный флаг
/// контейнера, `inline::inherit` начинает с `own.clone()`): их строчный
/// размер — тот же shrink-to-fit. Вертикальный флоат (случай 4) сюда не
/// доходит: его строит хост полос, и `float` до сборщика детей не доезжает.
pub(crate) fn children(children: Vec<Node>, own: &Computed, merged: &Computed) -> Vec<Node> {
    let auto_inline = |c: &Computed| matches!(c.height, None | Some(Len::Auto));
    let grid = matches!(own.display, Some(Display::Grid) | Some(Display::InlineGrid));
    let shrink = merged.hug_claim && auto_inline(own) && !grid;
    if !grid && !shrink {
        return children;
    }
    let mut out = children;
    for node in out.iter_mut() {
        let Node::Element(ch) = node else { continue };
        // Ортогональный ребёнок (своё горизонтальное письмо) — не наш случай:
        // его строчная ось горизонтальна.
        if ch.inline
            || ch.style.vertical == Some(false)
            || !crate::render::in_flow(&ch.style)
            || !auto_inline(&ch.style)
        {
            continue;
        }
        let hug = if grid {
            // Auto inline tracks need a contribution before stretch can resolve their size.
            auto_inline(own)
                || matches!(
                    ch.style.justify_self.or(own.justify_items),
                    Some(Align::Start)
                        | Some(Align::Center)
                        | Some(Align::End)
                        | Some(Align::Baseline)
                )
        } else {
            !matches!(
                ch.style.display,
                Some(Display::Flex) | Some(Display::Grid) | Some(Display::Table)
            )
        };
        if hug {
            ch.style.hug_inline = true;
            ch.style.hug_claim = true;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn auto_grid_tracks_claim_intrinsics_but_definite_tracks_keep_stretch() {
        let mut parent = Computed {
            display: Some(Display::Grid),
            vertical: Some(true),
            ..Computed::default()
        };
        let child = crate::render::anon_element("div", vec![]);
        let claimed = |child, parent: &Computed| {
            let out = children(vec![Node::Element(child)], parent, parent);
            let Node::Element(child) = &out[0] else {
                unreachable!()
            };
            (child.style.hug_inline, child.style.hug_claim)
        };
        assert_eq!(claimed(child.clone(), &parent), (true, true));
        parent.height = Some(Len::Px(50.0));
        assert_eq!(claimed(child.clone(), &parent), (false, false));
        parent.justify_items = Some(Align::Start);
        assert_eq!(claimed(child.clone(), &parent), (true, true));
        parent.height = None;
        let mut fixed = child.clone();
        fixed.style.height = Some(Len::Px(50.0));
        assert_eq!(claimed(fixed, &parent), (false, false));
        let mut horizontal = child.clone();
        horizontal.style.vertical = Some(false);
        assert_eq!(claimed(horizontal, &parent), (false, false));
        let mut positioned = child;
        positioned.style.position = Some(crate::computed::Position::Absolute);
        assert_eq!(claimed(positioned, &parent), (false, false));
    }
}
