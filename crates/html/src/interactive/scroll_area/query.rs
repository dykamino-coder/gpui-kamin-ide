//! Запрос копирует только смещения: layout не меняет живые ручки и extent кадра.

use super::Persistent;
use crate::layout::geometry::GeometryError;
use gpui::{ElementId, GlobalElementId, Window};
use std::collections::HashMap;

pub(crate) fn query_states(
    window: &Window,
    nodes: &HashMap<u64, u64>,
    scope: Option<&GlobalElementId>,
) -> Result<impl FnOnce(&mut Window) + use<>, GeometryError> {
    let mut states = HashMap::new();
    let mut ambiguous = false;
    window.read_element_states::<Persistent>(|id, old| {
        if scope.is_some_and(|scope| !id.starts_with(scope)) {
            return;
        }
        let first = id.iter().position(|part| {
            matches!(part, ElementId::NamedInteger(role, node)
                if role.starts_with("html-") && nodes.contains_key(node))
        });
        let Some(first) = first else { return };
        let state = Persistent::default();
        state.handle.set_offset(old.handle.offset());
        let mut key = id.clone();
        *key = id
            .iter()
            .skip(first)
            .filter_map(|part| match part {
                ElementId::View(_) => None,
                ElementId::NamedInteger(role, node) if role.starts_with("html-") => nodes
                    .get(node)
                    .map(|node| ElementId::NamedInteger(role.clone(), *node)),
                part => Some(part.clone()),
            })
            .collect::<Vec<_>>()
            .into();
        ambiguous |= states.insert(key, state).is_some();
    });
    if ambiguous {
        return Err(GeometryError::AmbiguousScrollScope);
    }
    Ok(move |window: &mut Window| {
        for (id, state) in states {
            window.with_element_state(&id, |_: Option<Persistent>, _| ((), state));
        }
    })
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
    fn geometry_scroll_scope_selects_an_instance_and_rejects_collisions(cx: &mut TestAppContext) {
        let doc = Rc::new(Document::builder("", BROWSER_CSS).build().unwrap());
        let (_, cx) = cx.add_window_view(|_, _| View(doc));
        cx.update(|window, cx| {
            window.with_layout_query(cx, |window, _| {
                let nodes = HashMap::from([(1, 11)]);
                let mut selected = GlobalElementId::default();
                for (name, y) in [("first", -12.5), ("second", -37.25)] {
                    window.with_global_id(name.into(), |scope, window| {
                        if name == "first" {
                            selected = scope.clone();
                        }
                        window.with_global_id(("html-scroll", 1_u64).into(), |id, window| {
                            let state = Persistent::default();
                            state.handle.set_offset(point(px(0.), px(y)));
                            window.with_element_state(id, |_: Option<Persistent>, _| ((), state));
                        });
                    });
                }
                assert!(matches!(
                    query_states(window, &nodes, None),
                    Err(GeometryError::AmbiguousScrollScope)
                ));
                let seed = query_states(window, &nodes, Some(&selected)).unwrap();
                seed(window);
                window.with_global_id(("html-scroll", 11_u64).into(), |id, window| {
                    window.with_element_state(id, |state: Option<Persistent>, _| {
                        let state = state.unwrap();
                        assert_eq!(state.handle.offset(), point(px(0.), px(-12.5)));
                        ((), state)
                    });
                });
            });
        });
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
