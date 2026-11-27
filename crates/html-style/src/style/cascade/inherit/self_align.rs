//! Стадия коробки, самовыравнивание: `normal` у элемента flex = `stretch`, оси `self-*` по письму
//! самого элемента, концы письма под `wrap-reverse` родителя (css-align-3 §6.1–6.2).
//! The used `align-self` of a box: the author's value applies to flex and grid
//! items and absolutely-positioned boxes only; any other box takes block
//! layout's placement (`SharedStyle::flow_align`).

use super::*;
use crate::style::computed::{Align, BoxStyle};

fn flip(a: Option<Align>) -> Option<Align> {
    match a {
        Some(Align::Start) => Some(Align::End),
        Some(Align::End) => Some(Align::Start),
        other => other,
    }
}

pub(super) fn self_alignment(
    parent: crate::box_facts::ApplicabilityInputs,
    own: &Computed,
    c: &mut BoxStyle,
) {
    let flex_parent = parent.flex;
    // css-align-3 §6.1: `align-self` applies to flex items, grid items and
    // absolutely-positioned boxes only ("does not apply to block-level
    // boxes", §6.2). Any other box takes the placement block layout gave it
    // (`SharedStyle::flow_align`: our block is a flex column) or none; the
    // author's value stays in the computed stage for `getComputedStyle`.
    // Blink reads the property only in flex and grid layout
    // (`ComputedStyle::ResolvedAlignSelf`), and a `-webkit-box` parent
    // resolves `-webkit-box-align` instead.
    let positioned = matches!(
        own.reset_positioned.position,
        Some(crate::style::computed::Position::Absolute | crate::style::computed::Position::Fixed)
    );
    if !c.context.parent_flex_grid && !positioned {
        set!(c.reset_flex.align_self, c.flow_align);
        set!(c.reset_flex.align_self_normal, false);
        return;
    }
    if c.reset_flex.align_self.is_none() && c.flow_align.is_some() {
        set!(c.reset_flex.align_self, c.flow_align);
    }
    // `normal` у элемента ГИБКОГО контейнера = `stretch` (§6.2), а не
    // «пусто»: пустое значение брало `align-items` родителя
    // (`self-align-normal-flex`).
    if own.reset_flex.align_self_normal && flex_parent {
        set!(c.reset_flex.align_self, Some(Align::Stretch));
    }
    // `self-start`/`self-end` меряются по письму САМОГО элемента (css-align-3
    // §6.2). Значение уже физическое (начало = левый край при ltr), поэтому
    // зеркалим ровно тогда, когда строчная ось элемента смотрит в другую
    // сторону, чем у родителя: `flexbox-align-self-vert-002` даёт элементам
    // `direction: rtl` внутри ltr-колонки и ждёт `self-start` СПРАВА.
    // Вертикальное письмо здесь НЕ зеркалим намеренно: там ось строки уже
    // переставлена поворотом. Письмо элемента — ДЕЙСТВУЮЩЕЕ (своё или
    // унаследованное, вычисленное `direction`): незаданное `direction` у
    // элемента в rtl-колонке — тоже rtl, и зеркалить нечего (поперечную ось
    // rtl-колонки разворачивает сама раскладка, `paint/apply/`:
    // `flex_cross_reverse` / `flip`).
    if c.context.parent_grid == 0
        && own.reset_flex.align_self_own_axis
        && c.inh.writing.rtl.unwrap_or(false) != parent.rtl
    {
        set!(c.reset_flex.align_self, flip(c.reset_flex.align_self));
    }
    // `start`/`end` (и `self-*`) меряются по ПИСЬМУ, а раскладка знает только
    // гибкие концы (`apply::to_items` → `FlexStart`/`FlexEnd`), которые
    // АВТОРСКИЙ `wrap-reverse` родителя переворачивает (css-align-3 §6.1,
    // css-flexbox-1 §5.2). Концы письма меняются местами ровно тогда: taffy
    // развернёт их обратно. Переворот поперёк письма (`paint/apply/`: `flip`
    // через тот же `WrapReverse`) здесь не участвует — он виден раскладке и
    // для `flex-*`, и для `start` одинаково (`self-align-start-end-flex-001`).
    if !own.reset_flex.align_self_flex_kw && parent.wrap_reverse && flex_parent {
        set!(c.reset_flex.align_self, flip(c.reset_flex.align_self));
    }
}

#[cfg(test)]
mod tests {
    use crate::style::cascade::materialize::{box_style, materialize};
    use crate::style::computed::{Align, BoxStyle, Computed};
    use std::rc::Rc;

    /// The computed and the used `align-self` of the `<p>` under the box of
    /// `<div>`, after `edit` of the paragraph's style by layout.
    fn stages(markup: &str, edit: impl FnOnce(&mut crate::dom::SharedStyle)) -> [Option<Align>; 2] {
        let parsed = crate::dom::parse(&format!("<!doctype html>{markup}"), "");
        materialize(
            &parsed.nodes,
            &Rc::new(Computed::default()),
            parsed.values(),
        );
        let find =
            |tag: &str| crate::dom::test_support::find(&parsed.nodes, &|e| e.tag == tag).unwrap();
        let parent = box_style(&BoxStyle::default(), &find("div").style, parsed.values());
        let mut own = find("p").style.clone();
        edit(&mut own);
        let used = box_style(&parent, &own, parsed.values());
        [
            own.computed().reset_flex.align_self,
            used.reset_flex.align_self,
        ]
    }

    #[test]
    fn a_block_child_keeps_the_authored_value_without_using_it() {
        let markup = r#"<div><p style="align-self:center">x</p></div>"#;
        assert_eq!(stages(markup, |_| {}), [Some(Align::Center), None]);
    }

    #[test]
    fn flex_and_grid_items_and_absolute_boxes_use_the_authored_value() {
        for markup in [
            r#"<div style="display:flex"><p style="align-self:center">x</p></div>"#,
            r#"<div style="display:grid"><p style="align-self:center">x</p></div>"#,
            r#"<div><p style="position:absolute;align-self:center">x</p></div>"#,
        ] {
            assert_eq!(stages(markup, |_| {}), [Some(Align::Center); 2], "{markup}");
        }
    }

    #[test]
    fn inherit_takes_the_parent_computed_value_in_every_container() {
        let block = r#"<div style="align-self:center"><p style="align-self:inherit">x</p></div>"#;
        assert_eq!(stages(block, |_| {}), [Some(Align::Center), None]);
        let grid = r#"<div style="display:grid;align-self:center"><p style="align-self:inherit">x</p></div>"#;
        assert_eq!(stages(grid, |_| {}), [Some(Align::Center); 2]);
    }

    #[test]
    fn block_layout_placement_replaces_the_inapplicable_value() {
        let markup = r#"<div><p style="align-self:center">x</p></div>"#;
        let placed = stages(markup, |s| s.set_flow_align(Align::End));
        assert_eq!(placed, [Some(Align::Center), Some(Align::End)]);
        let flex = r#"<div style="display:flex"><p style="align-self:center">x</p></div>"#;
        let authored = stages(flex, |s| s.set_flow_align(Align::End));
        assert_eq!(authored, [Some(Align::Center); 2]);
    }
}
