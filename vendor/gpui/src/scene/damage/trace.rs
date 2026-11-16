//! KaminIDE patch: журнал повреждения кадра (`gpui::frame`, уровень debug) —
//! какие клетки повреждены и какая первая пара записей в них различается.
//! Нужен, чтобы причину перерисовки неизменного по пикселям кадра доказывать
//! записями сцены, а не догадкой.

use super::{Compare, DamageIndex, KIND_SHIFT, Scene, entry_area};
use collections::FxHashMap;

/// Сколько повреждённых клеток описать за кадр.
const TILES: usize = 6;

/// Записать в журнал повреждение кадра `scene` против `previous`.
pub(super) fn log_damage(scene: &Scene, index: &DamageIndex, previous: &Scene) {
    if !log::log_enabled!(target: "gpui::frame", log::Level::Debug) {
        return;
    }
    let old = &*previous.damage;
    log::debug!(
        target: "gpui::frame",
        "damage {:?} frame {} base {} size {:?} backdrop {}/{}",
        index.damage,
        index.id,
        index.base,
        index.size,
        index.backdrop,
        old.backdrop
    );
    if old.size != index.size || index.size.is_none() {
        return;
    }
    let mut cmp = Compare {
        new: scene,
        old: previous,
        groups: FxHashMap::default(),
    };
    let cols = index.cols as usize;
    let mut shown = 0;
    let mut damaged = 0;
    for t in 0..index.starts.len() - 1 {
        let (a, b) = (index.tile(t), old.tile(t));
        let first = (0..a.len().max(b.len())).find(|&k| match (a.get(k), b.get(k)) {
            (Some(&x), Some(&y)) => !cmp.same(scene, x, previous, y),
            _ => true,
        });
        let Some(k) = first else { continue };
        damaged += 1;
        if shown == TILES {
            continue;
        }
        shown += 1;
        log::debug!(
            target: "gpui::frame",
            "damage tile ({}, {}) entries {}/{} first differs at {}: new {} old {}",
            t % cols,
            t / cols,
            a.len(),
            b.len(),
            k,
            describe(scene, a.get(k).copied()),
            describe(previous, b.get(k).copied())
        );
    }
    log::debug!(target: "gpui::frame", "damage tiles {damaged}");
}

fn describe(scene: &Scene, entry: Option<u32>) -> String {
    let Some(e) = entry else {
        return "none".into();
    };
    let (kind, i) = (e >> KIND_SHIFT, (e & ((1 << KIND_SHIFT) - 1)) as usize);
    let area = entry_area(scene, e);
    let extra = match kind {
        1 => format!("{:?}", scene.quads[i].bounds),
        #[cfg(not(target_os = "macos"))]
        7 => {
            let s = &scene.surfaces[i];
            let g = (s.group as usize)
                .checked_sub(1)
                .and_then(|g| scene.groups.get(g));
            format!(
                "group {} bounds {:?} projection {}",
                s.group,
                g.map(|g| g.bounds),
                g.is_some_and(|g| g.projection.is_some())
            )
        }
        _ => String::new(),
    };
    format!("kind {kind} #{i} area {area:?} {extra}")
}
