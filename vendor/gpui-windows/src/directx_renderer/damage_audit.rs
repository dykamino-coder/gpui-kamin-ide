//! KaminIDE patch: сверка кадра, нарисованного по плану (повреждение, кэш
//! групп), с тем же кадром, нарисованным целиком без кэша групп.
//!
//! Только для стенда ([`set_damage_audit`]): каждый нарисованный кадр
//! читается из цели, затем та же сцена рисуется заново целиком без картинок
//! прошлого кадра и читается ещё раз. Несовпадение пишется в лог ошибкой с
//! номером кадра, планом и рамкой различий. Цель после сверки держит целый
//! кадр: следующий кадр сравнивается с верной базой, и ошибка видна там,
//! где возникла, а не там, где накопилась.

use super::*;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

static AUDIT: AtomicBool = AtomicBool::new(false);
static MISMATCHES: AtomicU64 = AtomicU64::new(0);

/// Включить сверку для всех окон процесса (стенд, сверка RGB).
pub fn set_damage_audit(on: bool) {
    AUDIT.store(on, Ordering::Relaxed);
}

/// Число кадров, не совпавших с целой перерисовкой, с начала процесса.
pub fn damage_audit_mismatches() -> u64 {
    MISMATCHES.load(Ordering::Relaxed)
}

pub(super) fn enabled() -> bool {
    AUDIT.load(Ordering::Relaxed)
}

impl DirectXRenderer {
    /// Compare the frame just drawn by plan `within` with a whole redraw.
    pub(super) fn audit_frame(
        &mut self,
        scene: &Scene,
        background: WindowBackgroundAppearance,
        within: &retained::Plan,
    ) -> Result<()> {
        let planned = self.read_target()?;
        let (kept, drawn) = (
            self.retained.groups.counts.kept,
            self.retained.groups.counts.drawn,
        );
        // Целый кадр без картинок групп прошлого кадра.
        self.retained.chain = None;
        self.retained.groups.valid.clear();
        self.render(scene, background, None)?;
        let whole = self.read_target()?;
        let w = self.width as usize;
        let mut count = 0usize;
        let (mut x0, mut y0, mut x1, mut y1) = (usize::MAX, usize::MAX, 0, 0);
        let mut first = None;
        for (i, (a, b)) in planned
            .chunks_exact(4)
            .zip(whole.chunks_exact(4))
            .enumerate()
        {
            if a[..3] != b[..3] {
                count += 1;
                let (x, y) = (i % w, i / w);
                first.get_or_insert((x, y, [a[0], a[1], a[2]], [b[0], b[1], b[2]]));
                (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
            }
        }
        if count > 0 {
            MISMATCHES.fetch_add(1, Ordering::Relaxed);
            let (_, base, id, _) = scene.frame_damage();
            log::error!(
                "damage audit: frame {id} (base {base}) plan {within:?}, groups kept {kept} drawn {drawn}: \
                 {count} px differ from the whole frame in x {x0}..={x1} y {y0}..={y1}"
            );
            if let Some((x, y, planned, whole)) = first {
                log::error!(
                    "damage audit: first at ({x}, {y}) planned bgr {planned:?} whole {whole:?}; primitives there {:?}",
                    scene.primitives_at(x as f32, y as f32)
                );
            }
        }
        Ok(())
    }

    /// The render target as tightly packed BGRA rows.
    fn read_target(&mut self) -> Result<Vec<u8>> {
        let (w, h) = (self.width, self.height);
        let staging = self.capture_staging()?;
        let ctx = self.dev().device_context.clone();
        let target = self
            .res()
            .render_target
            .as_ref()
            .context("render target missing")?;
        unsafe { ctx.CopyResource(&staging, target) };
        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        unsafe { ctx.Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))? };
        let row = (w * 4) as usize;
        let mut out = vec![0u8; row * h as usize];
        for y in 0..h as usize {
            // KaminIDE patch: SAFETY: the mapped staging texture is w x h
            // BGRA8; each of its rows has RowPitch >= w * 4 readable bytes
            // until Unmap below.
            let src = unsafe {
                std::slice::from_raw_parts(
                    (mapped.pData as *const u8).add(y * mapped.RowPitch as usize),
                    row,
                )
            };
            out[y * row..(y + 1) * row].copy_from_slice(src);
        }
        unsafe { ctx.Unmap(&staging, 0) };
        Ok(out)
    }
}
