//! KaminIDE patch: the render target keeps the last frame; a new frame is
//! drawn only where its scene paints otherwise (`Scene::frame_damage`).
//!
//! Every target buffer (one offscreen texture, or each back buffer of the
//! flip-model swap chain) carries the damage accumulated since it was last
//! drawn: a frame unites its own damage into every buffer and redraws the
//! current buffer only within that buffer's union, scissored. Outside it the
//! buffer already holds the pixels this frame paints. Chromium keeps the same
//! per-buffer union for partial swap (`DirectRenderer::DrawFrame`, buffer
//! age); WebRender keeps it per tile (`dirty_rect` of each picture tile).
//!
//! Anything the scene does not describe forces a full frame: another size,
//! background or opaque backdrop colour, an atlas change that may alter the
//! pixels of a referenced tile, a sampled external texture whose change was
//! not reported as a rectangle (`external_damage.rs`), a frame that was
//! not compared with the one drawn last, or a failed draw.

use super::*;
mod full_by;
use full_by::FullBy;

/// What the target holds apart from the damage: the inputs of a frame that
/// the scene comparison does not cover.
#[derive(Clone, Copy, PartialEq)]
struct Target {
    size: (u32, u32),
    background: WindowBackgroundAppearance,
    backdrop: Option<[u32; 4]>,
    epoch: u64,
}

#[derive(Default)]
pub(super) struct Retained {
    /// Frame number (`Scene::frame_damage`) and target inputs of the last
    /// scene whose damage was united into `pending`.
    last: Option<(u64, Target)>,
    /// Damage since each buffer was last drawn, by back buffer index.
    pending: [SceneDamage; BUFFER_COUNT],
    /// Staging texture of frame capture, kept while the size is the same.
    staging: Option<(u32, u32, ID3D11Texture2D)>,
    /// How frame capture waits for the GPU (`gpu_wait`), kept with the device.
    pub(super) gpu_wait: super::gpu_wait::GpuWait,
    /// The frame rasterizer state with the scissor test on.
    scissor: Option<ID3D11RasterizerState>,
    /// A pass of this frame failed and was skipped: the frame is incomplete.
    pub(super) failed: bool,
    /// The group images kept in the group atlas between frames.
    pub(super) groups: group_cache::GroupCache,
    /// This frame continues the last one: `Some(true)` the same scene again,
    /// `Some(false)` compared with it (`Scene::group_base`).
    pub(super) chain: Option<(bool, u64)>,
    /// Why the last planned frame would be drawn whole.
    pub(super) full_by: FullBy,
    /// Frames the swap chain took (`present`): which buffer comes next.
    pub(super) presents: usize,
}

/// How the current frame reaches the target.
pub(super) enum Plan {
    /// The target and, when shown, the screen already hold this frame.
    Keep,
    /// Draw everything.
    Full,
    /// Draw within the rectangle only.
    Within(Region),
}

/// `GPUI_DAMAGE_REDRAW=0` draws every frame whole (comparison runs, fallback).
fn enabled() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var_os("GPUI_DAMAGE_REDRAW").is_none_or(|v| v != "0"))
}

/// `GPUI_DAMAGE_FULL=1` draws every frame whole but keeps the retained group
/// images (measuring the full-frame composite without the group renders).
fn force_full() -> bool {
    static FULL: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *FULL.get_or_init(|| std::env::var_os("GPUI_DAMAGE_FULL").is_some_and(|v| v == "1"))
}

impl DirectXRenderer {
    /// Buffer this frame draws into: the flip-sequential chain hands them out in turn
    /// per present (D3D11 `GetCurrentBackBufferIndex` did not follow; stand audit).
    fn buffer_slot(&self) -> Option<usize> {
        let chain = self.res().swap_chain.is_some();
        Some(if chain { self.retained.presents % BUFFER_COUNT } else { 0 })
    }

