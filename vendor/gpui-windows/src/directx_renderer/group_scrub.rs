//! KaminIDE patch: протирка удержанных картинок групп (`group_cache`).
//!
//! Удержанная картинка верна, пока верна та отрисовка, которой её получили.
//! Сбой одной отрисовки (GPU, драйвер, пропущенная зависимость) без кэша
//! исчезал со следующим кадром, а с кэшем он остаётся. Поэтому каждый кадр,
//! в котором удержана хотя бы одна группа, одна удержанная группа по кругу
//! рисуется заново на своё место в атласе. Так ошибка живёт не дольше круга.
//! Старая и новая картинки копируются в staging-текстуры. Сравниваются они
//! позже, когда GPU их отдаст без ожидания (`D3D11_MAP_FLAG_DO_NOT_WAIT`).
//! Расхождение пишется в лог и в счётчик [`group_scrub_mismatches`]: в бою
//! это детектор, в аудите гейта он обязан дать 0.

use super::*;
use std::sync::atomic::{AtomicU64, Ordering};

static MISMATCHES: AtomicU64 = AtomicU64::new(0);

/// Удержанные картинки групп, что не совпали со свежей отрисовкой, с начала процесса.
pub fn group_scrub_mismatches() -> u64 {
    MISMATCHES.load(Ordering::Relaxed)
}

#[derive(Default)]
pub(super) struct Scrub {
    /// Следующая группа круга.
    cursor: usize,
    /// Сравнение, ждущее GPU: размер, группа, кадр.
    pending: Option<(u32, u32, usize, u64)>,
    /// Staging старой и новой картинки, растут по мере надобности.
    staging: Option<(u32, u32, ID3D11Texture2D, ID3D11Texture2D)>,
    /// Расхождения этого рендерера (тесты).
    pub(super) mismatches: u64,
}

impl DirectXRenderer {
    /// Draw group `i` alone into the stage, then into its atlas rectangle.
    pub(super) fn draw_group_alone(&mut self, i: usize, group: &gpui::PaintGroup) -> Result<()> {
        let raw = Region::for_group(group, self.width, self.height)[0];
        self.draw_group_content(group, raw, None)?;
        match gaussian::direct_radius(group.blur_radius) {
            Some(r) => self.gaussian_group(i, group.blur_radius, r, raw)?,
            None => {
                if group.blur_radius > 0.0 {
                    self.blur_group(i, group)?;
                }
                self.save_group_stage(i);
            }
        }
        self.clear_group_stage()
    }

    /// Redraw the next kept group of `scene` (`reuse`) in place, comparing later.
    pub(super) fn scrub_kept_group(&mut self, scene: &Scene, reuse: &[bool]) -> Result<()> {
        self.finish_scrub(false)?;
        let n = reuse.len();
        if self.retained.groups.scrub.pending.is_some() || !reuse.contains(&true) {
            return Ok(());
        }
        let start = self.retained.groups.scrub.cursor % n;
        let Some(i) = (0..n).map(|k| (start + k) % n).find(|&k| reuse[k]) else {
            return Ok(());
        };
        self.retained.groups.scrub.cursor = i + 1;
        let (region, place) = (self.blur.group_regions[i], self.blur.group_place[i]);
        let (old, new) = self.scrub_staging(region.width, region.height)?;
        self.copy_place(&old, place, region);
        self.draw_group_alone(i, &scene.groups[i])?;
        self.copy_place(&new, place, region);
        let frame = scene.frame_damage().2;
        self.retained.groups.scrub.pending = Some((region.width, region.height, i, frame));
        Ok(())
    }

