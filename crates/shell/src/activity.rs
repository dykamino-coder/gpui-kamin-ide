//! Панельная модель 1:1 с renderer/signals/activity.ts: tool-хостящие слоты,
//! каждый — упорядоченный `pinned[]` + `active`. Реестр тулзов (built-in).
//! Мутаторы pin/unpin/set_active/move с той же семантикой (move = перенос,
//! не копия; кросс-слот отказ при дубле; active следует за тулом).
//!
//! centralTop тулзы не хостит (там file-viewer) — как в оригинале.

pub mod model;

pub use model::{ActivityModel, PanelState};
pub mod registry;

pub use registry::{dyn_tool, intern, is_singleton, lookup_any, static_id};
pub mod dyn_tools;

pub use dyn_tools::{DynTool, DynView, dyn_tools_list, set_dyn_tools};

/// Регион тела, хостящий тулзы (порядок = activity.ts PanelSlot).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum PanelSlot {
    Sidebar,
    Main,
    MainBottom,
    CentralBottom,
    RightTop,
    RightBottom,
}

impl PanelSlot {
    pub const ALL: [PanelSlot; 6] = [
        PanelSlot::Sidebar,
        PanelSlot::Main,
        PanelSlot::MainBottom,
        PanelSlot::CentralBottom,
        PanelSlot::RightTop,
        PanelSlot::RightBottom,
    ];

    /// Стабильный строковый id (persist + probe emit + ключи ui).
    pub fn as_str(self) -> &'static str {
        match self {
            PanelSlot::Sidebar => "sidebar",
            PanelSlot::Main => "main",
            PanelSlot::MainBottom => "mainBottom",
            PanelSlot::CentralBottom => "centralBottom",
            PanelSlot::RightTop => "rightTop",
            PanelSlot::RightBottom => "rightBottom",
        }
    }
}

/// Тул в реестре: id + иконка (token/codicon) + подпись.
#[derive(Clone, Debug)]
pub struct ActivityItem {
    pub id: &'static str,
    pub icon: &'static str,
    pub label: &'static str,
}

/// BUILTIN_ACTIVITIES (activity.ts:44-51) + console (Bridge Console вебвью).
pub const BUILTIN_ACTIVITIES: [ActivityItem; 6] = [
    ActivityItem {
        id: "projects",
        icon: "folders",
        label: "Projects",
    },
    ActivityItem {
        id: "tree",
        icon: "tree-view",
        label: "Folder tree",
    },
    ActivityItem {
        id: "search",
        icon: "search",
        label: "Search",
    },
    ActivityItem {
        id: "problems",
        icon: "warning",
        label: "Problems",
    },
    ActivityItem {
        id: "terminal",
        icon: "terminal",
        label: "Terminal",
    },
    ActivityItem {
        id: "extensions",
        icon: "extensions",
        label: "Extensions",
    },
];

impl Default for ActivityModel {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seeds() {
        let m = ActivityModel::new();
        assert_eq!(
            m.state(PanelSlot::Sidebar).active.as_deref(),
            Some("projects")
        );
        assert_eq!(m.state(PanelSlot::RightTop).active.as_deref(), Some("tree"));
        assert_eq!(m.state(PanelSlot::MainBottom).active, None);
    }

    #[test]
    fn pin_dedup_activate() {
        let mut m = ActivityModel::new();
        m.pin(PanelSlot::MainBottom, "terminal");
        m.pin(PanelSlot::MainBottom, "problems");
        m.pin(PanelSlot::MainBottom, "terminal");
        assert_eq!(
            m.state(PanelSlot::MainBottom).pinned,
            vec!["terminal", "problems"]
        );
        assert_eq!(
            m.state(PanelSlot::MainBottom).active.as_deref(),
            Some("terminal")
        );
    }

    #[test]
    fn unpin_reassigns() {
        let mut m = ActivityModel::new();
        m.pin(PanelSlot::MainBottom, "terminal");
        m.pin(PanelSlot::MainBottom, "problems");
        m.unpin(PanelSlot::MainBottom, "problems");
        assert_eq!(
            m.state(PanelSlot::MainBottom).active.as_deref(),
            Some("terminal")
        );
        m.unpin(PanelSlot::MainBottom, "terminal");
        assert_eq!(m.state(PanelSlot::MainBottom).active, None);
    }

    #[test]
    fn cross_slot_move_refuses_dup() {
        let mut m = ActivityModel::new();
        // tree из RightTop в MainBottom
        m.move_activity(PanelSlot::RightTop, "tree", PanelSlot::MainBottom, 0);
        assert!(m.state(PanelSlot::RightTop).pinned.is_empty());
        assert_eq!(
            m.state(PanelSlot::MainBottom).active.as_deref(),
            Some("tree")
        );
        // обратно закрепим и попробуем дубль
        m.pin(PanelSlot::RightTop, "tree");
        m.move_activity(PanelSlot::RightTop, "tree", PanelSlot::MainBottom, 0);
        assert!(m.is_pinned(PanelSlot::RightTop, "tree")); // отказ — остался
    }

    #[test]
    fn same_slot_reorder() {
        let mut m = ActivityModel::new();
        for id in ["terminal", "problems", "search"] {
            m.pin(PanelSlot::MainBottom, id);
        }
        m.move_activity(PanelSlot::MainBottom, "terminal", PanelSlot::MainBottom, 3);
        assert_eq!(
            m.state(PanelSlot::MainBottom).pinned,
            vec!["problems", "search", "terminal"]
        );
    }
}
