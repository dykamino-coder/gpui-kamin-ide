//! Only the child's linked physical axis may inherit parent subgrid tracks.
use crate::computed::Computed;

pub(super) fn linked(parent: &Computed, child: &Computed, parent_rows: bool) -> bool {
    let parallel = parent.vertical.unwrap_or(false) == child.vertical.unwrap_or(false);
    if parent_rows == parallel {
        child.subgrid_rows
    } else {
        child.subgrid_cols
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        computed::{Track, TrackSize},
        dom::{Node, parse},
    };

    fn target(nodes: &[Node]) -> Option<&Computed> {
        for node in nodes {
            if let Node::Element(element) = node {
                if element.attr("id") == Some("target") {
                    return Some(&element.style);
                }
                if let Some(found) = target(&element.children) {
                    return Some(found);
                }
            }
        }
        None
    }

    #[test]
    fn standalone_tracks_survive_parent_track_inheritance() {
        for rows in [false, true] {
            let template = if rows {
                "grid-template-rows: subgrid; grid-template-columns: repeat(4, 1fr)"
            } else {
                "grid-template-columns: subgrid; grid-template-rows: repeat(4, 1fr)"
            };
            let html = format!(
                "<div style='display:grid;grid-template-columns:100px;grid-template-rows:100px'><div id='target' style='display:grid;{template}'></div></div>"
            );
            let nodes = parse(&html, "");
            let style = target(&nodes).unwrap();
            let tracks = if rows {
                &style.grid_tracks
            } else {
                &style.grid_rows
            };
            let tracks = tracks.as_ref().unwrap();
            assert_eq!(tracks.len(), 4, "row_subgrid={rows} tracks={tracks:?}");
            assert!(
                tracks
                    .iter()
                    .all(|t| matches!(t, TrackSize::Single(Track::Fr(v)) if *v == 1.0))
            );
        }
    }

    #[test]
    fn linked_axis_maps_parallel_and_orthogonal_writing_modes() {
        for vertical in [false, true] {
            let parent = Computed {
                vertical: Some(vertical),
                ..Computed::default()
            };
            for orthogonal in [false, true] {
                let child = Computed {
                    vertical: Some(vertical != orthogonal),
                    subgrid_cols: true,
                    ..Computed::default()
                };
                assert_eq!(linked(&parent, &child, false), !orthogonal);
                assert_eq!(linked(&parent, &child, true), orthogonal);
            }
        }
    }
}
