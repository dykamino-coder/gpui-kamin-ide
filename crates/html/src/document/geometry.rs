//! Синхронный CSSOM View: временная раскладка, не состояние следующего кадра.

use super::{
    DocState, Document, DocumentFailure, ElementGeometry, GeometryError, GeometryLayout,
    LineHeight, Rect,
};
use crate::{RenderOpts, layout::geometry::Collector};
use gpui::{App, AvailableSpace, IntoElement, ParentElement, Styled, Window, div, point, px, size};
use markup5ever_rcdom::{Handle, NodeData};
use std::{collections::HashMap, rc::Rc};

struct QueryState(Rc<DocState>);

impl Drop for QueryState {
    fn drop(&mut self) {
        self.0.values.release_document();
    }
}

mod read;
#[cfg(test)]
mod tests;
pub use read::GeometryRead;

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
        self.element_geometry_with_layout(GeometryLayout::Continuous, scope, opts, window, cx)
    }

    /// Запрос в непрерывной или постраничной раскладке с геометрией листов host.
    pub fn element_geometry_with_layout(
        &self,
        layout: GeometryLayout,
        scope: Option<&gpui::GlobalElementId>,
        opts: &RenderOpts,
        window: &mut Window,
        cx: &mut App,
    ) -> Result<Vec<ElementGeometry>, GeometryError> {
        self.with_geometry(layout, scope, opts, window, cx, |read| read.elements())
    }

    /// Flush this view once. Read any number of element rectangles before the
    /// next mutation, viewport change or scripted scroll. The borrow ends with
    /// the callback, so geometry from a previous layout cannot be used again.
    pub fn with_geometry<R>(
        &self,
        layout: GeometryLayout,
        scope: Option<&gpui::GlobalElementId>,
        opts: &RenderOpts,
        window: &mut Window,
        cx: &mut App,
        read: impl FnOnce(GeometryRead<'_>) -> R,
    ) -> Result<R, GeometryError> {
        let budget = self.budget();
        let revision =
            crate::budget::DocumentBudget::new(budget.limits().clone(), budget.governor().clone())
                .map_err(|e| DocumentFailure::new(super::FailureReason::Budget(e)))?
                .revision();
        let Some(input) = self.state.values.query_input(revision.clone()) else {
            return Ok(read(GeometryRead(None)));
        };
        let mut media: crate::style::css::Media = self.state.media();
        media.width = opts.viewport.0;
        media.height = opts.viewport.1;
        let query_state = QueryState(DocState::new(media));
        let (state, mut nodes, _) = super::prepare::parse_document_for::<true>(
            &input,
            &self.theme_css,
            self.base_url.as_ref(),
            query_state.0.clone(),
            &self.font_context(),
            revision,
            None,
            #[cfg(feature = "engine-bench")]
            &mut super::BuildTimings::default(),
        )?;
        let mut opts = super::scope::frame_opts(&state, &nodes, opts);
        opts.cx.contracts.build = crate::render::geometry::builder();
        crate::text::metrics::set_doc_family(&opts.text.font_family, opts.cx.vcx());
        crate::style::cascade::materialize::materialize(
            &nodes,
            opts.root_style().style_rc(),
            opts.cx.vcx(),
        );
        let sink: Rc<Collector> = Rc::new(Collector::for_nodes(
            &nodes,
            self.state.values().doc.images.borrow().svg_fonts(),
        ));
        // Стили скрытого DOM уже сняты; обычный renderer получает дерево без
        // display:none, как после обычного convert. Сам DOM не меняется.
        let mut pending = vec![&mut nodes];
        while let Some(children) = pending.pop() {
            children.retain(|node| !matches!(node, crate::dom::Node::Element(e)
                if e.style.computed().reset_box_layout.display == Some(crate::style::computed::Display::None)
                    && e.style.cascaded().reset_context.col_role.is_none()));
            for node in children {
                if let crate::dom::Node::Element(e) = node {
                    pending.push(&mut *e.children);
                }
            }
        }
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
        #[cfg(test)]
        self.state
            .geometry_flushes
            .set(self.state.geometry_flushes.get() + 1);
        window.with_layout_query(cx, |window, cx| {
            scroll_states(window);
            window.with_render_context(Some(sink.clone()), |window| {
                let children = match layout {
                    GeometryLayout::Continuous => crate::render::render(&nodes, &opts),
                    GeometryLayout::Paged { pages, margins } => {
                        vec![crate::layout::page::paged::render_paged_for::<true>(
                            &nodes, &opts, pages, margins, None,
                        )]
                    }
                };
                let mut root = div()
                    .block()
                    .w(px(opts.viewport.0))
                    .h(px(opts.viewport.1))
                    .children(children)
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
        Ok(read(GeometryRead(Some(read::Fragments {
            root: &input.dom.document,
            ids: &ids,
            sink: &sink,
            base: opts.base_size(),
            state: &state,
        }))))
    }

    /// Read one element in continuous layout. Batch adjacent CSSOM reads with
    /// `with_geometry` when running a script turn.
    pub fn get_client_rects(
        &self,
        path: &[usize],
        opts: &RenderOpts,
        window: &mut Window,
        cx: &mut App,
    ) -> Result<Vec<Rect>, GeometryError> {
        self.with_geometry(GeometryLayout::Continuous, None, opts, window, cx, |read| {
            read.get_client_rects(path)
        })
    }

    /// Read one bounding rectangle without exporting every source element.
    pub fn get_bounding_client_rect(
        &self,
        path: &[usize],
        opts: &RenderOpts,
        window: &mut Window,
        cx: &mut App,
    ) -> Result<Rect, GeometryError> {
        self.with_geometry(GeometryLayout::Continuous, None, opts, window, cx, |read| {
            read.get_bounding_client_rect(path)
        })
    }
}
