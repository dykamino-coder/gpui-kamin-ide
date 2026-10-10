//! Профиль фигуры обтекания (shape-outside, shape-margin): маска, интервалы, расширение.

mod svg_path;
pub(super) use svg_path::svg_path_of;

mod mask;
use mask::dilate;
use mask::mask_intervals;
use mask::shape_mask;

use crate::paint::background::*;

// --- Общий путь формы обтекания (css-shapes-1 §3, §shape-margin) ---------
//
// Форма растрируется альфа-маской в холст margin-box (1 пиксель = 1 точка,
// как blink RasterShape), маска сводится к интервалам строк «первый..
// последний непрозрачный», интервалы раздуваются диском shape-margin
// (дилатация Минковского по blink ComputeShapeMarginIntervals) и
// превращаются в экстенты от начала стороны текста. Клип к margin-box
// двойной: холст режет форму, зажим на шаге экстента режет поле
// («a shape can only ever reduce a float area»).

/// Геометрия флоата в системе его margin-box, всё в CSS-точках.
pub struct ShapeBox {
    pub mw: f32,
    pub mh: f32,
    /// Опорная коробка формы.
    pub rx: f32,
    pub ry: f32,
    pub rw: f32,
    pub rh: f32,
    /// Content-box (для картинки/градиента).
    pub cx: f32,
    pub cy: f32,
    pub cw: f32,
    pub ch: f32,
    /// Радиусы опорной коробки (tl,tr,br,bl), эллиптические.
    pub radius: [(f32, f32); 4],
    pub threshold: f32,
}

/// Экстенты обтекания по строкам margin-box; None — форма нераспознана
/// (вызывающий откатывается к прямоугольнику коробки). Пустая форма —
/// нули: «empty float area», НЕ фоллбек.
pub fn shape_profile(raw: &str, b: &ShapeBox, sm: f32, side: i32) -> Option<Vec<f32>> {
    let rows = b.mh.ceil().max(1.0) as usize;
    let cols = b.mw.ceil().max(1.0) as usize;
    let mask = shape_mask(raw, b, cols, rows)?;
    let mut iv = mask_intervals(&mask, cols, rows, b.threshold);
    // `shape-margin` раздувает фигуру наружу на своё расстояние
    // (css-shapes-1 §2.2): контур обтекания — множество точек не дальше
    // `shape-margin` от исходной фигуры. Раздутие было написано и не
    // подключено — общий растровый путь отдавал профиль как есть.
    dilate(&mut iv, sm, cols, rows);
    Some(
        iv.into_iter()
            .map(|slot| match slot {
                None => 0.0,
                Some((x1, x2)) => {
                    if side < 0 {
                        (x2 as f32).clamp(0.0, b.mw)
                    } else {
                        b.mw - (x1 as f32).clamp(0.0, b.mw)
                    }
                }
            })
            .collect(),
    )
}

/// Экстенты обтекания вдоль БЛОК-оси вертикального письма.
///
/// В вертикали строки набора — это колонки: блок-ось горизонтальна и идёт
/// от правого края (`vertical-rl`, `sideways-rl`), инлайн-ось вертикальна,
/// а line-left = верх, line-right = низ (css-writing-modes-4 §6.3, таблица
/// logical-to-physical). Поэтому ту же маску формы надо резать СТОЛБЦАМИ, а
/// экстент мерить вдоль физической вертикали. Индекс результата —
/// расстояние от блок-старта margin-box, значение — экстент от своей
/// line-стороны; ровно в этих осях работает `FlowRow::vertical_rl`.
///
/// Раздутие `shape-margin` — тот же диск Минковского (css-shapes-1 §2.2),
/// что и у строчного профиля: `dilate` не знает, какая ось «длинная», ей
/// достаточно поменять местами два размера.
pub fn shape_profile_block(raw: &str, b: &ShapeBox, sm: f32, side: i32) -> Option<Vec<f32>> {
    let rows = b.mh.ceil().max(1.0) as usize;
    let cols = b.mw.ceil().max(1.0) as usize;
    // Circle/ellipse without shape-margin: exact extent per one-pixel column,
    // i.e. the chord at the column edge nearest the centre — a line band
    // takes the shape's maximum over its whole block range (css-shapes-1
    // §2), and pixel-centre sampling fell half a pixel short
    // (`shape-outside-circle-048..053`: a box one device row high).
    if sm <= 0.0 && (raw.contains("circle(") || raw.contains("ellipse(")) {
        let raw = raw.trim();
        let at = raw.find("circle(").or_else(|| raw.find("ellipse("))?;
        let head = &raw[at..];
        let head = match head.find(')') {
            Some(end) => &head[..=end],
            None => head,
        };
        let (cx, cy, rx, ry) = shape_params(head, b.rw, b.rh, 1.0)?;
        let (cx, cy) = (cx + b.rx, cy + b.ry);
        return Some(
            (0..cols)
                .rev()
                .map(|x| {
                    if rx <= 0.0 || ry <= 0.0 {
                        return 0.0;
                    }
                    let (x0, x1) = (x as f32, x as f32 + 1.0);
                    let dx = if cx < x0 {
                        x0 - cx
                    } else if cx > x1 {
                        cx - x1
                    } else {
                        0.0
                    };
                    if dx >= rx {
                        return 0.0;
                    }
                    let h = ry * (1.0 - (dx / rx) * (dx / rx)).sqrt();
                    if side < 0 {
                        (cy + h).clamp(0.0, b.mh)
                    } else {
                        b.mh - (cy - h).clamp(0.0, b.mh)
                    }
                })
                .collect(),
        );
    }
    let mask = shape_mask(raw, b, cols, rows)?;
    let t = (b.threshold.clamp(0.0, 1.0) * 255.0) as u8;
    let mut iv: Vec<Option<(i32, i32)>> = (0..cols)
        .map(|x| {
            let first = (0..rows).find(|&y| mask[y * cols + x] > t)?;
            let last = (0..rows).rev().find(|&y| mask[y * cols + x] > t)?;
            Some((first as i32, last as i32 + 1))
        })
        .collect();
    dilate(&mut iv, sm, rows, cols);
    Some(
        (0..cols)
            .rev()
            .map(|x| match iv[x] {
                None => 0.0,
                Some((y1, y2)) => {
                    if side < 0 {
                        (y2 as f32).clamp(0.0, b.mh)
                    } else {
                        b.mh - (y1 as f32).clamp(0.0, b.mh)
                    }
                }
            })
            .collect(),
    )
}
