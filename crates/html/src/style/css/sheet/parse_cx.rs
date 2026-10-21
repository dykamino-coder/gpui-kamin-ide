//! Состояние разбора таблицы стилей, явно передаваемое вниз по парсеру.
//!
//! Кто чем владеет (Blink: `CSSParserContext` + пространства имён `StyleSheetContents`,
//! `CascadeLayerMap` движка стилей документа):
//! - [`LayerRegistry`] принадлежит разбору ДОКУМЕНТА: все таблицы документа (агента, приложения,
//!   `<style>`, теневые деревья) делят один порядок слоёв (css-cascade-5 §6.4.3,
//!   «in the order they are first declared»).
//! - [`SheetCx`] принадлежит ОДНОЙ таблице: её пролог `@namespace`, алиасы `@custom-media` и слой,
//!   внутри которого сейчас парсер.
//! - [`SelectorCx`] — то, что разбору селектора нужно от таблицы; селектор вне таблицы берёт
//!   значение по умолчанию (без проверки префикса и без пространства имён по умолчанию).

use std::collections::{HashMap, HashSet};
use std::rc::Rc;

/// Cascade layers of a document: full name → index path.
#[derive(Default)]
pub struct LayerRegistry {
    paths: HashMap<String, Vec<u32>>,
    /// Next child index of each parent (key: the parent's full name).
    next: HashMap<String, u32>,
    anon: u32,
}

impl LayerRegistry {
    /// Path of layer `name` (possibly dotted) inside `now`; registers unknown
    /// segments. An empty name is an anonymous layer, always a new one.
    pub(super) fn enter_path(
        &mut self,
        now: &(String, Vec<u32>),
        name: &str,
    ) -> (String, Vec<u32>) {
        let (mut full, mut path) = now.clone();
        let segs: Vec<String> = if name.trim().is_empty() {
            let n = self.anon;
            self.anon += 1;
            vec![format!("\u{1}anon{n}")]
        } else {
            name.split('.').map(|s| s.trim().to_string()).collect()
        };
        for seg in segs {
            let child = if full.is_empty() {
                seg
            } else {
                format!("{full}.{seg}")
            };
            path = match self.paths.get(&child) {
                Some(p) => p.clone(),
                None => {
                    let e = self.next.entry(full.clone()).or_insert(0);
                    let idx = *e;
                    *e += 1;
                    let mut p = path.clone();
                    p.push(idx);
                    self.paths.insert(child.clone(), p.clone());
                    p
                }
            };
            full = child;
        }
        (full, path)
    }
}

/// What a selector parse needs from its sheet.
#[derive(Clone, Default)]
pub(crate) struct SelectorCx {
    /// Prefixes of valid `@namespace` rules; `None` — not parsing a sheet,
    /// prefixes are not checked.
    pub(crate) prefixes: Option<Rc<HashSet<String>>>,
    /// Default namespace (`@namespace url(…)` without a prefix): `Some(true)`
    /// — XHTML, `Some(false)` — another namespace.
    pub(crate) default_ns: Option<bool>,
}

impl SelectorCx {
    /// A context with only a default namespace (`:is()` arguments re-parsed
    /// at match time carry their sheet's default namespace as a mark).
    pub(crate) fn with_default_ns(default_ns: Option<bool>) -> Self {
        Self {
            prefixes: None,
            default_ns,
        }
    }

    /// Is the prefix declared? Empty (`|div`) and `*` always are.
    pub(crate) fn ns_declared(&self, ns: &str) -> bool {
        ns.is_empty() || ns == "*" || self.prefixes.as_ref().is_none_or(|s| s.contains(ns))
    }
}

/// `@custom-media` aliases visible to a media query evaluation, with the
/// substitution depth that stops cyclic aliases (mediaqueries-5 §5.1).
#[derive(Clone, Copy)]
pub(crate) struct CustomMedia<'a> {
    pub(crate) aliases: Option<&'a HashMap<String, String>>,
    pub(crate) depth: u8,
}

impl CustomMedia<'_> {
    /// Outside a stylesheet there are no aliases: `(--name)` is false.
    pub(crate) const NONE: CustomMedia<'static> = CustomMedia {
        aliases: None,
        depth: 0,
    };
}

/// One stylesheet being parsed.
pub(crate) struct SheetCx<'a> {
    /// The document whose at-rules (`@page`, `@position-try`, `@property`,
    /// `@counter-style`) the sheet registers.
    pub(super) doc: &'a crate::document::state::DocState,
    pub(super) layers: &'a mut LayerRegistry,
    /// Layer of the current parse position: full name and path.
    pub(super) layer_now: (String, Vec<u32>),
    pub(crate) selectors: SelectorCx,
    pub(crate) custom_media: HashMap<String, String>,
}

impl SheetCx<'_> {
    pub(crate) fn custom(&self) -> CustomMedia<'_> {
        CustomMedia {
            aliases: Some(&self.custom_media),
            depth: 0,
        }
    }

    /// Path for the rules at the CURRENT parse position (with the trailing
    /// «own rules» entry).
    pub(crate) fn layer_of_rules(&self) -> Vec<u32> {
        let mut p = self.layer_now.1.clone();
        p.push(u32::MAX);
        p
    }

    pub(super) fn layer_enter_path(&mut self, name: &str) -> (String, Vec<u32>) {
        self.layers.enter_path(&self.layer_now, name)
    }

    /// Enter the `@layer` block `name`; returns the layer to restore.
    pub(super) fn enter_layer(&mut self, name: &str) -> (String, Vec<u32>) {
        let entered = self.layer_enter_path(name);
        std::mem::replace(&mut self.layer_now, entered)
    }
}
