//! Расширение маски силуэта для полос рамки.

/// Дилатация Минковского диском `sm` (blink ComputeShapeMarginIntervals):
/// каждый интервал раздаётся соседним строкам с сужением по дуге; ранний
/// выход, когда сосед и так шире. Вертикаль жёстко в [0, rows).
pub(crate) fn dilate(iv: &mut [Option<(i32, i32)>], sm: f32, cols: usize, rows: usize) {
    if sm <= 0.0 {
        return;
    }
    let cap = ((cols.max(rows) as f32) * std::f32::consts::SQRT_2) as i32;
    let r = (sm.ceil() as i32).clamp(0, cap.max(1));
    let dx: Vec<i32> = (0..=r)
        .map(|k| (((r * r - k * k) as f32).sqrt()) as i32)
        .collect();
    let src: Vec<Option<(i32, i32)>> = iv.to_vec();
    let top = src.iter().position(|s| s.is_some());
    let bot = src.iter().rposition(|s| s.is_some());
    let (top, bot) = match (top, bot) {
        (Some(a), Some(b)) => (a as i32, b as i32),
        _ => return,
    };
    let unite = |slot: &mut Option<(i32, i32)>, x1: i32, x2: i32| match slot {
        None => *slot = Some((x1, x2)),
        Some((a, b)) => {
            *a = (*a).min(x1);
            *b = (*b).max(x2);
        }
    };
    for y in 0..rows as i32 {
        let Some((x1, x2)) = src[y as usize] else {
            continue;
        };
        let contains =
            |m: i32| -> bool { matches!(src[m as usize], Some((a, b)) if a <= x1 && b >= x2) };
        // вверх
        let y0 = (y - r).max(0);
        let mut my = y - 1;
        while my >= y0 {
            if my > top && contains(my) {
                break;
            }
            let d = dx[(y - my) as usize];
            unite(&mut iv[my as usize], x1 - d, x2 + d);
            my -= 1;
        }
        unite(&mut iv[y as usize], x1 - dx[0], x2 + dx[0]);
        // вниз
        let y1 = (y + r).min(rows as i32 - 1);
        let mut my = y + 1;
        while my <= y1 {
            if my < bot && contains(my) {
                break;
            }
            let d = dx[(my - y) as usize];
            unite(&mut iv[my as usize], x1 - d, x2 + d);
            my += 1;
        }
    }
}
