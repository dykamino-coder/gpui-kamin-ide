//! Resolves an element's `@keyframes` during the walk: frames as finished styles.
//!
//! Input is the element style, the document keyframe sets and its variables; output is
//! a list of `(progress, Computed)` where each frame is declarations over the base
//! style (a frame replaces `transform`, it does not append). It is computed here
//! because the stylesheets are gone by paint time.
//! Missing `0%`/`100%` frames are built from the underlying style
//! (css-animations-1 §keyframes); several frozen animations stack in list order (§3).

use crate::style::computed::Computed;
use crate::style::css::{DeclBlock, Keyframes, VarMap};
use crate::values::ValueCx;
use std::collections::HashMap;

#[allow(clippy::too_many_arguments)]
pub(super) fn animation_frames(
    style: &Computed,
    frames: &HashMap<String, Keyframes>,
    vars: &VarMap,
    important: Option<&DeclBlock>,
    vcx: ValueCx<'_>,
) -> Option<Vec<(f32, Computed)>> {
    // Кадры разрешаются здесь же: к моменту отрисовки таблицы стилей
    // уже нет, а интерполировать нужно готовые стили, а не текст.
    let mut anim = style.reset_context.animation.as_ref().and_then(|a| {
        // Набор `name` поверх стиля `base`.
        let resolve = |name: &str, base: &Computed| -> Option<Vec<(f32, Computed)>> {
            let track = frames.get(name)?;
            let mut resolved: Vec<(f32, Computed)> = track
                .iter()
                .map(|(at, decls)| {
                    let mut c = base.clone();
                    // Кадр ЗАМЕНЯЕТ `transform`, а разбор свойства
                    // дописывает функции к уже стоящим (ветка
                    // `"transform"`: `self.transform.unwrap_or_default()`).
                    if decls.contains("transform") {
                        c.reset_transforms.transform = None;
                    }
                    c.apply_decls_with_vars(decls, vars, vcx);
                    // Происхождение анимаций ниже важных объявлений
                    // (css-cascade-5 §6.2): важные победители тех свойств,
                    // что трогает кадр, ложатся обратно поверх него.
                    if let Some(important) = important {
                        let over: DeclBlock = important
                            .iter()
                            .filter(|d| decls.iter().any(|k| related(&d.name, &k.name)))
                            .cloned()
                            .collect();
                        if !over.is_empty() {
                            // An important transform replaces the animation's
                            // list; applying it to that matrix would append it.
                            if over.contains("transform") {
                                c.reset_transforms.transform = None;
                            }
                            c.apply_decls_with_vars(&over, vars, vcx);
                        }
                    }
                    (*at, c)
                })
                .collect();
            // Missing `0%`/`100%` frames are built from the underlying computed
            // style (css-animations-1 §keyframes), for running and frozen
            // animations alike: `to { opacity: 1 }` over `opacity: 0` fades in.
            close_track(&mut resolved, base);
            (resolved.len() >= 2).then_some(resolved)
        };
        // Несколько остановленных анимаций — слоями по порядку списка
        // (css-animations-1 §3: при общем свойстве побеждает имя,
        // стоящее позже); итог — постоянный набор из двух одинаковых
        // кадров (`individual-transform-ordering`: `anim-7, anim-8`).
        if a.frozen() && a.names.len() > 1 {
            let mut base = style.clone();
            for name in &a.names {
                if let Some(track) = resolve(name, &base)
                    && let Some(sample) = crate::style::animation::sample_at(&track, a, 0.0, vcx)
                {
                    base = sample;
                }
            }
            return Some(vec![(0.0, base.clone()), (1.0, base)]);
        }
        resolve(&a.name, &style)
    });
    if let (Some(resolved), Some(spec)) = (anim.as_mut(), style.reset_context.animation.as_ref()) {
        // Frames contain underlying values too. Only animated margins may replace
        // the node's edges: flow may already have consumed its collapsed margins.
        let animated = ["margin-top", "margin-right", "margin-bottom", "margin-left"].map(|key| {
            spec.names
                .iter()
                .chain(std::iter::once(&spec.name))
                .any(|name| {
                    frames.get(name).is_some_and(|track| {
                        track
                            .iter()
                            .any(|(_, decls)| decls.contains("margin") || decls.contains(key))
                    })
                })
        });
        for (_, frame) in resolved {
            let margin = &mut frame.reset_box_layout.margin;
            for (edge, active) in [
                &mut margin.top,
                &mut margin.right,
                &mut margin.bottom,
                &mut margin.left,
            ]
            .into_iter()
            .zip(animated)
            {
                *edge = active.then_some(edge.unwrap_or(crate::style::values::value::Len::Px(0.0)));
            }
        }
    }
    anim
}

