//! KaminIDE patch: экземпляр счётчиков пачек и времени GPU для приложения.

use std::sync::Mutex;

/// Виды пачек — в порядке `PrimitiveBatch`, последняя — буферы групп.
pub const KINDS: [&str; 9] = [
    "shadows", "quads", "paths", "underlines", "mono", "subpixel", "poly", "surfaces", "groups",
];

#[derive(Default, Clone, Copy)]
struct Kind {
    batches: u64,
    prims: u64,
    area_px: f64,
    gpu_ns: f64,
}

#[derive(Default)]
struct Totals {
    frames: u64,
    timed_frames: u64,
    frame_gpu_ns: f64,
    kinds: [Kind; 9],
}

/// Счётчики одного приложения; backend получает явный общий экземпляр.
#[derive(Default)]
pub struct BatchCost {
    totals: Mutex<Option<Totals>>,
    detail: Mutex<String>,
}

impl BatchCost {
    fn with(&self, f: impl FnOnce(&mut Totals)) {
        if let Ok(mut guard) = self.totals.lock() {
            f(guard.get_or_insert_with(Totals::default));
        }
    }

    /// Начат кадр окна.
    pub fn frame(&self) {
        self.with(|t| t.frames += 1);
    }

    /// Пачка на CPU-стороне: сколько примитивов и какой площади.
    pub fn count(&self, kind: usize, prims: usize, area_px: f64) {
        self.with(|t| {
            let k = &mut t.kinds[kind];
            k.batches += 1;
            k.prims += prims as u64;
            k.area_px += area_px;
        });
    }

    /// Готовый GPU-замер кадра: всё время и время каждой пачки.
    pub fn timed(&self, frame_ns: f64, kinds: &[(usize, f64)]) {
        self.with(|t| {
            t.timed_frames += 1;
            t.frame_gpu_ns += frame_ns;
            for (kind, ns) in kinds {
                t.kinds[*kind].gpu_ns += ns;
            }
        });
    }

    /// Описать крупнейшие квады сцены (зовёт рендерер раз на кадр).
    pub fn quad_detail(&self, quads: &[crate::Quad]) {
        let mut top: Vec<(f32, String)> = quads
            .iter()
            .map(|q| {
                let i = q.bounds.intersect(&q.content_mask.bounds);
                let (w, h) = (i.size.width.0.max(0.0), i.size.height.0.max(0.0));
                let c = &q.corner_radii;
                let r = [c.top_left, c.top_right, c.bottom_left, c.bottom_right]
                    .iter()
                    .fold(0.0f32, |m, v| m.max(v.0));
                let e = &q.border_widths;
                let b = [e.top, e.right, e.bottom, e.left]
                    .iter()
                    .fold(0.0f32, |m, v| m.max(v.0));
                let tf = q.transformation != crate::TransformationMatrix::unit();
                (
                    w * h,
                    format!(
                        "{w:.0}x{h:.0}@{:.0},{:.0} bg={:?} a={:.2} r={r:.0} b={b:.0}{}",
                        i.origin.x.0,
                        i.origin.y.0,
                        q.background.tag,
                        q.background.solid.a,
                        if tf { " tf" } else { "" }
                    ),
                )
            })
            .collect();
        top.sort_by(|a, b| b.0.total_cmp(&a.0));
        let line = top
            .iter()
            .take(10)
            .map(|(_, d)| d.as_str())
            .collect::<Vec<_>>()
            .join(" | ");
        if let Ok(mut detail) = self.detail.lock() {
            *detail = line;
        }
    }

    /// Строка для `diag.log` за период; счётчики обнуляются.
    pub fn take_line(&self) -> Option<String> {
        let t = self.totals.lock().ok()?.take()?;
        if t.frames == 0 {
            return None;
        }
        let per = |v: f64| v / t.frames as f64;
        let timed = t.timed_frames.max(1) as f64;
        let kinds = KINDS
            .iter()
            .zip(t.kinds.iter())
            .filter(|(_, k)| k.batches > 0)
            .map(|(name, k)| {
                let gpu = if t.timed_frames > 0 {
                    format!(" gpu_us={:.0}", k.gpu_ns / timed / 1000.0)
                } else {
                    String::new()
                };
                format!(
                    "{name}: n={:.0} kpx={:.0}{gpu}",
                    per(k.prims as f64),
                    per(k.area_px) / 1000.0
                )
            })
            .collect::<Vec<_>>()
            .join("; ");
        let detail = self.detail.lock().map(|d| d.clone()).unwrap_or_default();
        let frame_gpu = if t.timed_frames > 0 {
            format!(
                " timed={} frame_gpu_us={:.0}",
                t.timed_frames,
                t.frame_gpu_ns / timed / 1000.0
            )
        } else {
            String::new()
        };
        Some(format!(
            "[gpu] frames={}{frame_gpu} per frame: {kinds}; top quads: {detail}",
            t.frames
        ))
    }
}
