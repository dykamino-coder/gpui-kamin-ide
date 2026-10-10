//! Проход размещения элементов сетки по рядам авто-полос.

use crate::dom::{Element, Node};
use crate::layout::fragment::ShapeCx;
use crate::layout::fragment::clone::solid_box;
use crate::layout::fragment::fragment_size::shape_full;
use crate::render::{is_blank, out_of_flow};
use crate::style::computed::Placement;
use crate::style::computed::{Computed, Display};
use crate::style::values::value::Len;

#[allow(clippy::too_many_arguments)]
pub(crate) fn grid_pass(
    c: &Element,
    depth: u8,
    cx: ShapeCx,
    s: &Computed,
    cols: usize,
    used: &mut Vec<Vec<bool>>,
    spots: &mut Vec<(
        usize,
        usize,
        f32,
        (f32, f32, f32, Vec<(f32, f32)>, Vec<f32>, Vec<(f32, f32)>),
        bool,
    )>,
    fill: &mut Vec<f32>,
    pass: u8,
) -> Option<()> {
    let mut cur_row = 0usize;
    let mut cur_col = 0usize;
    for (ix, n) in c.children.iter().enumerate().filter(|(_, n)| !is_blank(n)) {
        let Node::Element(k) = n else {
            return None;
        };
        if matches!(k.style.display, Some(Display::None)) || out_of_flow(&k.style) {
            continue;
        }
        // `(ряд, начальная колонка, охват колонок)`. Именованная область —
        // её прямоугольник (как `place_named_areas`), только в ОДИН ряд:
        // охват рядов сам меняет размер дорожек (css-grid-1 §12.5).
        // Колонка — css-grid-1 §8.3: `N`, `span M`, `N / span M`, `N / M`;
        // неявные колонки за краем и отрицательные линии — отказ.
        let (line, cline, cspan) = if let Some(name) = &k.style.grid_area_name {
            let areas = s.grid_areas.as_ref()?;
            let (mut r0, mut r1, mut c0, mut c1) = (usize::MAX, 0usize, usize::MAX, 0usize);
            for (r, cells) in areas.iter().enumerate() {
                for (cc, cell) in cells.iter().enumerate() {
                    if cell == name {
                        r0 = r0.min(r);
                        r1 = r1.max(r + 1);
                        c0 = c0.min(cc);
                        c1 = c1.max(cc + 1);
                    }
                }
            }
            if r0 == usize::MAX || r1 != r0 + 1 {
                return None;
            }
            (Some(r0), Some(c0), c1 - c0)
        } else {
            let line = match k.style.grid_row {
                None | Some((Placement::Auto, Placement::Auto)) => None,
                Some((Placement::Line(a), Placement::Auto)) if a >= 1 => Some((a - 1) as usize),
                _ => return None,
            };
            let (cline, cspan) = match k.style.grid_col {
                None | Some((Placement::Auto, Placement::Auto)) => (None, 1usize),
                Some((Placement::Span(m), Placement::Auto)) => (None, m.max(1) as usize),
                Some((Placement::Line(a), Placement::Auto)) if a >= 1 => {
                    (Some((a - 1) as usize), 1usize)
                }
                Some((Placement::Line(a), Placement::Span(m))) if a >= 1 => {
                    (Some((a - 1) as usize), m.max(1) as usize)
                }
                Some((Placement::Line(a), Placement::Line(b))) if a >= 1 && b > a => {
                    (Some((a - 1) as usize), (b - a) as usize)
                }
                _ => return None,
            };
            (line, cline, cspan)
        };
        if cspan > cols || cline.is_some_and(|c0| c0 + cspan > cols) {
            return None;
        }
        // Проход 0 — «locked to a given row» (§8.5 шаг 2), проход 1 —
        // курсор (§8.5 шаг 4). Мера ребёнка берётся ровно один раз.
        if (pass == 0) != line.is_some() {
            continue;
        }
        let ks = shape_full(k, depth - 1, cx)?;
        let h = ks.0 + ks.1 + ks.2;
        let busy = |used: &Vec<Vec<bool>>, r: usize, c0: usize| {
            used.get(r)
                .is_some_and(|v| v[c0..c0 + cspan].iter().any(|x| *x))
        };
        let (row, col) = match (line, cline) {
            // Ряд и колонка заданы (область): ячейки как есть — §8.5
            // шаг 1, перекрытие законно.
            (Some(r), Some(c0)) => (r, c0),
            (Some(r), None) => {
                // «the earliest line index that ensures this item's grid
                // area will not overlap any occupied grid cells».
                let mut cc = 0usize;
                while cc + cspan < cols && busy(&*used, r, cc) {
                    cc += 1;
                }
                (r, cc)
            }
            // §8.5 шаг 4 «sparse», заданная колонка: курсор-ряд растёт,
            // если колонка левее курсора, и дальше — до свободных ячеек.
            (None, Some(c0)) => {
                let mut rr = cur_row + usize::from(c0 < cur_col);
                while busy(&*used, rr, c0) {
                    rr += 1;
                }
                (rr, c0)
            }
            // Авто-колонка с охватом: первое место от курсора, где
            // свободны все `cspan` ячеек подряд.
            (None, None) => {
                let mut rr = cur_row;
                let mut cc = cur_col;
                loop {
                    if cc + cspan > cols {
                        cc = 0;
                        rr += 1;
                        continue;
                    }
                    if !busy(&*used, rr, cc) {
                        break;
                    }
                    cc += 1;
                }
                (rr, cc)
            }
        };
        while used.len() <= row {
            used.push(vec![false; cols]);
            fill.push(0.0);
        }
        for x in &mut used[row][col..col + cspan] {
            *x = true;
        }
        fill[row] = fill[row].max(h);
        // Внутренние точки элемента годны сетке, только если его коробка
        // стоит в начале ряда: монолит (`solid_box`) целиком не режется,
        // выравнивание center/end/baseline и `margin-top: auto` сдвигают
        // коробку на величину, которой мера не знает
        // (`grid-item-fragmentation-024`, `-008`).
        let aligned = |a: &Option<crate::style::computed::Align>| {
            matches!(
                a,
                Some(crate::style::computed::Align::Center)
                    | Some(crate::style::computed::Align::End)
                    | Some(crate::style::computed::Align::Baseline)
                    | Some(crate::style::computed::Align::AnchorCenter)
            )
        };
        let plain = !solid_box(k)
            && !aligned(&k.style.align_self)
            && !(k.style.align_self.is_none() && aligned(&c.style.align_items))
            && !matches!(k.style.margin.top, Some(Len::Auto));
        spots.push((ix, row, ks.1, ks, plain));
        if line.is_none() {
            cur_row = row;
            cur_col = col + cspan;
            if cur_col >= cols {
                cur_col = 0;
                cur_row += 1;
            }
        }
    }
    Some(())
}