    /// Compare the pending pair; with `wait`, block until the GPU has it.
    pub(super) fn finish_scrub(&mut self, wait: bool) -> Result<()> {
        let Some((w, h, group, frame)) = self.retained.groups.scrub.pending else {
            return Ok(());
        };
        let Some((_, _, old, new)) = self.retained.groups.scrub.staging.clone() else {
            self.retained.groups.scrub.pending = None;
            return Ok(());
        };
        let ctx = self.dev().device_context.clone();
        let flags = if wait { 0 } else { D3D11_MAP_FLAG_DO_NOT_WAIT.0 as u32 };
        let (mut a, mut b) = (D3D11_MAPPED_SUBRESOURCE::default(), D3D11_MAPPED_SUBRESOURCE::default());
        if unsafe { ctx.Map(&old, 0, D3D11_MAP_READ, flags, Some(&mut a)) }.is_err() {
            return Ok(());
        }
        if unsafe { ctx.Map(&new, 0, D3D11_MAP_READ, flags, Some(&mut b)) }.is_err() {
            unsafe { ctx.Unmap(&old, 0) };
            return Ok(());
        }
        let row = (w * 4) as usize;
        let mut differ = 0usize;
        for y in 0..h as usize {
            // KaminIDE patch: SAFETY: both staging textures are at least w x h
            // BGRA8 and stay mapped until the Unmap calls below; each row has
            // RowPitch >= w * 4 readable bytes.
            let (x, z) = unsafe {
                (
                    std::slice::from_raw_parts((a.pData as *const u8).add(y * a.RowPitch as usize), row),
                    std::slice::from_raw_parts((b.pData as *const u8).add(y * b.RowPitch as usize), row),
                )
            };
            differ += x.chunks_exact(4).zip(z.chunks_exact(4)).filter(|(p, q)| p != q).count();
        }
        unsafe {
            ctx.Unmap(&new, 0);
            ctx.Unmap(&old, 0);
        }
        self.retained.groups.scrub.pending = None;
        if differ > 0 {
            MISMATCHES.fetch_add(1, Ordering::Relaxed);
            self.retained.groups.scrub.mismatches += 1;
            log::error!("GROUP_SCRUB_MISMATCH: kept image of group {group} (frame {frame}, {w}x{h}) differs from its redraw in {differ} px; replaced");
        }
        Ok(())
    }

    /// Copy the atlas rectangle of `place` (size of `region`) to (0, 0) of `dst`.
    fn copy_place(&self, dst: &ID3D11Texture2D, place: group_atlas::Place, region: Region) {
        let area = D3D11_BOX {
            left: place.x,
            top: place.y,
            front: 0,
            right: place.x + region.width,
            bottom: place.y + region.height,
            back: 1,
        };
        let page = &self.blur.groups[place.page].texture;
        unsafe { self.dev().device_context.CopySubresourceRegion(dst, 0, 0, 0, 0, page, 0, Some(&area)) };
    }

    /// The staging pair, at least `w` x `h`, in the format of the atlas pages.
    fn scrub_staging(&mut self, w: u32, h: u32) -> Result<(ID3D11Texture2D, ID3D11Texture2D)> {
        if let Some((sw, sh, a, b)) = &self.retained.groups.scrub.staging
            && *sw >= w
            && *sh >= h
        {
            return Ok((a.clone(), b.clone()));
        }
        let old = self.retained.groups.scrub.staging.as_ref().map_or((0, 0), |s| (s.0, s.1));
        let (sw, sh) = (w.max(old.0).next_multiple_of(64), h.max(old.1).next_multiple_of(64));
        let mut page = D3D11_TEXTURE2D_DESC::default();
        unsafe { self.blur.groups[0].texture.GetDesc(&mut page) };
        let desc = D3D11_TEXTURE2D_DESC {
            Width: sw,
            Height: sh,
            MipLevels: 1,
            ArraySize: 1,
            Format: page.Format,
            SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
            Usage: D3D11_USAGE_STAGING,
            BindFlags: 0,
            CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
            MiscFlags: 0,
        };
        let device = self.dev().device.clone();
        let make = || -> Result<ID3D11Texture2D> {
            let mut t = None;
            unsafe { device.CreateTexture2D(&desc, None, Some(&mut t))? };
            t.context("scrub staging texture")
        };
        let (a, b) = (make()?, make()?);
        self.retained.groups.scrub.staging = Some((sw, sh, a.clone(), b.clone()));
        Ok((a, b))
    }
}
