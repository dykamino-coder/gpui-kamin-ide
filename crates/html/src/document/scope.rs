//! РЎР±РѕСЂРєР° РєР°РґСЂР° РґРѕРєСѓРјРµРЅС‚Р° РґР»СЏ С…РѕСЃС‚Р°: РѕРїС†РёРё СЃР±РѕСЂРєРё СЃ СЌС‚РёРј РґРѕРєСѓРјРµРЅС‚РѕРј, СЃР±РѕСЂРєР°
//! РєР°РґСЂР° Рё Р±Р»РѕРєРѕРІ, РѕР±С‘СЂС‚РєР° `Scoped` РґР»СЏ С„Р°Р· GPUI, Р·РЅР°С‡РµРЅРёСЏ Рё РїСЂР°РІРёР»Р° `@page`
//! РґР»СЏ Р»РёСЃС‚РѕРІ.

use super::Document;
use super::state::Scoped;
use crate::values::ValueCx;

impl Document {
    /// Р”РѕРєСѓРјРµРЅС‚ Р·Р°РІРµСЂС€РёР» СЂР°Р±РѕС‚Сѓ С‚РµРєСѓС‰РµРіРѕ РєР°РґСЂР°. Р§РёС‚Р°С‚СЊ РїРѕСЃР»Рµ Р·Р°РІРµСЂС€РµРЅРёСЏ РєР°РґСЂР°
    /// С…РѕСЃС‚РѕРј, РІРєР»СЋС‡Р°СЏ deferred paint; Р°РЅРёРјР°С†РёРѕРЅРЅРѕРµ РІСЂРµРјСЏ С…РѕСЃС‚ С„РёРєСЃРёСЂСѓРµС‚ СЃР°Рј.
    /// Р­С‚Рѕ РЅРµ РїРѕРґС‚РІРµСЂР¶РґРµРЅРёРµ РїСЂРµР·РµРЅС‚Р°С†РёРё GPU Рё РЅРµ СЂР°Р·СЂРµС€РµРЅРёРµ РїСЂРѕРїСѓСЃС‚РёС‚СЊ РµС‘.
    pub fn frame_ready(&self) -> bool {
        self.state.values.frame_ready()
    }

    /// РќР°С‡Р°С‚СЊ РєР°РґСЂ Рё РІРµСЂРЅСѓС‚СЊ РѕРїС†РёРё СЃР±РѕСЂРєРё СЃ СЌС‚РёРј РґРѕРєСѓРјРµРЅС‚РѕРј: СЃ РЅРёРјРё С…РѕСЃС‚
    /// РІС‹Р·С‹РІР°РµС‚ `render::*` РїРѕ `nodes()` (СЃС‚РѕРїРєР° СЃС‚СЂР°РЅРёС†) СЃР°Рј.
    pub fn frame_opts(&self, opts: &crate::RenderOpts) -> crate::RenderOpts {
        frame_opts(&self.state, &self.nodes, opts)
    }

    /// Р—РЅР°С‡РµРЅРёСЏ РєР°РґСЂР° РґРѕРєСѓРјРµРЅС‚Р°: РёРјРё С…РѕСЃС‚ СЂР°Р·Р±РёСЂР°РµС‚ РѕР±СЉСЏРІР»РµРЅРёСЏ Р»РёСЃС‚Р°
    /// (`Len::parse` Рё `Color::parse` РїРѕ РїСЂР°РІРёР»Р°Рј `@page`).
    pub fn values(&self) -> ValueCx<'_> {
        self.state.frame_values()
    }

    /// РћР±РµСЂРЅСѓС‚СЊ СЌР»РµРјРµРЅС‚, СЃРѕР±СЂР°РЅРЅС‹Р№ РїРѕ `frame_opts`: РґРѕРєСѓРјРµРЅС‚ РґРѕР¶РёРІР°РµС‚ РґРѕ
    /// РєСЂР°СЃРєРё Рё РѕС‚РјРµС‡Р°РµС‚СЃСЏ РїРѕРєСЂР°С€РµРЅРЅС‹Рј.
    pub fn scoped(&self, element: gpui::AnyElement) -> gpui::AnyElement {
        Scoped::wrap(&self.state, element)
    }

    /// РЎРѕР±СЂР°С‚СЊ РєР°РґСЂ РІСЃРµРіРѕ РґРѕРєСѓРјРµРЅС‚Р° (`render::render`).
    pub fn render(&self, opts: &crate::RenderOpts) -> Vec<gpui::AnyElement> {
        render(&self.state, &self.nodes, opts)
    }

    /// РЎРѕР±СЂР°С‚СЊ РѕРґРёРЅ Р±Р»РѕРє РІРµСЂС…РЅРµРіРѕ СѓСЂРѕРІРЅСЏ (`render::render_block`).
    pub fn render_block(&self, index: usize, opts: &crate::RenderOpts) -> Option<gpui::AnyElement> {
        crate::render::render_block(&self.nodes, index, &self.frame_opts(opts))
            .map(|e| self.scoped(e))
    }

    /// РџСЂР°РІРёР»Р° `@page` РґРѕРєСѓРјРµРЅС‚Р° вЂ” РїРѕ РЅРёРј С…РѕСЃС‚ СЃС‚СЂРѕРёС‚ Р»РёСЃС‚С‹
    /// (`css::page_decls_in`, `css::page_margins_in`).
    pub fn page_rules(&self) -> Vec<crate::style::css::PageRule> {
        self.state.page_rules()
    }
}

/// РќР°С‡Р°С‚СЊ РєР°РґСЂ РґРѕРєСѓРјРµРЅС‚Р° `state` СЃ РґРµСЂРµРІРѕРј `nodes` (`Document::frame_opts`).
pub(super) fn frame_opts(
    state: &std::rc::Rc<super::state::DocState>,
    nodes: &[crate::dom::Node],
    opts: &crate::RenderOpts,
) -> crate::RenderOpts {
    state.begin_frame();
    crate::values::image_values::refresh(state.frame_values());
    let mut opts = opts.clone();
    opts.cx.clamp.input = None;
    opts.cx.values.inert_clear = false;
    opts.cx.contracts.document_root = Some((nodes.as_ptr() as usize, nodes.len()));
    opts.cx.contracts.paragraph_frame_started = true;
    opts.cx.values.doc = crate::layout::cx::BuildDoc(state.clone());
    opts
}

/// РЎРѕР±СЂР°С‚СЊ РєР°РґСЂ РІСЃРµРіРѕ РґРѕРєСѓРјРµРЅС‚Р° (`Document::render`).
pub(super) fn render(
    state: &std::rc::Rc<super::state::DocState>,
    nodes: &[crate::dom::Node],
    opts: &crate::RenderOpts,
) -> Vec<gpui::AnyElement> {
    crate::render::render(nodes, &frame_opts(state, nodes, opts))
        .into_iter()
        .map(|e| Scoped::wrap(state, e))
        .collect()
}
