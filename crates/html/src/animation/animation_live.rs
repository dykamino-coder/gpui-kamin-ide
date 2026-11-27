//! Live CSS keyframes rebuild the actual formatting element on the shared clock.
//! Frozen effects remain baked styles. Samples reach layout, child styles and
//! transforms through `animation_frame::sample`, without a separate painted box.

use crate::animation::animation_frame;
use crate::animation::frames::{bake_frozen, sample_at};
use crate::dom::Element;
use crate::layout::options::RenderOpts;
use crate::paint::effects::transform::transformed_node;
use crate::style::computed::{AnimSpec, BoxStyle, Computed};
use crate::values::ValueCx;
use gpui::{AnyElement, IntoElement};
use std::rc::Rc;

fn painted(e: &Element, inherited: &BoxStyle, opts: &RenderOpts) -> AnyElement {
    transformed_node(
        opts.cx.contracts.build.element(e, inherited, opts),
        &crate::style::computed::BoxStyle::shared(e.style.used()),
        inherited,
        e.node_id,
        opts.cx.frame(),
    )
}

async fn painted_stack(
    stk: &mut reblessive::Stk,
    e: &Element,
    inherited: &BoxStyle,
    opts: &RenderOpts,
) -> AnyElement {
    transformed_node(
        opts.cx
            .contracts
            .build
            .element_stack(stk, e, inherited, opts)
            .await,
        &crate::style::computed::BoxStyle::shared(e.style.used()),
        inherited,
        e.node_id,
        opts.cx.frame(),
    )
}

fn sample_live(
    source: &Element,
    frames: &[(f32, Computed)],
    spec: &AnimSpec,
    elapsed: f64,
    vcx: ValueCx<'_>,
) -> Element {
    sample_at(frames, spec, elapsed, vcx)
        .map(|frame| animation_frame::sample(source, &frame, true, vcx))
        .unwrap_or_else(|| source.clone())
}

pub(crate) async fn animated(
    stk: &mut reblessive::Stk,
    e: &Element,
    inherited: &BoxStyle,
    opts: &RenderOpts,
) -> AnyElement {
    let vcx = opts.cx.vcx();
    let (Some(frames), Some(spec)) = (
        e.anim.clone(),
        e.style.used().reset_context.animation.clone(),
    ) else {
        return painted_stack(stk, e, inherited, opts).await;
    };
    // The baked copy carries `transform`/`rotate`/`scale` as well: the block
    // path builds the transform wrapper from the element it is handed.
    if let Some(frozen) = bake_frozen(e, true, vcx) {
        return painted_stack(stk, &frozen, inherited, opts).await;
    }
    if spec.frozen() {
        return painted_stack(stk, e, inherited, opts).await;
    }
    let id = e.state_id("html-animation", vcx);
    let source = e.clone();
    let inherited = inherited.clone();
    let doc = opts.cx.doc().clone();
    let opts = opts.clone();
    let sampled_spec = spec.clone();
    super::keyframe::Keyframe {
        id,
        spec,
        doc,
        build: Rc::new(move |elapsed| {
            let vcx = opts.cx.vcx();
            let sampled = sample_live(&source, &frames, &sampled_spec, elapsed, vcx);
            painted(&sampled, &inherited, &opts)
        }),
    }
    .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(infinite: bool) -> AnimSpec {
        AnimSpec {
            name: "resize".into(),
            seconds: 0.5,
            infinite,
            alternate: true,
            delay: 0.0,
            paused: false,
            names: Vec::new(),
            fill: 0,
            iterations: 1.0,
            reverse: false,
        }
    }

    #[test]
    fn alternate_repeat_starts_at_first_keyframe_and_gives_each_leg_its_duration() {
        for (time, progress) in [(0.0, 0.0), (0.25, 0.5), (0.5, 1.0), (0.75, 0.5), (1.0, 0.0)] {
            assert_eq!(spec(true).progress_at(time), Some(progress));
        }
    }

    #[test]
    fn single_alternate_iteration_plays_forward_once() {
        assert_eq!(spec(false).progress_at(0.0), Some(0.0));
        assert_eq!(spec(false).progress_at(0.25), Some(0.5));
        assert_eq!(spec(false).progress_at(0.5), None);
    }

    #[test]
    fn live_sample_reaches_layout_and_transform_and_expires_without_fill() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        use crate::style::values::value::Len;

        let mut source = crate::layout::table::anon::anon_element("div", vec![]);
        source
            .style
            .used_mut()
            .apply_one("animation", "move 1s linear", vcx);
        let own = source.style.used();
        let spec = own.reset_context.animation.as_ref().unwrap();
        let mut start = source.style.cascaded().clone();
        start.apply_one("margin", "0px", vcx);
        start.apply_one("transform", "translateX(0px)", vcx);
        let mut end = source.style.cascaded().clone();
        end.apply_one("margin", "200px", vcx);
        end.apply_one("transform", "translateX(200px)", vcx);
        let frames = [(0.0, start), (1.0, end)];
        for (ms, x) in [(0, 0.0), (250, 50.0), (500, 100.0), (750, 150.0)] {
            let sampled = sample_live(&source, &frames, spec, f64::from(ms) / 1000.0, vcx);
            assert_eq!(
                sampled.style.used().reset_box_layout.margin.left,
                Some(Len::Px(x))
            );
            assert_eq!(
                sampled.style.used().reset_box_layout.margin.top,
                Some(Len::Px(x))
            );
            assert_eq!(
                sampled.style.used().reset_box_layout.margin.right,
                Some(Len::Px(x))
            );
            assert_eq!(
                sampled.style.used().reset_box_layout.margin.bottom,
                Some(Len::Px(x))
            );
            assert_eq!(
                sampled
                    .style
                    .used()
                    .reset_transforms
                    .transform
                    .unwrap()
                    .translate
                    .0,
                x
            );
        }
        let expired = sample_live(&source, &frames, spec, 1.5, vcx);
        assert_eq!(
            expired.style.used().reset_box_layout.margin,
            source.style.used().reset_box_layout.margin
        );
        assert_eq!(
            expired.style.used().reset_transforms.transform,
            source.style.used().reset_transforms.transform
        );
    }
}
