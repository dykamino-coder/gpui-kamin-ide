//! Animation frames for walk; split out to keep the owning module within 250 lines.

use crate::style::computed::Computed;
use crate::style::css::{Decls, Keyframes};
use std::collections::HashMap;

#[allow(clippy::too_many_arguments)]
#[allow(clippy::let_and_return, clippy::needless_borrow)]
pub(super) fn animation_frames(
    style: &Computed,
    frames: &HashMap<String, Keyframes>,
    vars: &Decls,
) -> Option<Vec<(f32, Computed)>> {
    // Кадры разрешаются здесь же: к моменту отрисовки таблицы стилей
    // уже нет, а интерполировать нужно готовые стили, а не текст.
    let anim = style.animation.as_ref().and_then(|a| {
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
                    if decls.contains_key("transform") {
                        c.transform = None;
                    }
                    c.apply_decls_with_vars(decls, vars);
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
            let t = a.frozen_t();
            let mut base = style.clone();
            for name in &a.names {
                if let Some(track) = resolve(name, &base) {
                    base = crate::animation::frames::frame_at(&track, t);
                }
            }
            return Some(vec![(0.0, base.clone()), (1.0, base)]);
        }
        resolve(&a.name, &style)
    });
    anim
}
