//! Видимость слотов повторяет гейты сборки колонок; warm retention не означает
//! право Chromium рисовать. Registry fallback ограничен одним слотом и tool id.

use std::collections::HashMap;

#[derive(Default)]
pub(super) struct Layout {
    pub sidebar: bool,
    pub main: bool,
    pub main_bottom: bool,
    pub file: bool,
    pub file_bottom: bool,
    pub right: bool,
    pub right_bottom: bool,
    pub has_active: bool,
    pub has_open: bool,
}

impl Layout {
    pub(super) fn renders(&self, slot: &str) -> bool {
        match slot {
            "sidebar" => self.sidebar,
            "main" => self.main && self.has_active,
            "mainBottom" => self.main && self.main_bottom && self.has_open,
            "centralBottom" => self.file && self.file_bottom && self.has_open,
            "rightTop" => self.right && self.has_open,
            "rightBottom" => self.right && self.right_bottom && self.has_open,
            _ => false,
        }
    }
}

#[derive(Default)]
pub(crate) struct Slots(HashMap<&'static str, (String, Vec<String>)>);

pub(super) struct Slot {
    pub name: &'static str,
    pub tool: Option<String>,
    pub views: Option<Vec<String>>,
}

impl Slots {
    pub(super) fn collect(
        &mut self,
        layout: &Layout,
        customize: bool,
        slots: impl IntoIterator<Item = Slot>,
        browser: bool,
        customize_view: Option<String>,
    ) -> (Vec<String>, Vec<String>) {
        let mut visible = Vec::new();
        let mut retained = Vec::new();
        for slot in slots {
            let views = self.resolve(slot.name, slot.tool.as_deref(), slot.views);
            if !layout.renders(slot.name) {
                continue;
            }
            if customize {
                retained.extend(views);
            } else {
                visible.extend(views);
            }
        }
        if browser && layout.file && layout.has_open {
            if customize {
                retained.push("browser".to_string());
            } else {
                visible.push("browser".to_string());
            }
        }
        if customize && let Some(active) = customize_view {
            visible.push(active);
        }
        (visible, retained)
    }

    pub(super) fn resolve(
        &mut self,
        slot: &'static str,
        tool: Option<&str>,
        snapshot: Option<Vec<String>>,
    ) -> Vec<String> {
        let Some(tool) = tool else {
            self.0.remove(slot);
            return Vec::new();
        };
        let cached = self.0.entry(slot).or_default();
        if cached.0 != tool {
            *cached = (tool.to_string(), Vec::new());
        }
        if let Some(views) = snapshot {
            cached.1 = views;
        }
        cached.1.clone()
    }
}

#[cfg(test)]
#[path = "webview_visibility_tests.rs"]
mod tests;
