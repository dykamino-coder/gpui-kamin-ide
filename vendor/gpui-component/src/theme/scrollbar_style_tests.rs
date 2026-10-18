//! Application scrollbar policies run on each projection and survive config loads.

use super::{ScrollbarStyles, Theme, ThemeConfig};
use std::{
    rc::Rc,
    sync::atomic::{AtomicUsize, Ordering},
};

#[test]
fn application_policy_survives_theme_reload_and_reprojects() {
    static CALLS: AtomicUsize = AtomicUsize::new(0);
    fn configure(styles: ScrollbarStyles) -> ScrollbarStyles {
        CALLS.fetch_add(1, Ordering::Relaxed);
        styles.track(|style| style.width(gpui::px(8.)))
    }
    let mut theme = Theme::default();
    assert!(theme.scrollbar_style.is_none());
    theme.scrollbar_style = Some(configure);
    let _ = theme.base_theme();
    assert_eq!(CALLS.load(Ordering::Relaxed), 1);
    theme.apply_config(&Rc::new(ThemeConfig::default()));
    let _ = theme.base_theme();
    assert_eq!(CALLS.load(Ordering::Relaxed), 2);
}
