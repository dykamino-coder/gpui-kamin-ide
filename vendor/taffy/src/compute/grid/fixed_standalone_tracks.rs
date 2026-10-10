//! Fixed standalone tracks constrain flattened subgrid items even when container size is auto.
use super::*;

#[cfg(test)]
pub(super) fn standalone_tracks<Tree: crate::LayoutPartialTree>(
    tree: &Tree,
    style: &impl crate::GridContainerStyle,
    columns: bool,
    basis: Size<Option<f32>>,
) -> Option<(Vec<f32>, f32)> {
    standalone_tracks_with_known(tree, style, columns, basis, Size::NONE)
}

pub(super) fn standalone_tracks_with_known<Tree: crate::LayoutPartialTree>(
    tree: &Tree,
    style: &impl crate::GridContainerStyle,
    columns: bool,
    basis: Size<Option<f32>>,
    known_content: Size<Option<f32>>,
) -> Option<(Vec<f32>, f32)> {
    let size = style
        .size()
        .maybe_resolve(basis, |val, b| tree.calc(val, b));
    let mut size = if columns { size.width } else { size.height };
    if style.box_sizing() == crate::BoxSizing::BorderBox {
        let padding = style
            .padding()
            .map(|p| p.resolve_or_zero(basis.width, |val, b| tree.calc(val, b)));
        let border = style
            .border()
            .map(|p| p.resolve_or_zero(basis.width, |val, b| tree.calc(val, b)));
        let edges = if columns {
            padding.left + padding.right + border.left + border.right
        } else {
            padding.top + padding.bottom + border.top + border.bottom
        };
        size = size.map(|value| value - edges);
    }
    let known = if columns {
        known_content.width
    } else {
        known_content.height
    };
    let size = known.or(size).map(|value| value.max(0.0));
    let gap = if columns {
        style.gap().width
    } else {
        style.gap().height
    };
    let gap = gap.resolve_or_zero(size, |val, b| tree.calc(val, b));
    let template = if columns {
        style.grid_template_columns()
    } else {
        style.grid_template_rows()
    }?;
    // (С‚РѕС‡РєРё, РґРѕР»СЏ)
    let mut tracks: Vec<(Option<f32>, f32)> = Vec::new();
    let mut push = |t: crate::TrackSizingFunction| -> Option<()> {
        let max = t.max_sizing_function();
        let min = t.min_sizing_function();
        if max.is_fr() {
            // Flexible tracks require a definite standalone-axis size.
            size?;
            tracks.push((None, max.into_raw().value()));
            return Some(());
        }
        let hi = max.definite_value(size, |val, b| tree.calc(val, b))?;
        let lo = min.definite_value(size, |val, b| tree.calc(val, b))?;
        ((hi - lo).abs() < 0.001).then(|| tracks.push((Some(hi), 0.0)))
    };
    for component in template {
        match component {
            crate::GenericGridTemplateComponent::Single(t) => push(t)?,
            crate::GenericGridTemplateComponent::Repeat(repeat) => match repeat.count() {
                crate::RepetitionCount::Count(n) => {
                    for _ in 0..n {
                        for t in repeat.tracks() {
                            push(t)?;
                        }
                    }
                }
                _ => return None,
            },
        }
    }
    if tracks.is_empty() {
        return None;
    }
    let fixed: f32 = tracks.iter().filter_map(|t| t.0).sum();
    let flex: f32 = tracks.iter().map(|t| t.1).sum();
    let leftover = size
        .map(|value| (value - fixed - gap * (tracks.len() as f32 - 1.0)).max(0.0))
        .unwrap_or(0.0);
    let per = leftover / flex.max(1.0);
    Some((
        tracks.iter().map(|t| t.0.unwrap_or(t.1 * per)).collect(),
        gap,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style_helpers::*;
    use crate::{BoxSizing, Display, Style, TaffyTree};

    fn resolve(style: Style, columns: bool) -> Option<(Vec<f32>, f32)> {
        let mut tree: TaffyTree<()> = TaffyTree::new();
        let view = tree.as_layout_tree();
        standalone_tracks(&view, &style, columns, Size::NONE)
    }

    #[test]
    fn fixed_standalone_tracks_do_not_require_container_size() {
        for columns in [false, true] {
            let mut style = Style {
                display: Display::Grid,
                ..Style::DEFAULT
            };
            if columns {
                style.grid_template_columns = vec![length(25.0), length(35.0)];
            } else {
                style.grid_template_rows = vec![length(25.0), length(35.0)];
            }
            style.gap = Size {
                width: length(10.0),
                height: length(10.0),
            };
            assert_eq!(resolve(style, columns), Some((vec![25.0, 35.0], 10.0)));
        }
    }

    #[test]
    fn fractional_and_percentage_tracks_still_require_their_basis() {
        let mut fractions = Style {
            display: Display::Grid,
            grid_template_rows: vec![fr(1.0), fr(3.0)],
            ..Style::DEFAULT
        };
        assert_eq!(resolve(fractions.clone(), false), None);
        fractions.size.height = length(100.0);
        assert_eq!(resolve(fractions, false), Some((vec![25.0, 75.0], 0.0)));
        let mut percentage = Style {
            display: Display::Grid,
            grid_template_rows: vec![percent(0.5)],
            ..Style::DEFAULT
        };
        assert_eq!(resolve(percentage.clone(), false), None);
        percentage.size.height = length(100.0);
        assert_eq!(resolve(percentage, false), Some((vec![50.0], 0.0)));
    }

    #[test]
    fn definite_fractional_tracks_deduct_border_padding_and_gap() {
        let style = Style {
            display: Display::Grid,
            box_sizing: BoxSizing::BorderBox,
            size: Size::from_lengths(100.0, 100.0),
            padding: Rect {
                left: length(5.0),
                right: length(5.0),
                top: length(5.0),
                bottom: length(5.0),
            },
            border: Rect {
                left: length(2.0),
                right: length(2.0),
                top: length(2.0),
                bottom: length(2.0),
            },
            gap: Size {
                width: length(6.0),
                height: length(6.0),
            },
            grid_template_rows: vec![fr(1.0), fr(1.0)],
            ..Style::DEFAULT
        };
        assert_eq!(resolve(style, false), Some((vec![40.0, 40.0], 6.0)));
    }
}
