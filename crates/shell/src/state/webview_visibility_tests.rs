//! Матрица layout parent/child, session, registry и view в другом слоте.

use super::*;

fn all_visible() -> Layout {
    Layout {
        sidebar: true,
        main: true,
        main_bottom: true,
        file: true,
        file_bottom: true,
        right: true,
        right_bottom: true,
        has_active: true,
        has_open: true,
    }
}

#[test]
fn every_slot_obeys_parent_and_child_flags() {
    let names = [
        "sidebar",
        "main",
        "mainBottom",
        "centralBottom",
        "rightTop",
        "rightBottom",
    ];
    for slot in names {
        assert!(all_visible().renders(slot));
    }
    let hidden = Layout::default();
    for slot in names {
        assert!(!hidden.renders(slot));
    }
    let layout = Layout {
        sidebar: false,
        main: false,
        file: false,
        right: false,
        ..all_visible()
    };
    for slot in names {
        assert!(!layout.renders(slot), "parent hidden: {slot}");
    }
    let layout = Layout {
        main_bottom: false,
        file_bottom: false,
        right_bottom: false,
        ..all_visible()
    };
    for slot in ["mainBottom", "centralBottom", "rightBottom"] {
        assert!(!layout.renders(slot));
    }
    for slot in ["main", "sidebar", "rightTop"] {
        assert!(layout.renders(slot));
    }
}

#[test]
fn no_sessions_shows_sidebar_and_welcome_instead_of_webview() {
    let layout = Layout {
        has_open: false,
        has_active: false,
        ..all_visible()
    };
    assert!(layout.renders("sidebar"));
    for slot in [
        "main",
        "mainBottom",
        "centralBottom",
        "rightTop",
        "rightBottom",
    ] {
        assert!(!layout.renders(slot), "no sessions: {slot}");
    }
    let layout = Layout {
        has_active: false,
        ..all_visible()
    };
    assert!(!layout.renders("main"));
    assert!(layout.renders("rightTop"));
}

#[test]
fn incomplete_registry_does_not_keep_hidden_slot_visible() {
    let mut slots = Slots::default();
    slots.resolve("rightBottom", Some("tool"), Some(vec!["view".into()]));
    let cached = slots.resolve("rightBottom", Some("tool"), None);
    assert_eq!(cached, ["view"]);
    let layout = Layout {
        right_bottom: false,
        ..all_visible()
    };
    let visible: Vec<_> = cached
        .into_iter()
        .filter(|_| layout.renders("rightBottom"))
        .collect();
    assert!(visible.is_empty());
    assert!(
        slots
            .resolve("rightBottom", Some("replacement"), None)
            .is_empty()
    );
    assert!(slots.resolve("rightBottom", None, None).is_empty());
}

#[test]
fn same_view_rendered_elsewhere_survives_one_hidden_slot() {
    let mut slots = Slots::default();
    let layout = Layout {
        right_bottom: false,
        ..all_visible()
    };
    let visible: Vec<_> = ["rightBottom", "main"]
        .into_iter()
        .flat_map(|slot| {
            let views = slots.resolve(slot, Some("tool"), Some(vec!["same-view".into()]));
            if layout.renders(slot) {
                views
            } else {
                Vec::new()
            }
        })
        .collect();
    assert_eq!(visible, ["same-view"]);
}

#[test]
fn customize_retains_warm_views_without_leaving_them_visible() {
    let mut slots = Slots::default();
    let snapshot = || {
        [Slot {
            name: "rightBottom",
            tool: Some("tool".into()),
            views: Some(vec!["view".into()]),
        }]
    };
    let (visible, retained) = slots.collect(
        &all_visible(),
        true,
        snapshot(),
        true,
        Some("settings".into()),
    );
    assert_eq!(visible, ["settings"]);
    assert_eq!(retained, ["view", "browser"]);
    let (visible, retained) = slots.collect(
        &all_visible(),
        false,
        snapshot(),
        true,
        Some("settings".into()),
    );
    assert_eq!(visible, ["view", "browser"]);
    assert!(retained.is_empty());
    let hidden = Layout {
        right_bottom: false,
        file: false,
        ..all_visible()
    };
    let (visible, retained) = slots.collect(&hidden, true, snapshot(), true, None);
    assert!(visible.is_empty());
    assert!(
        retained.is_empty(),
        "layout-hidden views must remain eligible for eviction"
    );
}
