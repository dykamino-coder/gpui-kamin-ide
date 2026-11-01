//! Проверки наследования и представления.
use super::*;
#[test]
fn word_break_keeps_its_own_fields() {
    let c = style("overflow-wrap: normal; word-break: break-word");
    assert_eq!(c.inh.text_flow.break_word, Some(false));
    assert_eq!(c.inh.text_flow.wrap_anywhere, Some(false));
    assert_eq!(c.inh.text_flow.word_break_word, Some(true));
    let c = style("line-break: anywhere");
    assert_eq!(c.inh.text_flow.break_anywhere, None);
    assert_eq!(c.inh.text_flow.break_anywhere_strict, Some(true));
}

#[test]
fn line_clamp_expands_to_max_lines_ellipsis_and_continue() {
    let c = style("line-clamp: 2");
    assert_eq!(c.reset_fragmentation.line_clamp, Some(2));
    assert_eq!(c.inh.text_flow.clamp_mark, Some(None));
    assert_eq!(c.reset_fragmentation.clamp_auto, Some(false));
    assert_eq!(
        (
            c.reset_fragmentation.clamp_continue,
            c.inh.text_flow.clamp_legacy
        ),
        (Some(true), Some(false))
    );
    assert!(!c.clamp_by_height());
    let c = style("line-clamp: auto");
    assert_eq!(c.reset_fragmentation.line_clamp, None);
    assert!(c.clamp_by_height());
    // Lines and the box height both clamp (`line-clamp-041`).
    let c = style("line-clamp: 4 auto");
    assert_eq!(c.reset_fragmentation.line_clamp, Some(4));
    assert!(c.clamp_by_height());
    let c = style("line-clamp: auto no-ellipsis");
    assert_eq!(c.used_block_ellipsis().as_deref(), Some(""));
    assert!(c.clamp_by_height());
    // `continue: collapse` alone: `max-lines` is initially `auto`.
    assert!(style("continue: collapse").clamp_by_height());
    let c = style("line-clamp: 3 'x' -webkit-legacy");
    assert_eq!(c.inh.text_flow.clamp_mark, Some(Some("x".into())));
    assert_eq!(c.inh.text_flow.clamp_legacy, Some(true));
    let c = style("line-clamp: none");
    assert_eq!(c.used_block_ellipsis().as_deref(), Some(""));
    let c = style("-webkit-line-clamp: 4");
    assert_eq!(c.reset_fragmentation.line_clamp, Some(4));
    assert_eq!(c.used_block_ellipsis(), None);
    assert_eq!(c.inh.text_flow.clamp_legacy, Some(true));
    // `block-ellipsis` is inherited: the root default is `no-ellipsis`.
    assert_eq!(
        Computed::default().used_block_ellipsis().as_deref(),
        Some("")
    );
}

#[test]
fn text_box_shorthand_defaults_to_trim_both() {
    use crate::style::computed::TextEdge;
    let c = style("text-box: cap alphabetic");
    assert!(c.reset_text_box.text_box_trim_start && c.reset_text_box.text_box_trim_end);
    assert_eq!(
        (
            c.inh.text_flow.text_box_over,
            c.inh.text_flow.text_box_under
        ),
        (TextEdge::Cap, TextEdge::Alphabetic)
    );
    let c = style("text-box: trim-start");
    assert!(c.reset_text_box.text_box_trim_start && !c.reset_text_box.text_box_trim_end);
    assert!(c.inh.text_flow.text_box_edge_set);
    let c = style("text-box: normal");
    assert!(!c.reset_text_box.text_box_trim_start && !c.reset_text_box.text_box_trim_end);
    for invalid in ["none", "cap trim-start alphabetic", "trim-start trim-end"] {
        let c = style(&format!("text-box: trim-end; text-box: {invalid}"));
        assert!(
            !c.reset_text_box.text_box_trim_start && c.reset_text_box.text_box_trim_end,
            "{invalid}"
        );
    }
}

#[test]
fn tab_size_number_and_length_replace_each_other() {
    let c = style("tab-size: 40px; tab-size: 2.5");
    assert_eq!(
        (c.inh.text_flow.tab_size, c.inh.text_flow.tab_size_len),
        (Some(2.5), None)
    );
    let c = style("tab-size: 2.5; tab-size: 40px");
    assert_eq!(
        (c.inh.text_flow.tab_size, c.inh.text_flow.tab_size_len),
        (None, Some(Len::Px(40.0)))
    );
    for invalid in ["-1", "-1px", "10%", "auto", "NaN", "inf", "6.", "1e99"] {
        let c = style(&format!("tab-size: 2.5; tab-size: {invalid}"));
        assert_eq!(
            (c.inh.text_flow.tab_size, c.inh.text_flow.tab_size_len),
            (Some(2.5), None),
            "{invalid}"
        );
    }
}

#[test]
fn user_select_is_not_inherited_and_auto_follows_the_parent_box() {
    use crate::style::cascade::inherit::inherit;
    use crate::style::computed::UserSelect;
    let doc = DocumentValues::detached(Media::default());
    let vcx = doc.values();
    let none = inherit(&BoxStyle::default(), &style("user-select: none"), vcx);
    assert_eq!(none.reset_context.user_select, Some(UserSelect::None));
    assert_eq!(none.reset_context.no_select, Some(true));
    // css-ui-4 §6.1: the computed value is not inherited ...
    let child = inherit(&none, &Computed::default(), vcx);
    assert_eq!(child.reset_context.user_select, None);
    // ... but the used value of `auto` under a `none` box is `none`.
    assert_eq!(child.reset_context.no_select, Some(true));
    assert_eq!(
        inherit(&none, &style("user-select: auto"), vcx)
            .reset_context
            .no_select,
        Some(true)
    );
    assert_eq!(
        inherit(&none, &style("user-select: text"), vcx)
            .reset_context
            .no_select,
        Some(false)
    );
    let all = inherit(&BoxStyle::default(), &style("user-select: all"), vcx);
    assert_eq!(
        inherit(&all, &style("user-select: auto"), vcx)
            .reset_context
            .no_select,
        Some(false)
    );
    // `inherit` takes the parent's computed keyword.
    let explicit = inherit(&all, &style("user-select: inherit"), vcx);
    assert_eq!(explicit.reset_context.user_select, Some(UserSelect::All));
    assert_eq!(
        style("-webkit-user-select: contain")
            .reset_context
            .user_select,
        Some(UserSelect::Contain)
    );
}
