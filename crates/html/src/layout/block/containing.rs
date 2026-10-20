//! Ширина содержимого коробки как содержащий блок её детей на спуске схлопывания полей.
// owner: A

use crate::document::state::ValueCx;
use crate::render::in_flow;
use crate::style::computed::Computed;
use crate::style::values::value::Len;

/// Ширина СОДЕРЖИМОГО коробки в точках — содержащий блок её детей (CSS 2.1
/// §10.1 п.2) — для процентных полей внуков в цепочках схлопывания. `cb` —
/// содержащий блок самой коробки; `None` — в точках не выводится.
fn inner_width_px(c: &Computed, cb: Option<f32>) -> Option<f32> {
    let side = |l: Option<Len>| match l {
        None | Some(Len::Auto) => Some(0.0),
        Some(Len::Px(v)) => Some(v),
        Some(Len::Pct(k)) => cb.map(|w| k * w),
        _ => None,
    };
    let b = c.borders();
    let edges =
        || Some(side(c.padding.left)? + side(c.padding.right)? + side(b.left)? + side(b.right)?);
    match c.width {
        Some(Len::Px(w)) if c.border_box == Some(true) => Some((w - edges()?).max(0.0)),
        Some(Len::Px(w)) => Some(w),
        Some(Len::Pct(k)) if c.border_box != Some(true) => cb.map(|w| k * w),
        // §10.3.3: блок в потоке с `width: auto` занимает содержащий блок за
        // вычетом своих полей, рамок и отступов (auto-поле — ноль).
        None | Some(Len::Auto) if in_flow(c) => {
            Some((cb? - side(c.margin.left)? - side(c.margin.right)? - edges()?).max(0.0))
        }
        _ => None,
    }
}

/// `f` под шириной содержимого коробки `c` вместо ширины уровня. Прежде
/// цепочка мерила доли внуков от содержащего блока УРОВНЯ: у `body` с
/// `width: auto` его нет, и `15%` у `#parent` внутри `#grand-parent {width:
/// 400px}` роняло цепочку целиком — поля абзаца и `#parent` складывались
/// (16 + 60 вместо 60, `margin-percentage-inherit-001`).
pub(super) fn with_inner_cb<'a, T>(
    c: &Computed,
    vcx: ValueCx<'a>,
    f: impl FnOnce(ValueCx<'a>) -> T,
) -> T {
    let level = vcx.collapse.width;
    // Кегль уровня — тоже от спуска: поля детей в `em` без своего кегля (и
    // с кеглем в `em`) меряются от кегля ЭТОЙ коробки, а не уровня выше.
    // Прежде `html{font-size:2em} p{font-size:.5em}` схлопывал поле абзаца
    // сквозь `body` по 8 точкам вместо 16 (`numbers-units-021`).
    let font = vcx.collapse.font;
    let own = match c.font_size {
        Some(Len::Px(v)) => v,
        Some(Len::Em(k)) | Some(Len::Pct(k)) => k * font,
        _ => font,
    };
    f(
        vcx.with_collapse(crate::layout::block::margins::CollapseInputs {
            font: own,
            width: inner_width_px(c, level),
            ..vcx.collapse
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::state::DocState;
    use crate::layout::block::margins::CollapseInputs;

    #[test]
    fn descent_changes_only_child_length_inputs() {
        let doc = DocState::new(Default::default());
        let parent = doc.values().with_collapse(CollapseInputs {
            font: 20.0,
            width: Some(200.0),
            height_def: true,
        });
        let style = Computed {
            width: Some(Len::Pct(0.5)),
            font_size: Some(Len::Em(2.0)),
            ..Default::default()
        };
        let snapshot = with_inner_cb(&style, parent, |child| {
            assert_eq!(
                child.collapse,
                CollapseInputs {
                    font: 40.0,
                    width: Some(100.0),
                    height_def: true
                }
            );
            let grandchild = Computed {
                font_size: Some(Len::Em(0.5)),
                ..Default::default()
            };
            with_inner_cb(&grandchild, child, |grandchild| {
                assert_eq!(grandchild.collapse.font, 20.0);
                assert_eq!(grandchild.collapse.width, Some(100.0));
            });
            child.keep()
        });
        assert_eq!(snapshot.vcx().collapse.font, 40.0);
        assert_eq!(parent.collapse.font, 20.0);
        assert_eq!(parent.collapse.width, Some(200.0));
        assert_eq!(doc.values().collapse, CollapseInputs::default());
        let other = DocState::new(Default::default());
        assert_eq!(other.values().collapse, CollapseInputs::default());
    }

    #[test]
    fn percent_padding_uses_only_the_supplied_level_width() {
        let doc = DocState::new(Default::default());
        let mut e = crate::layout::table::anon::anon_element("div", vec![]);
        e.style.padding.top = Some(Len::Pct(0.5));
        use crate::layout::block::struts::{top_edge_open, top_edge_open_at};
        assert!(top_edge_open_at(&e, Some(0.0)));
        assert!(!top_edge_open_at(&e, Some(100.0)));
        assert!(!top_edge_open(&e));
        assert_eq!(doc.values().collapse.width, None);
    }
}
