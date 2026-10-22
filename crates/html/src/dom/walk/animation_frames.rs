//! Разрешение `@keyframes` элемента при обходе: кадры как готовые стили.
//!
//! Вход — стиль элемента, наборы кадров документа и его переменные; выход —
//! список `(доля, Computed)`, где каждый кадр — объявления поверх базового
//! стиля (кадр заменяет `transform`, а не дописывает). Считается здесь,
//! потому что к отрисовке таблицы стилей уже нет. У остановленной анимации
//! достраиваются `0%`/`100%` (css-animations-1 §keyframes), несколько
//! остановленных складываются слоями по порядку списка (§3).

use crate::document::state::ValueCx;
use crate::style::computed::Computed;
use crate::style::css::{Keyframes, VarMap};
use std::collections::HashMap;

#[allow(clippy::too_many_arguments)]
pub(super) fn animation_frames(
    style: &Computed,
    frames: &HashMap<String, Keyframes>,
    vars: &VarMap,
    vcx: ValueCx<'_>,
) -> Option<Vec<(f32, Computed)>> {
    // Кадры разрешаются здесь же: к моменту отрисовки таблицы стилей
    // уже нет, а интерполировать нужно готовые стили, а не текст.
    let mut anim = style.animation.as_ref().and_then(|a| {
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
                        c.transform = None;
                    }
                    c.apply_decls_with_vars(decls, vars, vcx);
                    (*at, c)
                })
                .collect();
            // Недостающие `0%`/`100%` строятся из вычисленного стиля
            // (css-animations-1 §keyframes). Только у остановленной:
            // живая обёртка с таким кадром сделала бы reftest
            // недетерминированным (`individual-transform-combine`: пять
            // наборов из одного `to`).
            if a.frozen() {
                if resolved.first().is_some_and(|f| f.0 > 0.0) {
                    resolved.insert(0, (0.0, base.clone()));
                }
                if resolved.last().is_some_and(|f| f.0 < 1.0) {
                    resolved.push((1.0, base.clone()));
                }
            }
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
                    && let Some(sample) = crate::animation::frames::sample_at(&track, a, 0.0, vcx)
                {
                    base = sample;
                }
            }
            return Some(vec![(0.0, base.clone()), (1.0, base)]);
        }
        resolve(&a.name, &style)
    });
    if let (Some(resolved), Some(spec)) = (anim.as_mut(), style.animation.as_ref()) {
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
            let margin = &mut frame.margin;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::values::value::Len;

    #[test]
    fn only_declared_margin_tracks_replace_normalized_flow_edges() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
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
            let frames = animation_frames(&base, &tracks, &HashMap::new(), vcx).unwrap();
            for (_, frame) in frames {
                assert_eq!(frame.margin.left, expected);
                assert_eq!(
                    frame.margin.top,
                    (property == "margin").then_some(Len::Px(20.0))
                );
            }
        }
    }
}
