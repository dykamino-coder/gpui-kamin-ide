//! Local baseline contracts expressed on native y and independent x baselines.

use crate::style::Style;
use crate::tree::{Baselines, LayoutOutput};

pub(super) fn adapt(mut output: LayoutOutput, style: &Style) -> LayoutOutput {
    if let (None, Some((offset, from_right))) = (output.baselines_x.first, style.baseline_x_hint) {
        output.baselines_x.first = Some(if from_right {
            output.size.width - offset
        } else {
            offset
        });
    }
    // A table (or its wrapper box) contributes no baseline to an enclosing
    // inline-block, whatever block wrappers sit in between.
    if style.no_inline_block_baseline {
        output.inline_block_last_y = Some(None);
    }
    if style.baseline_from_last {
        output.baselines.first = output.inline_block_last_y();
        output.baselines_x.first = output.last_or_first_x();
    }
    // General scroll-container baselines use native synthesis/clamping. The
    // inline-block boundary instead exports its margin-box bottom when its
    // overflow is non-visible (CSS 2.1 10.8.1); callers synthesize that fallback.
    // Paint containment uses Clip and does not suppress the text baseline.
    if style.contain.suppresses_baseline()
        || style.baseline_unavailable
        || (style.baseline_from_last && style.overflow.y.is_scroll_container())
    {
        output.baselines = Baselines::NONE;
        output.baselines_x = Baselines::NONE;
        output.inline_block_last_y = None;
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Size;
    use crate::style::Contain;

    fn measured() -> LayoutOutput {
        let mut result = LayoutOutput::from_outer_size(Size {
            width: 40.0,
            height: 50.0,
        });
        result.baselines = Baselines {
            first: Some(10.0),
            last: Some(30.0),
        };
        result
    }

    #[test]
    fn right_edge_hint_does_not_overwrite_measured_x_or_y() {
        let style = Style {
            baseline_x_hint: Some((4.0, true)),
            ..Style::DEFAULT
        };
        let result = adapt(measured(), &style);
        assert_eq!(result.baselines_x.first, Some(36.0));
        assert_eq!(result.baselines, measured().baselines);
        let mut existing = measured();
        existing.baselines_x = Baselines {
            first: Some(5.0),
            last: Some(8.0),
        };
        let expected = existing.baselines_x;
        assert_eq!(adapt(existing, &style).baselines_x, expected);
    }

    #[test]
    fn inline_block_last_baseline_policy_preserves_last() {
        let style = Style {
            baseline_from_last: true,
            ..Style::DEFAULT
        };
        let mut content = measured();
        // Content-order last is the leftmost line in a vertical-rl inline block.
        content.baselines_x = Baselines {
            first: Some(32.0),
            last: Some(8.0),
        };
        let result = adapt(content, &style);
        assert_eq!(
            result.baselines,
            Baselines {
                first: Some(30.0),
                last: Some(30.0)
            }
        );
        assert_eq!(
            result.baselines_x,
            Baselines {
                first: Some(8.0),
                last: Some(8.0)
            }
        );
    }

    #[test]
    fn layout_containment_suppresses_custom_output_and_x_hints() {
        let style = Style {
            contain: Contain::LAYOUT,
            baseline_x_hint: Some((4.0, true)),
            ..Style::DEFAULT
        };
        let result = adapt(measured(), &style);
        assert_eq!(result.baselines, Baselines::NONE);
        assert_eq!(result.baselines_x, Baselines::NONE);
    }

    #[test]
    fn unavailable_baseline_keeps_sizing_and_containment_independent() {
        let style = Style {
            baseline_unavailable: true,
            ..Style::DEFAULT
        };
        let result = adapt(measured(), &style);
        assert_eq!(result.size, measured().size);
        assert_eq!(style.contain, Contain::NONE);
        assert_eq!(result.baselines, Baselines::NONE);
        assert_eq!(result.baselines_x, Baselines::NONE);
    }

    #[test]
    fn paint_containment_keeps_baselines() {
        let style = Style {
            contain: Contain::PAINT,
            ..Style::DEFAULT
        };
        assert_eq!(adapt(measured(), &style).baselines, measured().baselines);
    }

    #[test]
    fn inline_block_scroll_overflow_does_not_export_content_baselines() {
        let mut style = Style::DEFAULT;
        style.baseline_from_last = true;
        style.overflow.y = crate::style::Overflow::Hidden;
        let result = adapt(measured(), &style);
        assert_eq!(result.baselines, Baselines::NONE);
        assert_eq!(result.baselines_x, Baselines::NONE);
    }

    #[test]
    fn ordinary_scroll_container_retains_native_content_baselines() {
        let mut style = Style::DEFAULT;
        style.overflow.y = crate::style::Overflow::Hidden;
        assert_eq!(adapt(measured(), &style).baselines, measured().baselines);
    }
}