/// Adds the implicit `0%`/`100%` keyframes (css-animations-1 §keyframes:
/// "If a 0% or from keyframe is not specified, then the user agent constructs
/// a 0% keyframe using the computed values of the properties being animated").
fn close_track(track: &mut Vec<(f32, Computed)>, base: &Computed) {
    if track.first().is_some_and(|f| f.0 > 0.0) {
        track.insert(0, (0.0, base.clone()));
    }
    if track.last().is_some_and(|f| f.0 < 1.0) {
        track.push((1.0, base.clone()));
    }
}

/// Одно свойство или сокращение и его длинное свойство (`background` и
/// `background-color`): пока сокращения не раскрыты при разборе, так
/// узнаётся, что кадр трогает важное свойство.
fn related(a: &str, b: &str) -> bool {
    let part = |long: &str, short: &str| {
        long.len() > short.len() && long.starts_with(short) && long.as_bytes()[short.len()] == b'-'
    };
    a == b || part(a, b) || part(b, a)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::values::value::Len;

    #[test]
    fn running_animation_gets_implicit_end_frames_from_the_underlying_style() {
        let doc = crate::values::DocumentValues::detached(crate::style::css::Media::default());
        let vcx = doc.values();
        let mut base = Computed::default();
        base.apply_one("animation", "fade 1s linear", vcx);
        base.apply_one("opacity", "0", vcx);
        assert!(!base.reset_context.animation.as_ref().unwrap().frozen());
        for (track, at) in [
            (
                vec![(1.0, crate::style::css::parse_decls("opacity: 1"))],
                [0.0, 1.0],
            ),
            (
                vec![(0.5, crate::style::css::parse_decls("opacity: 1"))],
                [0.0, 0.5],
            ),
        ] {
            let tracks = HashMap::from([("fade".into(), track)]);
            let frames = animation_frames(&base, &tracks, &HashMap::new(), None, vcx).unwrap();
            assert_eq!(frames.first().unwrap().0, 0.0);
            assert_eq!(frames.last().unwrap().0, 1.0);
            assert_eq!(frames[0].1.reset_effects.opacity, Some(0.0));
            let declared = frames.iter().find(|f| f.0 == at[1]).unwrap();
            assert_eq!(declared.1.reset_effects.opacity, Some(1.0));
        }
    }

    #[test]
    fn only_declared_margin_tracks_replace_normalized_flow_edges() {
        let doc = crate::values::DocumentValues::detached(crate::style::css::Media::default());
        let vcx = doc.values();
        let mut base = Computed::default();
        base.apply_one("animation", "a 1s linear", vcx);
        base.apply_one("margin", "12px", vcx);
        for (property, value, expected) in [
            ("color", "blue", None),
            ("margin-left", "0px", Some(Len::Px(0.0))),
            ("margin", "20px", Some(Len::Px(20.0))),
        ] {
            let decls = crate::style::css::parse_decls(&format!("{property}: {value}"));
            let tracks = HashMap::from([("a".into(), vec![(0.0, decls.clone()), (1.0, decls)])]);
            let frames = animation_frames(&base, &tracks, &HashMap::new(), None, vcx).unwrap();
            for (_, frame) in frames {
                assert_eq!(frame.reset_box_layout.margin.left, expected);
                assert_eq!(
                    frame.reset_box_layout.margin.top,
                    (property == "margin").then_some(Len::Px(20.0))
                );
            }
        }
    }
    #[test]
    fn important_transform_replaces_the_animated_list() {
        let doc = crate::values::DocumentValues::detached(crate::style::css::Media::default());
        let vcx = doc.values();
        let mut base = Computed::default();
        base.apply_one("animation", "a 1s linear", vcx);
        base.apply_one("transform", "translateX(10px)", vcx);
        let decls = crate::style::css::parse_decls("transform: scale(2)");
        let important = crate::style::css::parse_decls("transform: translateX(10px)");
        let tracks = HashMap::from([("a".into(), vec![(0.0, decls.clone()), (1.0, decls)])]);
        let frames =
            animation_frames(&base, &tracks, &HashMap::new(), Some(&important), vcx).unwrap();
        for (_, frame) in frames {
            assert_eq!(
                frame.reset_transforms.transform,
                base.reset_transforms.transform
            );
        }
    }
}
