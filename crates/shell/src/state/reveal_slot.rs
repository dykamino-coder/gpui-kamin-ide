//! Reveal a contributed tool in its pinned slot, including collapsed parent panels.

use crate::activity::{ActivityModel, PanelSlot};
use kamin_model::LayoutSnapshot;

/// Returns whether the active session's layout visibility needs to be saved.
pub(super) fn reveal(
    activity: &mut ActivityModel,
    layout: &mut LayoutSnapshot,
    sidebar: &mut bool,
    container: &str,
    location: Option<&str>,
) -> bool {
    use PanelSlot::*;
    // Preserve the existing preference order for duplicate legacy pins.
    let target = [
        Main,
        MainBottom,
        CentralBottom,
        RightTop,
        RightBottom,
        Sidebar,
    ]
    .into_iter()
    .find(|slot| activity.is_pinned(*slot, container))
    .unwrap_or(match location {
        Some("auxiliarybar") => RightTop,
        Some("panel") => CentralBottom,
        _ => Sidebar,
    });
    for slot in PanelSlot::ALL {
        if slot != target && activity.is_pinned(slot, container) {
            activity.unpin(slot, container);
        }
    }
    activity.pin(target, container);

    let mut opened = false;
    let mut show = |flag: &mut bool| {
        opened |= !*flag;
        *flag = true;
    };
    match target {
        Sidebar => show(sidebar),
        Main => show(&mut layout.main_visible),
        MainBottom => {
            show(&mut layout.main_visible);
            show(&mut layout.main_bottom_visible);
        }
        CentralBottom => {
            show(&mut layout.file_panel_visible);
            show(&mut layout.file_panel_bottom_visible);
        }
        RightTop => show(&mut layout.right_panel_visible),
        RightBottom => {
            show(&mut layout.right_panel_visible);
            show(&mut layout.right_panel_bottom_visible);
        }
    }
    opened
}

#[cfg(test)]
mod tests {
    use super::*;

    const CONSOLE: &str = "claudeBridgeConsole";

    #[test]
    fn revealing_an_unpinned_auxiliary_view_opens_the_right_column_once() {
        let mut activity = ActivityModel::new();
        activity.unpin(PanelSlot::CentralBottom, CONSOLE);
        let mut layout = LayoutSnapshot {
            right_panel_visible: false,
            right_panel_bottom_visible: false,
            ..Default::default()
        };
        let mut sidebar = false;
        assert!(reveal(
            &mut activity,
            &mut layout,
            &mut sidebar,
            CONSOLE,
            Some("auxiliarybar")
        ));
        assert_eq!(
            activity.state(PanelSlot::RightTop).active.as_deref(),
            Some(CONSOLE)
        );
        assert!(layout.right_panel_visible);
        assert!(!layout.right_panel_bottom_visible && !sidebar);
        assert!(!reveal(
            &mut activity,
            &mut layout,
            &mut sidebar,
            CONSOLE,
            Some("auxiliarybar")
        ));
        assert_eq!(
            activity
                .state(PanelSlot::RightTop)
                .pinned
                .iter()
                .filter(|id| id.as_str() == CONSOLE)
                .count(),
            1
        );
    }

    #[test]
    fn revealing_a_pinned_view_in_a_collapsed_right_column_reselects_it() {
        use PanelSlot::*;
        let mut activity = ActivityModel::new();
        activity.unpin(CentralBottom, CONSOLE);
        activity.pin(RightTop, CONSOLE);
        activity.set_active(RightTop, "tree");
        let mut layout = LayoutSnapshot {
            right_panel_visible: false,
            ..Default::default()
        };
        let mut sidebar = false;
        assert!(reveal(
            &mut activity,
            &mut layout,
            &mut sidebar,
            CONSOLE,
            Some("panel")
        ));
        assert_eq!(activity.slot_of(CONSOLE), Some(RightTop));
        assert_eq!(activity.state(RightTop).active.as_deref(), Some(CONSOLE));
        assert!(layout.right_panel_visible);
        assert!(!sidebar);
    }

    #[test]
    fn revealing_each_user_pinned_bottom_opens_its_parent_and_preserves_placement() {
        use PanelSlot::*;
        for target in [MainBottom, CentralBottom, RightBottom] {
            let mut activity = ActivityModel::new();
            activity.unpin(CentralBottom, CONSOLE);
            activity.pin(target, CONSOLE);
            let mut layout = LayoutSnapshot {
                main_visible: false,
                main_bottom_visible: false,
                file_panel_visible: false,
                file_panel_bottom_visible: false,
                right_panel_visible: false,
                right_panel_bottom_visible: false,
                ..Default::default()
            };
            let mut sidebar = false;
            assert!(reveal(
                &mut activity,
                &mut layout,
                &mut sidebar,
                CONSOLE,
                Some("auxiliarybar")
            ));
            assert_eq!(activity.slot_of(CONSOLE), Some(target));
            assert_eq!(
                (layout.main_visible, layout.main_bottom_visible),
                (target == MainBottom, target == MainBottom)
            );
            assert_eq!(
                (layout.file_panel_visible, layout.file_panel_bottom_visible),
                (target == CentralBottom, target == CentralBottom)
            );
            assert_eq!(
                (
                    layout.right_panel_visible,
                    layout.right_panel_bottom_visible
                ),
                (target == RightBottom, target == RightBottom)
            );
            assert!(!sidebar);
        }
    }
}
