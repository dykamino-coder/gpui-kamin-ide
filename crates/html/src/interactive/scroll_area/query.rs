//! Запрос копирует только смещения: layout не меняет живые ручки и extent кадра.

use super::Persistent;
use gpui::{ElementId, Window};
use std::collections::HashMap;

pub(crate) fn query_states(
    window: &Window,
    nodes: &HashMap<u64, u64>,
) -> impl FnOnce(&mut Window) + use<> {
    let mut states = Vec::new();
    window.read_element_states::<Persistent>(|id, old| {
        let state = Persistent::default();
        state.handle.set_offset(old.handle.offset());
        let mut key = id.clone();
        *key = id
            .iter()
            .skip_while(|part| {
                !matches!(part, ElementId::NamedInteger(role, node)
                if role.starts_with("html-") && nodes.contains_key(node))
            })
            .filter_map(|part| match part {
                ElementId::View(_) => None,
                ElementId::NamedInteger(role, node) if role.starts_with("html-") => nodes
                    .get(node)
                    .map(|node| ElementId::NamedInteger(role.clone(), *node)),
                part => Some(part.clone()),
            })
            .collect::<Vec<_>>()
            .into();
        states.push((key, state));
    });
    move |window| {
        for (id, state) in states {
            window.with_element_state(&id, |_: Option<Persistent>, _| ((), state));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BROWSER_CSS, Document, RenderOpts};
    use gpui::{
        Context, IntoElement, ParentElement, Render, Styled, TestAppContext, div, point, px,
    };
    use std::rc::Rc;

    struct View(Rc<Document>);

    fn options() -> RenderOpts {
        RenderOpts {
            text: gpui::TextStyle::default(),
            viewport: (300., 300.),
            normal_line_height: 1.31,
            cx: Default::default(),
            frag: Default::default(),
        }
    }

    impl Render for View {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div().w(px(300.)).children(self.0.render(&options()))
        }
    }

    #[gpui::test]
    fn query_preserves_live_scroll_offsets_and_measures_unclipped_boxes(cx: &mut TestAppContext) {
        let doc = Rc::new(Document::builder(
            "<style>body{margin:0}</style><div style='width:100px;height:40px;overflow:scroll'><div id='child' style='width:200px;height:200px'></div></div>",
            BROWSER_CSS,
        ).build().unwrap());
        let (view, cx) = cx.add_window_view(|_, _| View(doc));
        cx.update(|window, cx| {
            window.refresh();
            window.draw(cx).clear(cx);
            let doc = view.read(cx).0.clone();
            let before = doc.element_geometry(&options(), window, cx).unwrap();
            let mut handles = Vec::new();
            window.read_element_states::<Persistent>(|_, state| {
                if state.handle.bounds().size.width < px(200.) {
                    handles.push(state.handle.clone());
                }
            });
            assert_eq!(handles.len(), 1, "expected the nested scroll area");
            let offset = point(px(-7.25), px(-12.5));
            for handle in &handles {
                handle.set_offset(offset);
            }
            let after = doc.element_geometry(&options(), window, cx).unwrap();
            let rect = |all: &[crate::ElementGeometry]| {
                all.iter().find(|e| e.id == "child").unwrap().border_box
            };
            assert_eq!(rect(&after).x - rect(&before).x, -7.25);
            assert_eq!(rect(&after).y - rect(&before).y, -12.5);
            assert_eq!(rect(&after).width, 200.);
            assert_eq!(rect(&after).height, 200.);
            assert!(handles.iter().all(|handle| handle.offset() == offset));
        });
    }
}
