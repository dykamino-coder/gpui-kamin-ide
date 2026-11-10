//! Синхронный CSSOM View: временная раскладка, не состояние следующего кадра.

use super::{
    DocState, Document, DocumentFailure, ElementGeometry, GeometryError, LineHeight, Rect,
};
use crate::{RenderOpts, layout::geometry::Collector};
use gpui::{App, AvailableSpace, IntoElement, ParentElement, Styled, Window, div, point, px, size};
use markup5ever_rcdom::NodeData;
use std::{collections::HashMap, rc::Rc};

struct QueryState(Rc<DocState>);

impl Drop for QueryState {
    fn drop(&mut self) {
        self.0.values.release_document();
    }
}

#[cfg(test)]
mod tests;

impl Document {
    /// Снимок всех исходных элементов в DOM preorder, включая элементы без коробок.
    /// Запрос синхронно завершает отдельную раскладку; не публикует кадр и не
    /// сохраняет геометрию. `opts` задаёт viewport и шрифты; смещения областей
    /// прокрутки берутся из текущего состояния элементов этого окна.
    pub fn element_geometry(
        &self,
        opts: &RenderOpts,
        window: &mut Window,
        cx: &mut App,
    ) -> Result<Vec<ElementGeometry>, GeometryError> {
        self.element_geometry_in_scope(None, opts, window, cx)
    }

    /// `scope` — префикс GlobalElementId, под которым host рисует этот DOM.
    /// Он различает несколько документов или экземпляров документа в окне,
    /// не добавляя идентичность или сохранённую геометрию в обычный кадр.
    /// Без scope неоднозначное состояние прокрутки возвращает ошибку.
    pub fn element_geometry_in_scope(
        &self,
        scope: Option<&gpui::GlobalElementId>,
        opts: &RenderOpts,
        window: &mut Window,
        cx: &mut App,
    ) -> Result<Vec<ElementGeometry>, GeometryError> {
        let budget = self.budget();
        let revision =
            crate::budget::DocumentBudget::new(budget.limits().clone(), budget.governor().clone())
                .map_err(|e| DocumentFailure::new(super::FailureReason::Budget(e)))?
                .revision();
        let Some(input) = self.state.values.query_input(revision.clone()) else {
            return Ok(Vec::new());
        };
        let mut media: crate::style::css::Media = self.state.media();
        media.width = opts.viewport.0;
        media.height = opts.viewport.1;
        let query_state = QueryState(DocState::new(media));
        let (state, nodes, _) = super::prepare::parse_document_for::<true>(
            &input,
            &self.theme_css,
            self.base_url.as_ref(),
            query_state.0.clone(),
            &self.font_context(),
            revision,
            #[cfg(feature = "engine-bench")]
            &mut super::BuildTimings::default(),
        )?;
        state
            .values()
            .doc
            .images
            .borrow_mut()
            .inherit_svg_fonts(&self.state.values().doc.images.borrow());
        let mut opts = super::scope::frame_opts(&state, &nodes, opts);
        opts.cx.contracts.build = crate::render::geometry::builder();
        let sink: Rc<Collector> = Rc::new(Collector::for_nodes(&nodes));
        let ids: HashMap<_, _> = state
            .values
            .traversal
            .elements()
            .iter()
            .enumerate()
            .map(|(i, address)| (*address, i as u64 + 1))
            .collect();
        let node_map = self
            .state
            .values
            .traversal
            .elements()
            .iter()
            .enumerate()
            .filter_map(|(i, address)| ids.get(address).map(|id| (i as u64 + 1, *id)))
            .collect();
        let scroll_states =
            crate::interactive::scroll_area::query_states(window, &node_map, scope)?;
        window.with_layout_query(cx, |window, cx| {
            scroll_states(window);
            window.with_render_context(Some(sink.clone()), |window| {
                let mut root = div()
                    .block()
                    .w(px(opts.viewport.0))
                    .h(px(opts.viewport.1))
                    .children(crate::render::render(&nodes, &opts))
                    .into_any_element();
                root.prepaint_as_root(
                    point(px(0.), px(0.)),
                    size(
                        AvailableSpace::Definite(px(opts.viewport.0)),
                        AvailableSpace::Definite(px(opts.viewport.1)),
                    ),
                    window,
                    cx,
                );
                window.finish_layout_query(cx);
            });
        });
        sink.finish_captions();
        let mut boxes = sink.0.borrow_mut();
        let mut out = Vec::new();
        let mut pending = vec![(input.dom.document.clone(), Vec::new())];
        while let Some((node, path)) = pending.pop() {
            if let NodeData::Element { name, attrs, .. } = &node.data {
                let id = ids.get(&(Rc::as_ptr(&node) as usize)).copied().unwrap_or(0);
                let b = boxes.remove(&id).unwrap_or_default();
                let attr = |key: &str| {
                    attrs
                        .borrow()
                        .iter()
                        .find(|a| a.name.local.as_ref() == key)
                        .map(|a| a.value.to_string())
                        .unwrap_or_default()
                };
                let (font_size, line_height) = sink
                    .1
                    .get(&id)
                    .map(|style| {
                        crate::layout::geometry::font(style, opts.base_size(), state.values())
                    })
                    .or(b.font)
                    .unwrap_or((16., LineHeight::Normal));
                out.push(ElementGeometry {
                    path: path.clone(),
                    tag: name.local.to_string(),
                    id: attr("id"),
                    class: attr("class"),
                    border_box: Rect::bounding(&b.border),
                    client_rects: b.border,
                    padding_boxes: b.padding,
                    content_boxes: b.content,
                    font_size,
                    line_height,
                });
            }
            for (i, child) in node.children.borrow().iter().enumerate().rev() {
                let mut path = path.clone();
                path.push(i);
                pending.push((child.clone(), path));
            }
        }
        Ok(out)
    }

    /// CSSOM View §6: список border boxes фрагментов в порядке содержимого.
    pub fn get_client_rects(
        &self,
        path: &[usize],
        opts: &RenderOpts,
        window: &mut Window,
        cx: &mut App,
    ) -> Result<Vec<Rect>, GeometryError> {
        Ok(self
            .element_geometry(opts, window, cx)?
            .into_iter()
            .find(|e| e.path == path)
            .map(|e| e.client_rects)
            .unwrap_or_default())
    }

    /// CSSOM View §6: объединение непустых прямоугольников getClientRects.
    pub fn get_bounding_client_rect(
        &self,
        path: &[usize],
        opts: &RenderOpts,
        window: &mut Window,
        cx: &mut App,
    ) -> Result<Rect, GeometryError> {
        self.get_client_rects(path, opts, window, cx)
            .map(|r| Rect::bounding(&r))
    }
}