    /// Unite this frame's damage into every buffer and choose what to draw.
    pub(super) fn plan_frame(
        &mut self,
        scene: &Scene,
        background: WindowBackgroundAppearance,
    ) -> Plan {
        let (damage, base, id, size) = scene.frame_damage();
        let backdrop = backdrop::opaque_backdrop(scene, self.width, self.height);
        let target = Target {
            size: (self.width, self.height),
            background,
            backdrop: backdrop.map(|c| c.map(f32::to_bits)),
            epoch: self.atlas.epoch(),
        };
        let external = self.atlas.external_damage(scene);
        let comparable = id != 0 && size == target.size && external != SceneDamage::Unknown;
        self.retained.chain = match self.retained.last {
            Some((last, t)) if comparable && t == target && last == id => Some((true, id)),
            Some((last, t)) if comparable && t == target && last == base => Some((false, base)),
            _ => None,
        };
        let frame = match self.retained.last {
            Some((last, t)) if comparable && t == target && last == id => SceneDamage::Unchanged,
            Some((last, t)) if comparable && t == target && last == base => damage,
            _ => SceneDamage::Unknown,
        };
        let retarget = self.retained.last.is_some_and(|(_, t)| t != target);
        self.retained.full_by = FullBy::of(external, retarget, damage, frame);
        let frame = frame.union(external);
        self.retained.last = Some((id, target));
        for pending in &mut self.retained.pending {
            *pending = pending.union(frame);
        }
        let Some(slot) = self.buffer_slot().filter(|_| enabled()) else {
            self.retained.full_by = FullBy::Disabled;
            self.forget_frames();
            return Plan::Full;
        };
        if force_full() {
            self.retained.full_by = FullBy::Disabled;
            return Plan::Full;
        }
        // The screen shows this frame already; capture of a swap chain reads
        // the back buffer, which may hold an older one.
        let capture_reads_back = self.capture.is_some() && self.res().swap_chain.is_some();
        if frame == SceneDamage::Unchanged && !self.unpresented && !capture_reads_back {
            return Plan::Keep;
        }
        match self.retained.pending[slot] {
            SceneDamage::Unchanged => Plan::Keep,
            SceneDamage::Unknown => Plan::Full,
            SceneDamage::Rect {
                x,
                y,
                width,
                height,
            } => {
                let region = Region {
                    x,
                    y,
                    width: width.min(self.width.saturating_sub(x)).max(1),
                    height: height.min(self.height.saturating_sub(y)).max(1),
                };
                if region == Region::full(self.width, self.height) {
                    self.retained.full_by = FullBy::Region;
                    Plan::Full
                } else {
                    Plan::Within(region)
                }
            }
        }
    }

    /// The current buffer now holds the frame.
    pub(super) fn frame_drawn(&mut self, plan: &Plan) {
        if std::mem::take(&mut self.retained.failed) {
            // An incomplete frame is not a base for the next one.
            self.forget_frames();
        } else if !matches!(plan, Plan::Keep)
            && let Some(slot) = self.buffer_slot()
        {
            self.retained.pending[slot] = SceneDamage::Unchanged;
        }
    }

    /// No buffer holds a known frame (resize, device loss, a failed or foreign draw).
    pub(super) fn forget_frames(&mut self) {
        self.retained.last = None;
        self.retained.chain = None;
        self.retained.groups.valid.clear();
        self.retained.pending = [SceneDamage::Unknown; BUFFER_COUNT];
    }

    /// The device is lost: drop its objects along with the kept frames.
    pub(super) fn reset_retained(&mut self) {
        self.forget_frames();
        self.retained.staging = None;
        self.retained.gpu_wait = Default::default();
        self.retained.scissor = None;
        self.retained.groups.scrub = Default::default();
        self.retained.failed = false;
    }

    /// Switch the frame rasterizer to the scissor test on `region`; returns
    /// the state to restore.
    pub(super) fn begin_scissor(&mut self, region: Region) -> Result<ID3D11RasterizerState> {
        let device = self.dev().device.clone();
        let dc = self.dev().device_context.clone();
        let original = unsafe { dc.RSGetState()? };
        if self.retained.scissor.is_none() {
            let mut desc = D3D11_RASTERIZER_DESC::default();
            unsafe { original.GetDesc(&mut desc) };
            desc.ScissorEnable = true.into();
            unsafe { device.CreateRasterizerState(&desc, Some(&mut self.retained.scissor))? };
        }
        let rect = windows::Win32::Foundation::RECT {
            left: region.x as i32,
            top: region.y as i32,
            right: (region.x + region.width) as i32,
            bottom: (region.y + region.height) as i32,
        };
        unsafe {
            dc.RSSetState(self.retained.scissor.as_ref());
            dc.RSSetScissorRects(Some(&[rect]));
        }
        Ok(original)
    }

    /// The capture staging texture for the current size, created once per size.
    pub(super) fn capture_staging(&mut self) -> Result<ID3D11Texture2D> {
        let (w, h) = (self.width, self.height);
        if let Some((sw, sh, texture)) = &self.retained.staging
            && (*sw, *sh) == (w, h)
        {
            return Ok(texture.clone());
        }
        let desc = D3D11_TEXTURE2D_DESC {
            Width: w,
            Height: h,
            MipLevels: 1,
            ArraySize: 1,
            Format: RENDER_TARGET_FORMAT,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Usage: D3D11_USAGE_STAGING,
            BindFlags: 0,
            CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
            MiscFlags: 0,
        };
        let mut staging: Option<ID3D11Texture2D> = None;
        unsafe {
            self.dev()
                .device
                .CreateTexture2D(&desc, None, Some(&mut staging))?
        };
        let staging = staging.context("staging texture")?;
        self.retained.staging = Some((w, h, staging.clone()));
        Ok(staging)
    }
}
