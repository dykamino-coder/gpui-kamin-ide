//! Дорожки родительской сетки — вниз, в подсетку (css-grid-2 §subgrids).
//!
//! Обход сверху вниз по собранному дереву: у сетки с точечными дорожками (доли
//! заранее переведены `fr_tracks_to_px`) каждому подсеточному ребёнку
//! вырезается кусок по его пролёту (явное размещение или курсор
//! авто-размещения §8.5, который двигают все дети). Запрещённая подсетка
//! получает `none` (§subgrid-listing); срез кладётся только в связанную ось
//! (`subgrid_axes::linked`), доработка краёв, зазоров и растяжки — `finish_slice`.

mod fraction_tracks;
mod slice_finish;
pub(super) use crate::dom::fixup_grid::subgrid_tracks::fraction_tracks::fr_tracks_to_px;
pub(crate) use crate::dom::fixup_grid::subgrid_tracks::slice_finish::finish_slice;

use super::{subgrid_gap_slice, subgrid_inhibited, subgrid_slot, subgrid_span};
use crate::dom::*;
use crate::style::computed::Display;

pub(crate) fn subgrid_takes_parent_tracks(nodes: &mut [Node]) {
    for node in nodes.iter_mut() {
        let Node::Element(el) = node else { continue };
        if matches!(
            el.style.cascaded().display,
            Some(Display::Grid) | Some(Display::InlineGrid)
        ) {
            for row_dir in [false, true] {
                let raw = if row_dir {
                    el.style.cascaded().grid_rows.clone()
                } else {
                    el.style.cascaded().grid_tracks.clone()
                }
                .unwrap_or_default();
                // Доли `fr` при точечном размере родителя — в точки ДО нарезки
                // (`fr_tracks_to_px`). Такой срез годен только оси, где ребёнок
                // вправду подсеточный (проверка ниже, у ребёнка).
                let fr_px = fr_tracks_to_px(el.style.cascaded(), &raw, row_dir);
                let from_fr = fr_px.is_some();
                let tracks = fr_px.unwrap_or(raw);
                // Гейт не расширять с «все дорожки `Px`» до «все нарезаемы»
                // (симметрично здесь и в `render::generic_box`): грубо ломаются
                // `subgrid-gap-decorations-003`, `auto-track-sizing-001`,
                // `row-subgrid-orthogonal-writing-mode-002`. Сначала нужно
                // снять фазовый разрыв между лунками и обычной сеткой: в
                // `grid-subgridded-to-grid-lanes/**` тест и эталон отличаются
                // одним словом разметки и идут разными путями, поэтому
                // односторонняя правка разводит пару.
                if tracks.is_empty()
                    || !tracks.iter().all(|t| {
                        matches!(
                            t,
                            crate::style::computed::TrackSize::Single(
                                crate::style::computed::Track::Px(_)
                            )
                        )
                    })
                {
                    continue;
                }
                // Курсор авто-размещения (§8.5, разрежённая укладка): у
                // подсетки без начальной линии срез всё равно ЕСТЬ — она
                // встаёт в следующее свободное место своего пролёта. Курсор
                // ведут ВСЕ дети, а не только подсеточные: место занимает
                // каждый.
                let mut cur = 0usize;
                for child in el.children.iter_mut() {
                    let Node::Element(child) = child else {
                        continue;
                    };
                    let place = if row_dir {
                        &child.style.cascaded().grid_row
                    } else {
                        &child.style.cascaded().grid_col
                    };
                    let slot = match subgrid_slot(place, tracks.len()) {
                        Some((at, span)) => {
                            cur = (at + span).min(tracks.len());
                            Some((at, span))
                        }
                        None => {
                            let span = subgrid_span(place);
                            if cur + span > tracks.len() {
                                cur = 0;
                            }
                            let at = cur;
                            cur = (cur + span).min(tracks.len());
                            (at + span <= tracks.len()).then_some((at, span))
                        }
                    };
                    if !child.style.cascaded().subgrid {
                        continue;
                    }
                    // css-grid-2 §subgrid-listing: использованное значение у
                    // такого элемента — НАЧАЛЬНОЕ `none`, то есть не «срез не
                    // выдали», а «явных дорожек нет вовсе». Иначе слово
                    // `subgrid` доживает до раскладки счётной дорожкой:
                    // `count_tracks` считает его за одну.
                    if subgrid_inhibited(child.style.cascaded()) {
                        if row_dir {
                            child.style.cascaded_mut().grid_rows = None;
                        } else {
                            child.style.cascaded_mut().grid_tracks = None;
                            child.style.cascaded_mut().grid_cols = None;
                        }
                        continue;
                    }
                    // Переведённые доли режутся только в ПАРАЛЛЕЛЬНУЮ ось, где
                    // написано `subgrid`: своя ось подсетки остаётся своей
                    // (`subgrid-gap-decorations-003`: ряды `subgrid`, колонки
                    // `repeat(2, 1fr)` — сырая доля в своей оси ломает пару).
                    let parallel = child.style.cascaded().wm().vertical()
                        == el.style.cascaded().wm().vertical();
                    if !subgrid_axes::linked(el.style.cascaded(), child.style.cascaded(), row_dir)
                        || (from_fr && !parallel)
                    {
                        continue;
                    }
                    let Some((at, span)) = slot else {
                        continue;
                    };
                    let slice: Vec<crate::style::computed::TrackSize> = (at..at + span)
                        .filter_map(|i| tracks.get(i).cloned())
                        .collect();
                    if slice.len() != span || span == 0 {
                        continue;
                    }
                    // Свои края подсетки вычитаются из первой и последней
                    // дорожки куска — ровно как в раскладке лунок.
                    finish_slice(
                        child,
                        el.style.cascaded(),
                        row_dir,
                        parallel,
                        at,
                        span,
                        &tracks,
                        slice,
                    );
                }
            }
        }
        subgrid_takes_parent_tracks(&mut el.children);
    }
}
