//! ::scroll-marker и ::scroll-marker-group.

mod marker_collect;
use crate::dom::scroll_markers::marker_collect::collect_scroll_markers;
use crate::dom::scroll_markers::marker_collect::purge_scroll_markers;

use crate::dom::*;
use crate::style::computed::{Computed, Display, Position};
use crate::style::css::{Decls, Rule};
use crate::style::select::matching::matches_ignoring_pseudo;
use crate::style::select::{Ancestor, Sibs};

/// Псевдокоробки скроллера ВНЕ его коробки (css-overflow-5): группа маркеров
/// и кнопки прокрутки. Порядок в дереве — Blink `kBoxTreeOrder`.
pub(super) struct ScrollPseudos {
    pub(crate) group_before: Option<Element>,
    pub(crate) group_after: Option<Element>,
    /// block-start, inline-start, inline-end, block-end — те, у кого есть
    /// `content`.
    pub(crate) buttons: Vec<Element>,
}

/// Собрать группу маркеров и кнопки элемента; `children` — уже построенные
/// дети (с их `::scroll-marker`), из них маркеры ВЫНИМАЮТСЯ.
///
/// Скроллер (overflow scroll/auto/hidden — Blink `IsScrollContainer`) или
/// корень: со свойством `scroll-marker-group` собирает маркеры потомков, без
/// него — гасит их (§scroll-markers: «nearest ancestor scroll container …
/// not none»). Группа создаётся по одному свойству, даже без правил
/// `::scroll-marker-group` (`scroll-marker-group-015`), блокифицируется и
/// получает `contain: layout` (+ `size` в потоке) поверх авторского
/// (`style_adjuster.cc` 827, 1219–1230; `scroll-marker-007/008`).
#[allow(clippy::too_many_arguments)]
pub(super) fn scroll_marker_pass(
    rules: &[Rule],
    vars: &Decls,
    counters: &mut crate::style::generated::counters::Counters,
    me: &Ancestor,
    path: &[Ancestor],
    sibs: Sibs,
    tag: &str,
    style: &Computed,
    attrs: &[(String, String)],
    children: &mut Vec<Node>,
) -> ScrollPseudos {
    use crate::style::computed::Overflow;
    let mut out = ScrollPseudos {
        group_before: None,
        group_after: None,
        buttons: vec![],
    };
    // Кнопки — по `content`, скроллер не обязателен (Blink
    // `CanGeneratePseudoElement`; `scroll-buttons-001` — `div` без overflow).
    // Физическая сторона переводится в логическую по письму элемента
    // (`scroll_button_pseudo_element.cc` PhysicalToLogical).
    if rules.iter().any(|r| {
        r.sel
            .pseudo
            .as_deref()
            .is_some_and(|p| p.starts_with("scroll-button("))
    }) {
        let vertical = style.vertical == Some(true);
        let rl = style.vertical_rl == Some(true);
        let rtl = style.rtl == Some(true);
        for logical in ["block-start", "inline-start", "inline-end", "block-end"] {
            let physical = match (logical, vertical) {
                ("block-start", false) => "up",
                ("block-end", false) => "down",
                ("inline-start", false) => {
                    if rtl {
                        "right"
                    } else {
                        "left"
                    }
                }
                ("inline-end", false) => {
                    if rtl {
                        "left"
                    } else {
                        "right"
                    }
                }
                ("block-start", true) => {
                    if rl {
                        "right"
                    } else {
                        "left"
                    }
                }
                ("block-end", true) => {
                    if rl {
                        "left"
                    } else {
                        "right"
                    }
                }
                ("inline-start", true) => {
                    if rtl {
                        "down"
                    } else {
                        "up"
                    }
                }
                _ => {
                    if rtl {
                        "up"
                    } else {
                        "down"
                    }
                }
            };
            let l = format!("scroll-button({logical})");
            let p = format!("scroll-button({physical})");
            if let Some(el) = pseudo_box_named(
                rules,
                vars,
                counters,
                me,
                path,
                sibs,
                &[l.as_str(), p.as_str(), "scroll-button(*)"],
                &l,
                false,
                attrs,
            ) {
                out.buttons.push(el);
            }
        }
    }
    let scrolls =
        |o: Option<Overflow>| matches!(o, Some(Overflow::Scroll) | Some(Overflow::Hidden));
    let scroller = tag == "html" || scrolls(style.overflow_x) || scrolls(style.overflow_y);
    let Some(before) = style.scroll_marker_group else {
        if scroller {
            purge_scroll_markers(children);
        }
        return out;
    };
    // Группа — только у скролл-контейнера (§scroll-marker-group-property:
    // «on a scroll container … generates a ::scroll-marker-group»; Blink
    // `CanGeneratePseudoElement`: `IsScrollContainer()`); у `div` без
    // overflow свойство молчит (`scroll-marker-group-010`).
    if !scroller {
        return out;
    }
    let mut markers = vec![];
    {
        let abs_ok = crate::text::inline::establishes_cb(style);
        let fixed_ok = style.transform.is_some()
            || style.contain_layout == Some(true)
            || style.contain_paint == Some(true);
        collect_scroll_markers(children, abs_ok, fixed_ok, &mut markers);
    }
    let mut matched: Vec<&Rule> = rules
        .iter()
        .filter(|r| r.sel.pseudo.as_deref() == Some("scroll-marker-group"))
        .filter(|r| matches_ignoring_pseudo(&r.sel, me, path, sibs))
        .collect();
    let mut gstyle = Computed::resolve_with_vars(&mut matched, &Decls::new(), vars);
    if gstyle.display == Some(Display::None) {
        return out;
    }
    gstyle.display = match gstyle.display {
        Some(Display::InlineBlock) => Some(Display::Block),
        Some(Display::InlineFlex) => Some(Display::Flex),
        Some(Display::InlineGrid) => Some(Display::Grid),
        Some(Display::InlineTable) => Some(Display::Table),
        other => other,
    };
    gstyle.contain_layout = Some(true);
    if !matches!(
        gstyle.position,
        Some(Position::Absolute) | Some(Position::Fixed)
    ) {
        gstyle.contain_size = Some(true);
    }
    let group = Element {
        list_item: None,
        node_id: 0,
        anim: None,
        inline: false,
        tag: "::scroll-marker-group".to_string(),
        style: gstyle,
        hover: None,
        first_letter: None,
        first_line: None,
        children: markers.into_iter().map(Node::Element).collect(),
        attrs: vec![],
    };
    if before {
        out.group_before = Some(group);
    } else {
        out.group_after = Some(group);
    }
    out
}
