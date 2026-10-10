//! Вычисление треков и промежутков grid для gap rules.

use super::{GAP_EPS, GapItem, uniq_sorted};

/// Дорожки по оси `a`: начало — уникальные ближние края элементов, конец —
/// дальний край элемента, начатого в дорожке и не заходящего в следующую;
/// когда такого нет (все — спаны), начало следующей минус зазор.
pub(super) fn tracks_a(items: &[GapItem], gap: Option<f32>) -> Vec<(f32, f32)> {
    let st = uniq_sorted(items.iter().map(|i| i.a0).collect());
    (0..st.len())
        .map(|k| {
            let next = st.get(k + 1).copied();
            let end = items
                .iter()
                .filter(|i| {
                    (i.a0 - st[k]).abs() <= GAP_EPS && next.is_none_or(|nx| i.a1 <= nx + GAP_EPS)
                })
                .map(|i| i.a1)
                .fold(f32::NEG_INFINITY, f32::max);
            let end = if end.is_finite() {
                end
            } else {
                match (next, gap) {
                    (Some(nx), Some(g)) => nx - g,
                    (Some(nx), None) => nx,
                    (None, _) => st[k],
                }
            };
            (st[k], end)
        })
        .collect()
}

/// Дорожки по ШАБЛОНУ контейнера, привязанные к наблюдённым краям элементов.
///
/// `tracks_a` знает только границы коробок, поэтому дорожка без элементов
/// пропадает: два промежутка вокруг неё сливаются в один широкий, а
/// `§visibility-items: between/around` не может спрятать линейку над пустой
/// областью — хотя спека написана именно про неё («whether a gap decoration
/// segment is painted in portions of gaps adjacent to empty areas»,
/// css-gaps-1 §visibility-items).
///
/// Шаблон даёт РАЗМЕРЫ дорожек, но не их начало на экране; начало ищется
/// перебором: первый наблюдённый край элемента — это начало какой-то из
/// дорожек. Годной считается только та привязка, при которой КАЖДЫЙ элемент
/// стоит краями на линиях сетки. Проверка отсекает поля и выравнивание
/// элемента внутри дорожки, неявные дорожки авто-размещения и
/// нерасшифрованные доли `fr`: там возвращается `None` и работает прежний
/// счёт по элементам. Элемент-спан проверку проходит: его начало — начало
/// первой дорожки пролёта, конец — конец последней.
///
/// Ограничение: сетка, у которой пуста ВСЯ первая дорожка оси, привяжется со
/// сдвигом на дорожку — перебор идёт от нулевого смещения. В своде такой пары
/// нет (у всех 44 разрежённых первая строка и первая колонка заняты).
pub(super) fn template_tracks(
    sizes: &[f32],
    gap: Option<f32>,
    items: &[GapItem],
) -> Option<Vec<(f32, f32)>> {
    let g = gap?;
    if sizes.len() < 2 {
        return None;
    }
    let mut off = Vec::with_capacity(sizes.len());
    let mut y = 0.0f32;
    for w in sizes {
        off.push(y);
        y += w + g;
    }
    let first = uniq_sorted(items.iter().map(|i| i.a0).collect())
        .first()
        .copied()?;
    for shift in &off {
        let base = first - shift;
        let out: Vec<(f32, f32)> = sizes
            .iter()
            .zip(&off)
            .map(|(w, o)| (base + o, base + o + w))
            .collect();
        let fits = items.iter().all(|i| {
            out.iter().any(|t| (t.0 - i.a0).abs() <= GAP_EPS)
                && out.iter().any(|t| (t.1 - i.a1).abs() <= GAP_EPS)
        });
        if fits {
            return Some(out);
        }
    }
    None
}

/// Дорожки сетки по x и по y в координатах окна.
pub(crate) type GridTracks = (Vec<(f32, f32)>, Vec<(f32, f32)>);

/// Дорожки без схлопнутых: схлопнутая (`auto-fit` без элементов, css-grid-1
/// §7.2.3.2 «collapsed grid track… the gutters on either side of it…
/// collapse») приходит из раскладки дорожкой нулевого размера, прижатой к
/// соседу без зазора. При ненулевом `gap` такая дорожка — не дорожка и
/// промежутков не даёт (Blink `CollapsedTrackIndexes`).
pub(crate) fn uncollapsed(mut tracks: Vec<(f32, f32)>, gap: f32) -> Vec<(f32, f32)> {
    // При `rtl` и обратных осях раскладка отдаёт дорожки в логическом
    // порядке — здесь нужен физический.
    tracks.sort_by(|x, y| x.0.partial_cmp(&y.0).unwrap_or(std::cmp::Ordering::Equal));
    if gap <= GAP_EPS {
        return tracks;
    }
    let n = tracks.len();
    (0..n)
        .filter(|&k| {
            let (s, e) = tracks[k];
            let empty = e - s <= GAP_EPS;
            let flush_prev = k > 0 && (s - tracks[k - 1].1).abs() <= GAP_EPS;
            let flush_next = k + 1 < n && (tracks[k + 1].0 - e).abs() <= GAP_EPS;
            !(empty && (flush_prev || flush_next))
        })
        .map(|k| tracks[k])
        .collect()
}

/// Промежутки между соседними дорожками; нулевой зазор — тоже промежуток
/// (`flex-gap-decorations-033`).
pub(super) fn gaps_of(tracks: &[(f32, f32)]) -> Vec<(f32, f32)> {
    tracks
        .windows(2)
        .filter(|w| w[1].0 - w[0].1 >= -GAP_EPS)
        .map(|w| (w[0].1.min(w[1].0), w[1].0))
        .collect()
}
