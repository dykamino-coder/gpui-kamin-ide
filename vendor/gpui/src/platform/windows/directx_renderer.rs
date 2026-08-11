use std::{
    mem::ManuallyDrop,
    sync::{Arc, OnceLock},
};

use ::util::ResultExt;
use anyhow::{Context, Result};
use windows::{
    Win32::{
        Foundation::HWND,
        Graphics::{
            Direct3D::*,
            Direct3D11::*,
            DirectComposition::*,
            DirectWrite::*,
            Dxgi::{Common::*, *},
        },
    },
    core::Interface,
};

use crate::{
    platform::windows::directx_renderer::shader_resources::{
        RawShaderBytes, ShaderModule, ShaderTarget,
    },
    *,
};

pub(crate) const DISABLE_DIRECT_COMPOSITION: &str = "GPUI_DISABLE_DIRECT_COMPOSITION";
const RENDER_TARGET_FORMAT: DXGI_FORMAT = DXGI_FORMAT_B8G8R8A8_UNORM;
// This configuration is used for MSAA rendering on paths only, and it's guaranteed to be supported by DirectX 11.
const PATH_MULTISAMPLE_COUNT: u32 = 4;

pub(crate) struct FontInfo {
    pub gamma_ratios: [f32; 4],
    pub grayscale_enhanced_contrast: f32,
}

pub(crate) struct DirectXRenderer {
    hwnd: HWND,
    atlas: Arc<DirectXAtlas>,
    devices: ManuallyDrop<DirectXRendererDevices>,
    resources: ManuallyDrop<DirectXResources>,
    globals: DirectXGlobalElements,
    pipelines: DirectXRenderPipelines,
    direct_composition: Option<DirectComposition>,
    font_info: &'static FontInfo,
    /// KaminIDE patch: скретч backdrop blur (см. draw_surfaces).
    blur: BlurScratch,
    /// KaminIDE patch: куда идёт отрисовка сейчас. Пока рисуется группа
    /// (`Window::paint_group`), это её буфер, а не бэкбуфер — иначе проходы,
    /// возвращающие цель кадра (пути через MSAA), уводили бы примитивы
    /// группы прямо в кадр.
    group_target: Option<[Option<ID3D11RenderTargetView>; 1]>,
    /// KaminIDE patch: блендер для картинок с уже умноженным на прозрачность
    /// цветом (буферы групп). Обычный блендер умножает на неё второй раз —
    /// края группы уходили в чёрный ореол.
    blend_premultiplied: Option<ID3D11BlendState>,
    /// KaminIDE patch: блендер «писать поверх» — для групп, у которых
    /// смешивание с кадром посчитано в шейдере.
    blend_replace: Option<ID3D11BlendState>,
}

/// KaminIDE patch: ресурсы backdrop blur. Копия бэкбуфера (сэмплить
/// связанный RTV нельзя) + каскад даунсемплов области + свой GlobalParams
/// (вьюпорты проходов не равны вьюпорту кадра).
#[derive(Default)]
struct BlurScratch {
    copy: Option<BlurTexture>,
    down: Vec<BlurTexture>,
    globals: [Option<ID3D11Buffer>; 1],
    /// KaminIDE patch: буферы групп, по два на группу — сырая картинка и её
    /// размытая копия (сэмплировать связанную с целью текстуру нельзя).
    groups: Vec<BlurTexture>,
    /// Готовый буфер каждой группы: сырой или размытый.
    group_slots: Vec<usize>,
    /// Режим смешивания каждой группы (`mix-blend-mode`).
    group_blend: Vec<u32>,
    /// Обрезающий многоугольник каждой группы: вершины парами и их число.
    group_poly: Vec<([[f32; 4]; 4], u32)>,
}

struct BlurTexture {
    width: u32,
    height: u32,
    texture: ID3D11Texture2D,
    rtv: [Option<ID3D11RenderTargetView>; 1],
    srv: [Option<ID3D11ShaderResourceView>; 1],
}

/// Инстанс blur-прохода — зеркало `struct BlurQuad` в shaders.hlsl.
#[derive(Clone, Copy, Default)]
#[repr(C)]
struct BlurQuad {
    bounds: [f32; 4],
    content_mask: [f32; 4],
    corner_radii: [f32; 4],
    src_origin: [f32; 2],
    src_scale: [f32; 2],
    texel: [f32; 2],
    blur_pass: f32,
    pad: f32,
    blend_mode: u32,
    /// Число вершин обрезающего многоугольника (0 — не обрезать).
    poly_count: u32,
    pad2: [f32; 2],
    /// Вершины парами: (x0, y0, x1, y1).
    poly: [[f32; 4]; 4],
}

fn create_blur_texture(
    device: &ID3D11Device,
    width: u32,
    height: u32,
    render_target: bool,
) -> Result<BlurTexture> {
    let bind = if render_target {
        D3D11_BIND_SHADER_RESOURCE.0 | D3D11_BIND_RENDER_TARGET.0
    } else {
        D3D11_BIND_SHADER_RESOURCE.0
    };
    let desc = D3D11_TEXTURE2D_DESC {
        Width: width,
        Height: height,
        MipLevels: 1,
        ArraySize: 1,
        Format: RENDER_TARGET_FORMAT,
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        Usage: D3D11_USAGE_DEFAULT,
        BindFlags: bind as u32,
        CPUAccessFlags: 0,
        MiscFlags: 0,
    };
    let mut texture: Option<ID3D11Texture2D> = None;
    unsafe { device.CreateTexture2D(&desc, None, Some(&mut texture))? };
    let texture = texture.unwrap();
    let mut srv = None;
    unsafe { device.CreateShaderResourceView(&texture, None, Some(&mut srv))? };
    let rtv = if render_target {
        let mut v = None;
        unsafe { device.CreateRenderTargetView(&texture, None, Some(&mut v))? };
        [v]
    } else {
        [None]
    };
    Ok(BlurTexture {
        width,
        height,
        texture,
        rtv,
        srv: [srv],
    })
}

/// KaminIDE patch: один даунсемпл-проход каскада blur (квад на весь dest,
/// сэмпл src по uv-области, вьюпорт dest, tent-фильтр в шейдере).
#[allow(clippy::too_many_arguments)]
fn blur_down_pass(
    device: &ID3D11Device,
    dc: &ID3D11DeviceContext,
    pipeline: &mut PipelineState<BlurQuad>,
    globals: &[Option<ID3D11Buffer>; 1],
    sampler: &[Option<ID3D11SamplerState>; 1],
    font_info: &FontInfo,
    dest: &BlurTexture,
    src: &[Option<ID3D11ShaderResourceView>; 1],
    src_size: (f32, f32),
    uv_origin: [f32; 2],
    uv_scale: [f32; 2],
    keep_alpha: bool,
    blend: Option<&ID3D11BlendState>,
) -> Result<()> {
    let viewport = [D3D11_VIEWPORT {
        TopLeftX: 0.0,
        TopLeftY: 0.0,
        Width: dest.width as f32,
        Height: dest.height as f32,
        MinDepth: 0.0,
        MaxDepth: 1.0,
    }];
    update_buffer(
        dc,
        globals[0].as_ref().unwrap(),
        &[GlobalParams {
            gamma_ratios: font_info.gamma_ratios,
            viewport_size: [dest.width as f32, dest.height as f32],
            grayscale_enhanced_contrast: font_info.grayscale_enhanced_contrast,
            _pad: 0,
        }],
    )?;
    let quad = BlurQuad {
        bounds: [0.0, 0.0, dest.width as f32, dest.height as f32],
        content_mask: [0.0, 0.0, dest.width as f32, dest.height as f32],
        corner_radii: [0.0; 4],
        src_origin: uv_origin,
        src_scale: uv_scale,
        texel: [1.0 / src_size.0, 1.0 / src_size.1],
        // KaminIDE patch: у группы прозрачность — часть картинки, гасить её
        // до единицы нельзя (у копии кадра она и так единица).
        blur_pass: if keep_alpha { 2.0 } else { 1.0 },
        pad: 0.0,
        blend_mode: 0,
        poly_count: 0,
        pad2: [0.0; 2],
        poly: [[0.0; 4]; 4],
    };
    pipeline.update_buffer(device, dc, &[quad])?;
    unsafe {
        dc.OMSetRenderTargets(Some(&dest.rtv), None);
    }
    pipeline.draw_with_texture_blended(dc, src, &viewport, globals, sampler, 1, blend)
}

/// Direct3D objects
#[derive(Clone)]
pub(crate) struct DirectXRendererDevices {
    pub(crate) adapter: IDXGIAdapter1,
    pub(crate) dxgi_factory: IDXGIFactory6,
    pub(crate) device: ID3D11Device,
    pub(crate) device_context: ID3D11DeviceContext,
    dxgi_device: Option<IDXGIDevice>,
}

/// KaminIDE patch (#76): HANDLE waitable-объекта свапчейна (latency 1);
/// закрывается при пересоздании ресурсов (device lost).
struct FrameLatencyGate(windows::Win32::Foundation::HANDLE);
impl Drop for FrameLatencyGate {
    fn drop(&mut self) {
        unsafe {
            let _ = windows::Win32::Foundation::CloseHandle(self.0);
        }
    }
}

struct DirectXResources {
    // Direct3D rendering objects
    swap_chain: IDXGISwapChain1,
    /// KaminIDE patch (#76): см. FrameLatencyGate.
    frame_gate: Option<FrameLatencyGate>,
    render_target: ManuallyDrop<ID3D11Texture2D>,
    render_target_view: [Option<ID3D11RenderTargetView>; 1],

    // Path intermediate textures (with MSAA)
    path_intermediate_texture: ID3D11Texture2D,
    path_intermediate_srv: [Option<ID3D11ShaderResourceView>; 1],
    path_intermediate_msaa_texture: ID3D11Texture2D,
    path_intermediate_msaa_view: [Option<ID3D11RenderTargetView>; 1],

    // Cached window size and viewport
    width: u32,
    height: u32,
    viewport: [D3D11_VIEWPORT; 1],
}

struct DirectXRenderPipelines {
    shadow_pipeline: PipelineState<Shadow>,
    quad_pipeline: PipelineState<Quad>,
    path_rasterization_pipeline: PipelineState<PathRasterizationSprite>,
    path_sprite_pipeline: PipelineState<PathSprite>,
    underline_pipeline: PipelineState<Underline>,
    mono_sprites: PipelineState<MonochromeSprite>,
    poly_sprites: PipelineState<PolychromeSprite>,
    /// KaminIDE patch: backdrop blur (даунсемплы + композит с маской углов).
    blur_pipeline: PipelineState<BlurQuad>,
}

struct DirectXGlobalElements {
    global_params_buffer: [Option<ID3D11Buffer>; 1],
    sampler: [Option<ID3D11SamplerState>; 1],
}

struct DirectComposition {
    comp_device: IDCompositionDevice,
    comp_target: IDCompositionTarget,
    comp_visual: IDCompositionVisual,
    comp_root: IDCompositionVisual,
}

impl DirectXRendererDevices {
    pub(crate) fn new(
        directx_devices: &DirectXDevices,
        disable_direct_composition: bool,
    ) -> Result<ManuallyDrop<Self>> {
        let DirectXDevices {
            adapter,
            dxgi_factory,
            device,
            device_context,
        } = directx_devices;
        let dxgi_device = if disable_direct_composition {
            None
        } else {
            Some(device.cast().context("Creating DXGI device")?)
        };

        Ok(ManuallyDrop::new(Self {
            adapter: adapter.clone(),
            dxgi_factory: dxgi_factory.clone(),
            device: device.clone(),
            device_context: device_context.clone(),
            dxgi_device,
        }))
    }
}

impl DirectXRenderer {
    pub(crate) fn new(
        hwnd: HWND,
        directx_devices: &DirectXDevices,
        disable_direct_composition: bool,
    ) -> Result<Self> {
        if disable_direct_composition {
            log::info!("Direct Composition is disabled.");
        }

        let devices = DirectXRendererDevices::new(directx_devices, disable_direct_composition)
            .context("Creating DirectX devices")?;
        let atlas = Arc::new(DirectXAtlas::new(&devices.device, &devices.device_context));

        let resources = DirectXResources::new(&devices, 1, 1, hwnd, disable_direct_composition)
            .context("Creating DirectX resources")?;
        let globals = DirectXGlobalElements::new(&devices.device)
            .context("Creating DirectX global elements")?;
        let pipelines = DirectXRenderPipelines::new(&devices.device)
            .context("Creating DirectX render pipelines")?;

        let direct_composition = if disable_direct_composition {
            None
        } else {
            let composition = DirectComposition::new(devices.dxgi_device.as_ref().unwrap(), hwnd)
                .context("Creating DirectComposition")?;
            composition
                .set_swap_chain(&resources.swap_chain)
                .context("Setting swap chain for DirectComposition")?;
            Some(composition)
        };

        Ok(DirectXRenderer {
            hwnd,
            atlas,
            devices,
            resources,
            globals,
            pipelines,
            direct_composition,
            font_info: Self::get_font_info(),
            blur: BlurScratch::default(),
            group_target: None,
            blend_premultiplied: None,
            blend_replace: None,
        })
    }

    /// KaminIDE patch: raw ID3D11Device окна (AddRef) — по нему открывается
    /// общий handle текстуры от CEF.
    pub fn d3d_device_raw(&self) -> *mut std::ffi::c_void {
        use windows::core::Interface;
        self.devices.device.clone().into_raw()
    }

    /// KaminIDE patch: raw ID3D11DeviceContext окна (AddRef). Нужен, чтобы
    /// скопировать кадр CEF в свою текстуру на ТОМ ЖЕ потоке, где идёт
    /// отрисовка: контекст D3D11 не потокобезопасен.
    pub fn d3d_context_raw(&self) -> *mut std::ffi::c_void {
        use windows::core::Interface;
        self.devices.device_context.clone().into_raw()
    }

    /// KaminIDE patch: положить чужую текстуру D3D11 в атлас (кадр CEF).
    pub fn register_external_texture(
        &self,
        texture: ID3D11Texture2D,
        size: Size<DevicePixels>,
    ) -> Option<AtlasTile> {
        self.atlas.register_external(texture, size)
    }

    /// KaminIDE patch: заменить содержимое записи атласа новой текстурой.
    pub fn update_external_texture(&self, id: AtlasTextureId, texture: ID3D11Texture2D) -> bool {
        self.atlas.update_external(id, texture)
    }

    /// KaminIDE patch: забыть чужую текстуру.
    pub fn unregister_external_texture(&self, id: AtlasTextureId) {
        self.atlas.unregister_external(id);
    }

    pub(crate) fn sprite_atlas(&self) -> Arc<dyn PlatformAtlas> {
        self.atlas.clone()
    }

    fn pre_draw(&self) -> Result<()> {
        update_buffer(
            &self.devices.device_context,
            self.globals.global_params_buffer[0].as_ref().unwrap(),
            &[GlobalParams {
                gamma_ratios: self.font_info.gamma_ratios,
                viewport_size: [
                    self.resources.viewport[0].Width,
                    self.resources.viewport[0].Height,
                ],
                grayscale_enhanced_contrast: self.font_info.grayscale_enhanced_contrast,
                _pad: 0,
            }],
        )?;
        unsafe {
            self.devices.device_context.ClearRenderTargetView(
                self.resources.render_target_view[0].as_ref().unwrap(),
                &[0.0; 4],
            );
            self.devices
                .device_context
                .OMSetRenderTargets(Some(&self.resources.render_target_view), None);
            self.devices
                .device_context
                .RSSetViewports(Some(&self.resources.viewport));
        }
        Ok(())
    }

    #[inline]
    fn present(&mut self) -> Result<()> {
        // KaminIDE patch (#76): БЕЗ dcomp-Commit и без ожиданий. dcomp-дерево
        // статично после set_swap_chain (underlay-эпоха WebView2 удалена как
        // мёртвая), а прежний пер-кадровый commit() делал
        // WaitForCommitCompletion — блокировку UI-потока до завершения
        // композиции DWM: на RDP-компоузере это 30-90 мс НА КАЖДЫЙ КАДР
        // (замер diag.log юзера: «презент 397-911 мс»/с) — корень задержки
        // ввода ~1.5с на слабой машине.
        let result = unsafe { self.resources.swap_chain.Present(0, DXGI_PRESENT(0)) };
        result.ok().context("Presenting swap chain failed")
    }

    pub(crate) fn handle_device_lost(&mut self, directx_devices: &DirectXDevices) {
        try_to_recover_from_device_lost(
            || {
                self.handle_device_lost_impl(directx_devices)
                    .context("DirectXRenderer handling device lost")
            },
            |_| {},
            || {
                log::error!(
                    "DirectXRenderer failed to recover from device lost after multiple attempts"
                );
                // Do something here?
                // At this point, the device loss is considered unrecoverable.
            },
        );
    }

    fn handle_device_lost_impl(&mut self, directx_devices: &DirectXDevices) -> Result<()> {
        let disable_direct_composition = self.direct_composition.is_none();

        unsafe {
            #[cfg(debug_assertions)]
            report_live_objects(&self.devices.device)
                .context("Failed to report live objects after device lost")
                .log_err();

            ManuallyDrop::drop(&mut self.resources);
            self.devices.device_context.OMSetRenderTargets(None, None);
            self.devices.device_context.ClearState();
            self.devices.device_context.Flush();

            #[cfg(debug_assertions)]
            report_live_objects(&self.devices.device)
                .context("Failed to report live objects after device lost")
                .log_err();

            drop(self.direct_composition.take());
            ManuallyDrop::drop(&mut self.devices);
        }

        let devices = DirectXRendererDevices::new(directx_devices, disable_direct_composition)
            .context("Recreating DirectX devices")?;
        let resources = DirectXResources::new(
            &devices,
            self.resources.width,
            self.resources.height,
            self.hwnd,
            disable_direct_composition,
        )?;
        let globals = DirectXGlobalElements::new(&devices.device)?;
        let pipelines = DirectXRenderPipelines::new(&devices.device)?;

        let direct_composition = if disable_direct_composition {
            None
        } else {
            let composition =
                DirectComposition::new(devices.dxgi_device.as_ref().unwrap(), self.hwnd)?;
            composition.set_swap_chain(&resources.swap_chain)?;
            Some(composition)
        };

        self.atlas
            .handle_device_lost(&devices.device, &devices.device_context);
        self.devices = devices;
        self.resources = resources;
        self.globals = globals;
        self.pipelines = pipelines;
        self.direct_composition = direct_composition;

        unsafe {
            self.devices
                .device_context
                .OMSetRenderTargets(Some(&self.resources.render_target_view), None);
        }
        Ok(())
    }

    /// KaminIDE patch: блендер для буферов групп (создаётся по требованию).
    fn premultiplied_blend(&mut self) -> Result<ID3D11BlendState> {
        if self.blend_premultiplied.is_none() {
            self.blend_premultiplied =
                Some(create_blend_state_premultiplied(&self.devices.device)?);
        }
        Ok(self.blend_premultiplied.clone().unwrap())
    }

    /// KaminIDE patch: писать результат поверх — смешивание уже посчитано.
    fn replace_blend(&mut self) -> Result<ID3D11BlendState> {
        if self.blend_replace.is_none() {
            self.blend_replace = Some(create_blend_state_replace(&self.devices.device)?);
        }
        Ok(self.blend_replace.clone().unwrap())
    }

    /// KaminIDE patch: цель отрисовки сейчас — буфер группы или кадр.
    fn current_target(&self) -> &[Option<ID3D11RenderTargetView>; 1] {
        self.group_target
            .as_ref()
            .unwrap_or(&self.resources.render_target_view)
    }

    pub(crate) fn draw(&mut self, scene: &Scene) -> Result<()> {
        // KaminIDE patch (#76): буфер свапчейна занят компоузером (DWM/RDP
        // ещё кодирует прошлый кадр) — НЕ рисовать и НЕ блокироваться:
        // пропускаем кадр и просим низкоприоритетный WM_PAINT — он придёт
        // ПОСЛЕ обработки очереди ввода, содержимое догонит без лага букв.
        if let Some(gate) = self.resources.frame_gate.as_ref() {
            use windows::Win32::Foundation::WAIT_TIMEOUT;
            use windows::Win32::System::Threading::WaitForSingleObject;
            let wait = unsafe { WaitForSingleObject(gate.0, 0) };
            if wait == WAIT_TIMEOUT {
                unsafe {
                    let _ = windows::Win32::Graphics::Gdi::InvalidateRect(
                        Some(self.hwnd),
                        None,
                        false,
                    );
                }
                return Ok(());
            }
        }
        self.pre_draw()?;
        // KaminIDE patch: группы рисуются в свои буферы ДО кадра — в кадре
        // от них остаётся метка, по которой готовый буфер и композитится.
        if let Err(e) = self.render_groups(scene) {
            log::warn!("paint group failed: {e}");
        }
        self.draw_scene(scene)?;
        self.present()
    }

    /// KaminIDE patch: примитивы одной сцены — кадра или группы.
    fn draw_scene(&mut self, scene: &Scene) -> Result<()> {
        for batch in scene.batches() {
            match batch {
                PrimitiveBatch::Shadows(shadows) => self.draw_shadows(shadows),
                PrimitiveBatch::Quads(quads) => self.draw_quads(quads),
                PrimitiveBatch::Paths(paths) => {
                    self.draw_paths_to_intermediate(paths)?;
                    self.draw_paths_from_intermediate(paths)
                }
                PrimitiveBatch::Underlines(underlines) => self.draw_underlines(underlines),
                PrimitiveBatch::MonochromeSprites {
                    texture_id,
                    sprites,
                } => self.draw_monochrome_sprites(texture_id, sprites),
                PrimitiveBatch::PolychromeSprites {
                    texture_id,
                    sprites,
                } => self.draw_polychrome_sprites(texture_id, sprites),
                PrimitiveBatch::Surfaces(surfaces) => self.draw_surfaces(surfaces),
            }.context(format!("scene too large: {} paths, {} shadows, {} quads, {} underlines, {} mono, {} poly, {} surfaces",
                    scene.paths.len(),
                    scene.shadows.len(),
                    scene.quads.len(),
                    scene.underlines.len(),
                    scene.monochrome_sprites.len(),
                    scene.polychrome_sprites.len(),
                    scene.surfaces.len(),))?;
        }
        Ok(())
    }

    /// KaminIDE patch: нарисовать каждую группу в свой буфер.
    ///
    /// Эффектам вроде размытия поддерева нужна готовая картинка целиком,
    /// поэтому дети группы идут не в кадр, а в свою текстуру размером с окно
    /// — тогда координаты примитивов остаются кадровыми и ничего пересчитывать
    /// не нужно.
    fn render_groups(&mut self, scene: &Scene) -> Result<()> {
        if scene.groups.is_empty() {
            self.blur.groups.clear();
            self.blur.group_slots.clear();
            self.blur.group_blend.clear();
            self.blur.group_poly.clear();
            return Ok(());
        }
        let device = self.devices.device.clone();
        let dc = self.devices.device_context.clone();
        let (vw, vh) = (self.resources.width, self.resources.height);

        let need = scene.groups.len() * 2;
        if self.blur.groups.len() < need
            || self
                .blur
                .groups
                .first()
                .is_some_and(|t| t.width != vw || t.height != vh)
        {
            self.blur.groups.clear();
            for _ in 0..need {
                self.blur
                    .groups
                    .push(create_blur_texture(&device, vw, vh, true)?);
            }
        }
        self.blur.group_slots.clear();
        self.blur.group_blend.clear();
        self.blur.group_poly.clear();

        for (i, group) in scene.groups.iter().enumerate() {
            let raw_rtv = self.blur.groups[i * 2].rtv.clone();
            unsafe {
                dc.ClearRenderTargetView(raw_rtv[0].as_ref().unwrap(), &[0.0; 4]);
                dc.OMSetRenderTargets(Some(&raw_rtv), None);
                dc.RSSetViewports(Some(&self.resources.viewport));
            }
            self.group_target = Some(raw_rtv);
            let drawn = self.draw_scene(&group.scene);
            self.group_target = None;
            drawn?;

            let slot = if group.blur_radius > 0.0 {
                self.blur_group(i, group)?;
                i * 2 + 1
            } else {
                i * 2
            };
            self.blur.group_slots.push(slot);
            self.blur.group_blend.push(group.blend);
            let mut poly = [[0.0f32; 4]; 4];
            for (i, point) in group.polygon.iter().take(8).enumerate() {
                poly[i / 2][(i % 2) * 2] = point.x.0;
                poly[i / 2][(i % 2) * 2 + 1] = point.y.0;
            }
            self.blur
                .group_poly
                .push((poly, group.polygon.len().min(8) as u32));
        }

        // Вернуть состояние кадра: цель и вьюпорт меняли проходы групп.
        unsafe {
            dc.OMSetRenderTargets(Some(&self.resources.render_target_view), None);
            dc.RSSetViewports(Some(&self.resources.viewport));
        }
        self.restore_frame_globals()
    }

    /// KaminIDE patch: размыть готовый буфер группы (`filter: blur(N)`).
    fn blur_group(&mut self, index: usize, group: &crate::scene::PaintGroup) -> Result<()> {
        let device = self.devices.device.clone();
        let dc = self.devices.device_context.clone();
        let (vw, vh) = (self.resources.width as f32, self.resources.height as f32);
        // Шаг каскада от радиуса: делитель подобран сравнением с Chrome на
        // фикстуре blur.html — при 8 (как у фона) размытие выходило заметно
        // шире браузерного.
        let strength = (group.blur_radius / 14.0).clamp(0.2, 6.0);
        // Поля под растекание: размытая картинка шире исходной.
        let margin = (24.0 * strength).min(96.0);
        let bx = (group.bounds.origin.x.0 - margin).max(0.0).floor();
        let by = (group.bounds.origin.y.0 - margin).max(0.0).floor();
        let bx1 = (group.bounds.origin.x.0 + group.bounds.size.width.0 + margin)
            .min(vw)
            .ceil();
        let by1 = (group.bounds.origin.y.0 + group.bounds.size.height.0 + margin)
            .min(vh)
            .ceil();
        let (rw, rh) = (bx1 - bx, by1 - by);
        if rw < 8.0 || rh < 8.0 {
            return Ok(());
        }

        let src = self.blur.groups[index * 2].srv.clone();
        self.blur_cascade(&src, (vw, vh), (bx, by, rw, rh), strength, true)?;

        // Вернуть размытую область на её место в полноразмерном буфере: он и
        // будет композититься в кадр.
        let dest_rtv = self.blur.groups[index * 2 + 1].rtv.clone();
        let last = self.blur.down[2].srv.clone();
        let (lw, lh) = (
            self.blur.down[2].width as f32,
            self.blur.down[2].height as f32,
        );
        update_buffer(
            &dc,
            self.blur.globals[0].as_ref().unwrap(),
            &[GlobalParams {
                gamma_ratios: self.font_info.gamma_ratios,
                viewport_size: [vw, vh],
                grayscale_enhanced_contrast: self.font_info.grayscale_enhanced_contrast,
                _pad: 0,
            }],
        )?;
        let quad = BlurQuad {
            bounds: [bx, by, rw, rh],
            content_mask: [bx, by, rw, rh],
            corner_radii: [0.0; 4],
            src_origin: [0.0, 0.0],
            src_scale: [1.0, 1.0],
            texel: [1.0 / lw, 1.0 / lh],
            blur_pass: 2.0,
            pad: 0.0,
            blend_mode: 0,
            poly_count: 0,
            pad2: [0.0; 2],
            poly: [[0.0; 4]; 4],
        };
        self.pipelines
            .blur_pipeline
            .update_buffer(&device, &dc, &[quad])?;
        unsafe {
            dc.ClearRenderTargetView(dest_rtv[0].as_ref().unwrap(), &[0.0; 4]);
            dc.OMSetRenderTargets(Some(&dest_rtv), None);
        }
        let blend = self.premultiplied_blend()?;
        self.pipelines.blur_pipeline.draw_with_texture_blended(
            &dc,
            &last,
            &self.resources.viewport,
            &self.blur.globals,
            &self.globals.sampler,
            1,
            Some(&blend),
        )
    }

    /// KaminIDE patch: каскад уменьшений области текстуры.
    ///
    /// Широкое размытие одним фильтром стоит квадрат радиуса выборок; цепочка
    /// /2 → /4 → /8 с tent-фильтром даёт ту же мягкость за десятки.
    fn blur_cascade(
        &mut self,
        src: &[Option<ID3D11ShaderResourceView>; 1],
        src_size: (f32, f32),
        region: (f32, f32, f32, f32),
        strength: f32,
        keep_alpha: bool,
    ) -> Result<()> {
        let device = self.devices.device.clone();
        let dc = self.devices.device_context.clone();
        // Прозрачность значима только у групп — там цвет премультиплирован.
        let blend = if keep_alpha {
            Some(self.premultiplied_blend()?)
        } else {
            None
        };
        let (bx, by, rw, rh) = region;
        let step = |d: f32| -> (u32, u32) { (((rw / d) as u32).max(1), ((rh / d) as u32).max(1)) };
        let sizes = [
            step(2.0 * strength),
            step(4.0 * strength),
            step(8.0 * strength),
        ];
        if self.blur.down.len() != 3
            || self
                .blur
                .down
                .iter()
                .zip(sizes.iter())
                .any(|(t, s)| t.width != s.0 || t.height != s.1)
        {
            self.blur.down.clear();
            for (w, h) in sizes {
                self.blur.down.push(create_blur_texture(&device, w, h, true)?);
            }
        }
        self.ensure_blur_globals(&device)?;

        blur_down_pass(
            &device,
            &dc,
            &mut self.pipelines.blur_pipeline,
            &self.blur.globals,
            &self.globals.sampler,
            self.font_info,
            &self.blur.down[0],
            src,
            src_size,
            [bx / src_size.0, by / src_size.1],
            [rw / src_size.0, rh / src_size.1],
            keep_alpha,
            blend.as_ref(),
        )?;
        // split_at: цель и источник — разные элементы одного вектора.
        let (a, b) = self.blur.down.split_at(1);
        blur_down_pass(
            &device,
            &dc,
            &mut self.pipelines.blur_pipeline,
            &self.blur.globals,
            &self.globals.sampler,
            self.font_info,
            &b[0],
            &a[0].srv,
            (a[0].width as f32, a[0].height as f32),
            [0.0, 0.0],
            [1.0, 1.0],
            keep_alpha,
            blend.as_ref(),
        )?;
        blur_down_pass(
            &device,
            &dc,
            &mut self.pipelines.blur_pipeline,
            &self.blur.globals,
            &self.globals.sampler,
            self.font_info,
            &b[1],
            &b[0].srv,
            (b[0].width as f32, b[0].height as f32),
            [0.0, 0.0],
            [1.0, 1.0],
            keep_alpha,
            blend.as_ref(),
        )
    }

    /// KaminIDE patch: свой буфер параметров у проходов размытия — их вьюпорты
    /// не равны вьюпорту кадра.
    fn ensure_blur_globals(&mut self, device: &ID3D11Device) -> Result<()> {
        if self.blur.globals[0].is_some() {
            return Ok(());
        }
        let desc = D3D11_BUFFER_DESC {
            ByteWidth: std::mem::size_of::<GlobalParams>() as u32,
            Usage: D3D11_USAGE_DYNAMIC,
            BindFlags: D3D11_BIND_CONSTANT_BUFFER.0 as u32,
            CPUAccessFlags: D3D11_CPU_ACCESS_WRITE.0 as u32,
            ..Default::default()
        };
        let mut buffer = None;
        unsafe { device.CreateBuffer(&desc, None, Some(&mut buffer))? };
        self.blur.globals[0] = buffer;
        Ok(())
    }

    /// KaminIDE patch: вернуть параметры кадра после проходов с чужим вьюпортом.
    fn restore_frame_globals(&mut self) -> Result<()> {
        update_buffer(
            &self.devices.device_context,
            self.globals.global_params_buffer[0].as_ref().unwrap(),
            &[GlobalParams {
                gamma_ratios: self.font_info.gamma_ratios,
                viewport_size: [
                    self.resources.viewport[0].Width,
                    self.resources.viewport[0].Height,
                ],
                grayscale_enhanced_contrast: self.font_info.grayscale_enhanced_contrast,
                _pad: 0,
            }],
        )
    }

    pub(crate) fn resize(&mut self, new_size: Size<DevicePixels>) -> Result<()> {
        let width = new_size.width.0.max(1) as u32;
        let height = new_size.height.0.max(1) as u32;
        if self.resources.width == width && self.resources.height == height {
            return Ok(());
        }
        self.resources.width = width;
        self.resources.height = height;

        // Clear the render target before resizing
        unsafe { self.devices.device_context.OMSetRenderTargets(None, None) };
        unsafe { ManuallyDrop::drop(&mut self.resources.render_target) };
        drop(self.resources.render_target_view[0].take().unwrap());

        // Resizing the swap chain requires a call to the underlying DXGI adapter, which can return the device removed error.
        // The app might have moved to a monitor that's attached to a different graphics device.
        // When a graphics device is removed or reset, the desktop resolution often changes, resulting in a window size change.
        // But here we just return the error, because we are handling device lost scenarios elsewhere.
        unsafe {
            self.resources
                .swap_chain
                .ResizeBuffers(
                    BUFFER_COUNT as u32,
                    width,
                    height,
                    RENDER_TARGET_FORMAT,
                    // KaminIDE patch (#76): флаги обязаны совпадать с create.
                    DXGI_SWAP_CHAIN_FLAG_FRAME_LATENCY_WAITABLE_OBJECT,
                )
                .context("Failed to resize swap chain")?;
        }

        self.resources
            .recreate_resources(&self.devices, width, height)?;
        unsafe {
            self.devices
                .device_context
                .OMSetRenderTargets(Some(&self.resources.render_target_view), None);
        }

        Ok(())
    }

    fn draw_shadows(&mut self, shadows: &[Shadow]) -> Result<()> {
        if shadows.is_empty() {
            return Ok(());
        }
        self.pipelines.shadow_pipeline.update_buffer(
            &self.devices.device,
            &self.devices.device_context,
            shadows,
        )?;
        self.pipelines.shadow_pipeline.draw(
            &self.devices.device_context,
            &self.resources.viewport,
            &self.globals.global_params_buffer,
            D3D_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP,
            4,
            shadows.len() as u32,
        )
    }

    fn draw_quads(&mut self, quads: &[Quad]) -> Result<()> {
        if quads.is_empty() {
            return Ok(());
        }
        self.pipelines.quad_pipeline.update_buffer(
            &self.devices.device,
            &self.devices.device_context,
            quads,
        )?;
        self.pipelines.quad_pipeline.draw(
            &self.devices.device_context,
            &self.resources.viewport,
            &self.globals.global_params_buffer,
            D3D_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP,
            4,
            quads.len() as u32,
        )
    }

    fn draw_paths_to_intermediate(&mut self, paths: &[Path<ScaledPixels>]) -> Result<()> {
        if paths.is_empty() {
            return Ok(());
        }

        // Clear intermediate MSAA texture
        unsafe {
            self.devices.device_context.ClearRenderTargetView(
                self.resources.path_intermediate_msaa_view[0]
                    .as_ref()
                    .unwrap(),
                &[0.0; 4],
            );
            // Set intermediate MSAA texture as render target
            self.devices
                .device_context
                .OMSetRenderTargets(Some(&self.resources.path_intermediate_msaa_view), None);
        }

        // Collect all vertices and sprites for a single draw call
        let mut vertices = Vec::new();

        for path in paths {
            vertices.extend(path.vertices.iter().map(|v| PathRasterizationSprite {
                xy_position: v.xy_position,
                st_position: v.st_position,
                color: path.color,
                bounds: path.clipped_bounds(),
            }));
        }

        self.pipelines.path_rasterization_pipeline.update_buffer(
            &self.devices.device,
            &self.devices.device_context,
            &vertices,
        )?;
        self.pipelines.path_rasterization_pipeline.draw(
            &self.devices.device_context,
            &self.resources.viewport,
            &self.globals.global_params_buffer,
            D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST,
            vertices.len() as u32,
            1,
        )?;

        // Resolve MSAA to non-MSAA intermediate texture
        unsafe {
            self.devices.device_context.ResolveSubresource(
                &self.resources.path_intermediate_texture,
                0,
                &self.resources.path_intermediate_msaa_texture,
                0,
                RENDER_TARGET_FORMAT,
            );
            // Restore main render target
            self.devices
                .device_context
                .OMSetRenderTargets(Some(self.current_target()), None);
        }

        Ok(())
    }

    fn draw_paths_from_intermediate(&mut self, paths: &[Path<ScaledPixels>]) -> Result<()> {
        let Some(first_path) = paths.first() else {
            return Ok(());
        };

        // When copying paths from the intermediate texture to the drawable,
        // each pixel must only be copied once, in case of transparent paths.
        //
        // If all paths have the same draw order, then their bounds are all
        // disjoint, so we can copy each path's bounds individually. If this
        // batch combines different draw orders, we perform a single copy
        // for a minimal spanning rect.
        let sprites = if paths.last().unwrap().order == first_path.order {
            paths
                .iter()
                .map(|path| PathSprite {
                    bounds: path.clipped_bounds(),
                })
                .collect::<Vec<_>>()
        } else {
            let mut bounds = first_path.clipped_bounds();
            for path in paths.iter().skip(1) {
                bounds = bounds.union(&path.clipped_bounds());
            }
            vec![PathSprite { bounds }]
        };

        self.pipelines.path_sprite_pipeline.update_buffer(
            &self.devices.device,
            &self.devices.device_context,
            &sprites,
        )?;

        // Draw the sprites with the path texture
        self.pipelines.path_sprite_pipeline.draw_with_texture(
            &self.devices.device_context,
            &self.resources.path_intermediate_srv,
            &self.resources.viewport,
            &self.globals.global_params_buffer,
            &self.globals.sampler,
            sprites.len() as u32,
        )
    }

    fn draw_underlines(&mut self, underlines: &[Underline]) -> Result<()> {
        if underlines.is_empty() {
            return Ok(());
        }
        self.pipelines.underline_pipeline.update_buffer(
            &self.devices.device,
            &self.devices.device_context,
            underlines,
        )?;
        self.pipelines.underline_pipeline.draw(
            &self.devices.device_context,
            &self.resources.viewport,
            &self.globals.global_params_buffer,
            D3D_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP,
            4,
            underlines.len() as u32,
        )
    }

    fn draw_monochrome_sprites(
        &mut self,
        texture_id: AtlasTextureId,
        sprites: &[MonochromeSprite],
    ) -> Result<()> {
        if sprites.is_empty() {
            return Ok(());
        }
        self.pipelines.mono_sprites.update_buffer(
            &self.devices.device,
            &self.devices.device_context,
            sprites,
        )?;
        // KaminIDE patch: слота уже нет (см. unregister_external) — пачку
        // молча пропускаем, кадр без этих спрайтов лучше паники.
        let Some(texture_view) = self.atlas.get_texture_view(texture_id) else {
            return Ok(());
        };
        self.pipelines.mono_sprites.draw_with_texture(
            &self.devices.device_context,
            &texture_view,
            &self.resources.viewport,
            &self.globals.global_params_buffer,
            &self.globals.sampler,
            sprites.len() as u32,
        )
    }

    fn draw_polychrome_sprites(
        &mut self,
        texture_id: AtlasTextureId,
        sprites: &[PolychromeSprite],
    ) -> Result<()> {
        if sprites.is_empty() {
            return Ok(());
        }
        self.pipelines.poly_sprites.update_buffer(
            &self.devices.device,
            &self.devices.device_context,
            sprites,
        )?;
        // KaminIDE patch: см. draw_monochrome_sprites — слот освобождён →
        // пропуск пачки вместо паники.
        let Some(texture_view) = self.atlas.get_texture_view(texture_id) else {
            return Ok(());
        };
        self.pipelines.poly_sprites.draw_with_texture(
            &self.devices.device_context,
            &texture_view,
            &self.resources.viewport,
            &self.globals.global_params_buffer,
            &self.globals.sampler,
            sprites.len() as u32,
        )
    }

    // KaminIDE patch: на Windows Surface-примитив = backdrop blur (см.
    // window.rs::paint_backdrop_blur). Кадр под областью копируется, гонится
    // каскадом даунсемплов /2→/4→/8 (tent-фильтр в шейдере) и композитится
    // обратно с SDF-маской скруглённых углов.
    fn draw_surfaces(&mut self, surfaces: &[PaintSurface]) -> Result<()> {
        if surfaces.is_empty() {
            return Ok(());
        }
        for surface in surfaces {
            // KaminIDE patch: метка группы (`Window::paint_group`) — не
            // размытие фона, а «положить сюда готовый буфер группы».
            let drawn = if surface.group > 0 {
                self.draw_group_composite(surface)
            } else {
                self.draw_backdrop_blur(surface)
            };
            if let Err(e) = drawn {
                log::warn!("surface pass failed: {e}");
            }
        }
        // Вернуть состояние кадра (проходы меняли RTV/вьюпорт/globals).
        update_buffer(
            &self.devices.device_context,
            self.globals.global_params_buffer[0].as_ref().unwrap(),
            &[GlobalParams {
                gamma_ratios: self.font_info.gamma_ratios,
                viewport_size: [
                    self.resources.viewport[0].Width,
                    self.resources.viewport[0].Height,
                ],
                grayscale_enhanced_contrast: self.font_info.grayscale_enhanced_contrast,
                _pad: 0,
            }],
        )?;
        unsafe {
            self.devices
                .device_context
                .OMSetRenderTargets(Some(&self.resources.render_target_view), None);
            self.devices
                .device_context
                .RSSetViewports(Some(&self.resources.viewport));
        }
        Ok(())
    }

    /// KaminIDE patch: один blur-квад (см. draw_surfaces).
    fn draw_backdrop_blur(&mut self, s: &PaintSurface) -> Result<()> {
        let device = self.devices.device.clone();
        let dc = self.devices.device_context.clone();
        let (vw, vh) = (self.resources.width, self.resources.height);
        // KaminIDE patch: сила размытия из CSS. Каскад уменьшений задаёт
        // радиус: чем сильнее ужимаем перед растяжением обратно, тем шире
        // размывается. 8 — прежнее умолчание, от него и считаем.
        let strength = (s.blur_radius / 8.0).clamp(0.25, 6.0);
        // Область + поля под радиус размытия, кламп в кадр.
        let margin: f32 = 24.0 * strength;
        let margin = margin.min(96.0);
        let bx = (s.bounds.origin.x.0 - margin).max(0.0).floor();
        let by = (s.bounds.origin.y.0 - margin).max(0.0).floor();
        let bx1 = (s.bounds.origin.x.0 + s.bounds.size.width.0 + margin)
            .min(vw as f32)
            .ceil();
        let by1 = (s.bounds.origin.y.0 + s.bounds.size.height.0 + margin)
            .min(vh as f32)
            .ceil();
        let (rw, rh) = ((bx1 - bx) as u32, (by1 - by) as u32);
        if rw < 8 || rh < 8 {
            return Ok(());
        }

        // Копия бэкбуфера (по размеру окна, кэш).
        if self
            .blur
            .copy
            .as_ref()
            .is_none_or(|t| t.width != vw || t.height != vh)
        {
            self.blur.copy = Some(create_blur_texture(&device, vw, vh, false)?);
        }

        // Копия кадра под областью → каскад уменьшений.
        let copy_srv = {
            let copy = self.blur.copy.as_ref().unwrap();
            unsafe {
                dc.CopyResource(&copy.texture, &*self.resources.render_target);
            }
            copy.srv.clone()
        };
        self.blur_cascade(
            &copy_srv,
            (vw as f32, vh as f32),
            (bx, by, rw as f32, rh as f32),
            strength,
            false,
        )?;

        // Композит в бэкбуфер: bounds/маска исходной области, uv = положение
        // bounds внутри области каскада.
        update_buffer(
            &dc,
            self.blur.globals[0].as_ref().unwrap(),
            &[GlobalParams {
                gamma_ratios: self.font_info.gamma_ratios,
                viewport_size: [
                    self.resources.viewport[0].Width,
                    self.resources.viewport[0].Height,
                ],
                grayscale_enhanced_contrast: self.font_info.grayscale_enhanced_contrast,
                _pad: 0,
            }],
        )?;
        let last = &self.blur.down[2];
        let quad = BlurQuad {
            bounds: [
                s.bounds.origin.x.0,
                s.bounds.origin.y.0,
                s.bounds.size.width.0,
                s.bounds.size.height.0,
            ],
            content_mask: [
                s.content_mask.bounds.origin.x.0,
                s.content_mask.bounds.origin.y.0,
                s.content_mask.bounds.size.width.0,
                s.content_mask.bounds.size.height.0,
            ],
            corner_radii: [
                s.corner_radii.top_left.0,
                s.corner_radii.top_right.0,
                s.corner_radii.bottom_right.0,
                s.corner_radii.bottom_left.0,
            ],
            src_origin: [
                (s.bounds.origin.x.0 - bx) / rw as f32,
                (s.bounds.origin.y.0 - by) / rh as f32,
            ],
            src_scale: [
                s.bounds.size.width.0 / rw as f32,
                s.bounds.size.height.0 / rh as f32,
            ],
            texel: [1.0 / last.width as f32, 1.0 / last.height as f32],
            blur_pass: 0.0,
            pad: 0.0,
            blend_mode: 0,
            poly_count: 0,
            pad2: [0.0; 2],
            poly: [[0.0; 4]; 4],
        };
        self.pipelines
            .blur_pipeline
            .update_buffer(&device, &dc, &[quad])?;
        unsafe {
            dc.OMSetRenderTargets(Some(&self.resources.render_target_view), None);
        }
        self.pipelines.blur_pipeline.draw_with_texture(
            &dc,
            &last.srv,
            &self.resources.viewport,
            &self.blur.globals,
            &self.globals.sampler,
            1,
        )
    }

    /// KaminIDE patch: положить готовый буфер группы в кадр.
    ///
    /// Буфер размером с окно, поэтому координаты совпадают один в один:
    /// берётся ровно прямоугольник группы, гасится маской скруглений и
    /// прозрачностью группы.
    fn draw_group_composite(&mut self, s: &PaintSurface) -> Result<()> {
        let device = self.devices.device.clone();
        let dc = self.devices.device_context.clone();
        let Some(&slot) = self.blur.group_slots.get(s.group as usize - 1) else {
            return Ok(());
        };
        let Some(texture) = self.blur.groups.get(slot) else {
            return Ok(());
        };
        let srv = texture.srv.clone();
        let blend_mode = self
            .blur
            .group_blend
            .get(s.group as usize - 1)
            .copied()
            .unwrap_or(0);
        let (poly, poly_count) = self
            .blur
            .group_poly
            .get(s.group as usize - 1)
            .copied()
            .unwrap_or(([[0.0; 4]; 4], 0));
        let (vw, vh) = (self.resources.width as f32, self.resources.height as f32);

        // Смешивание считается в шейдере, а ему нужен цвет назначения:
        // сэмплировать связанный с целью буфер нельзя, поэтому копия.
        if blend_mode != 0 {
            let (rw, rh) = (self.resources.width, self.resources.height);
            if self
                .blur
                .copy
                .as_ref()
                .is_none_or(|t| t.width != rw || t.height != rh)
            {
                self.blur.copy = Some(create_blur_texture(&device, rw, rh, false)?);
            }
            let copy = self.blur.copy.as_ref().unwrap();
            unsafe {
                dc.CopyResource(&copy.texture, &*self.resources.render_target);
                dc.PSSetShaderResources(2, Some(&copy.srv));
            }
        }

        self.ensure_blur_globals(&device)?;
        update_buffer(
            &dc,
            self.blur.globals[0].as_ref().unwrap(),
            &[GlobalParams {
                gamma_ratios: self.font_info.gamma_ratios,
                viewport_size: [vw, vh],
                grayscale_enhanced_contrast: self.font_info.grayscale_enhanced_contrast,
                _pad: 0,
            }],
        )?;
        let quad = BlurQuad {
            bounds: [
                s.bounds.origin.x.0,
                s.bounds.origin.y.0,
                s.bounds.size.width.0,
                s.bounds.size.height.0,
            ],
            content_mask: [
                s.content_mask.bounds.origin.x.0,
                s.content_mask.bounds.origin.y.0,
                s.content_mask.bounds.size.width.0,
                s.content_mask.bounds.size.height.0,
            ],
            corner_radii: [
                s.corner_radii.top_left.0,
                s.corner_radii.top_right.0,
                s.corner_radii.bottom_right.0,
                s.corner_radii.bottom_left.0,
            ],
            src_origin: [s.bounds.origin.x.0 / vw, s.bounds.origin.y.0 / vh],
            src_scale: [s.bounds.size.width.0 / vw, s.bounds.size.height.0 / vh],
            texel: [1.0 / vw, 1.0 / vh],
            blur_pass: 3.0,
            pad: s.opacity,
            blend_mode,
            poly_count,
            pad2: [0.0; 2],
            poly,
        };
        self.pipelines
            .blur_pipeline
            .update_buffer(&device, &dc, &[quad])?;
        unsafe {
            dc.OMSetRenderTargets(Some(&self.resources.render_target_view), None);
        }
        // Смешанный цвет шейдер считает целиком, вместе с прозрачностью —
        // блендеру тут делать нечего, результат пишется поверх.
        let blend = if blend_mode == 0 {
            self.premultiplied_blend()?
        } else {
            self.replace_blend()?
        };
        let drawn = self.pipelines.blur_pipeline.draw_with_texture_blended(
            &dc,
            &srv,
            &self.resources.viewport,
            &self.blur.globals,
            &self.globals.sampler,
            1,
            Some(&blend),
        );
        if blend_mode != 0 {
            // Копия кадра больше не нужна: оставленная привязка запретила бы
            // рисовать в неё следующим кадром.
            unsafe { dc.PSSetShaderResources(2, Some(&[None])) };
        }
        drawn
    }

    pub(crate) fn gpu_specs(&self) -> Result<GpuSpecs> {
        let desc = unsafe { self.devices.adapter.GetDesc1() }?;
        let is_software_emulated = (desc.Flags & DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32) != 0;
        let device_name = String::from_utf16_lossy(&desc.Description)
            .trim_matches(char::from(0))
            .to_string();
        let driver_name = match desc.VendorId {
            0x10DE => "NVIDIA Corporation".to_string(),
            0x1002 => "AMD Corporation".to_string(),
            0x8086 => "Intel Corporation".to_string(),
            id => format!("Unknown Vendor (ID: {:#X})", id),
        };
        let driver_version = match desc.VendorId {
            0x10DE => nvidia::get_driver_version(),
            0x1002 => amd::get_driver_version(),
            // For Intel and other vendors, we use the DXGI API to get the driver version.
            _ => dxgi::get_driver_version(&self.devices.adapter),
        }
        .context("Failed to get gpu driver info")
        .log_err()
        .unwrap_or("Unknown Driver".to_string());
        Ok(GpuSpecs {
            is_software_emulated,
            device_name,
            driver_name,
            driver_info: driver_version,
        })
    }

    pub(crate) fn get_font_info() -> &'static FontInfo {
        static CACHED_FONT_INFO: OnceLock<FontInfo> = OnceLock::new();
        CACHED_FONT_INFO.get_or_init(|| unsafe {
            let factory: IDWriteFactory5 = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED).unwrap();
            let render_params: IDWriteRenderingParams1 =
                factory.CreateRenderingParams().unwrap().cast().unwrap();
            FontInfo {
                gamma_ratios: Self::get_gamma_ratios(render_params.GetGamma()),
                grayscale_enhanced_contrast: render_params.GetGrayscaleEnhancedContrast(),
            }
        })
    }

    // Gamma ratios for brightening/darkening edges for better contrast
    // https://github.com/microsoft/terminal/blob/1283c0f5b99a2961673249fa77c6b986efb5086c/src/renderer/atlas/dwrite.cpp#L50
    fn get_gamma_ratios(gamma: f32) -> [f32; 4] {
        const GAMMA_INCORRECT_TARGET_RATIOS: [[f32; 4]; 13] = [
            [0.0000 / 4.0, 0.0000 / 4.0, 0.0000 / 4.0, 0.0000 / 4.0], // gamma = 1.0
            [0.0166 / 4.0, -0.0807 / 4.0, 0.2227 / 4.0, -0.0751 / 4.0], // gamma = 1.1
            [0.0350 / 4.0, -0.1760 / 4.0, 0.4325 / 4.0, -0.1370 / 4.0], // gamma = 1.2
            [0.0543 / 4.0, -0.2821 / 4.0, 0.6302 / 4.0, -0.1876 / 4.0], // gamma = 1.3
            [0.0739 / 4.0, -0.3963 / 4.0, 0.8167 / 4.0, -0.2287 / 4.0], // gamma = 1.4
            [0.0933 / 4.0, -0.5161 / 4.0, 0.9926 / 4.0, -0.2616 / 4.0], // gamma = 1.5
            [0.1121 / 4.0, -0.6395 / 4.0, 1.1588 / 4.0, -0.2877 / 4.0], // gamma = 1.6
            [0.1300 / 4.0, -0.7649 / 4.0, 1.3159 / 4.0, -0.3080 / 4.0], // gamma = 1.7
            [0.1469 / 4.0, -0.8911 / 4.0, 1.4644 / 4.0, -0.3234 / 4.0], // gamma = 1.8
            [0.1627 / 4.0, -1.0170 / 4.0, 1.6051 / 4.0, -0.3347 / 4.0], // gamma = 1.9
            [0.1773 / 4.0, -1.1420 / 4.0, 1.7385 / 4.0, -0.3426 / 4.0], // gamma = 2.0
            [0.1908 / 4.0, -1.2652 / 4.0, 1.8650 / 4.0, -0.3476 / 4.0], // gamma = 2.1
            [0.2031 / 4.0, -1.3864 / 4.0, 1.9851 / 4.0, -0.3501 / 4.0], // gamma = 2.2
        ];

        const NORM13: f32 = ((0x10000 as f64) / (255.0 * 255.0) * 4.0) as f32;
        const NORM24: f32 = ((0x100 as f64) / (255.0) * 4.0) as f32;

        let index = ((gamma * 10.0).round() as usize).clamp(10, 22) - 10;
        let ratios = GAMMA_INCORRECT_TARGET_RATIOS[index];

        [
            ratios[0] * NORM13,
            ratios[1] * NORM24,
            ratios[2] * NORM13,
            ratios[3] * NORM24,
        ]
    }
}

impl DirectXResources {
    pub fn new(
        devices: &DirectXRendererDevices,
        width: u32,
        height: u32,
        hwnd: HWND,
        disable_direct_composition: bool,
    ) -> Result<ManuallyDrop<Self>> {
        let swap_chain = if disable_direct_composition {
            create_swap_chain(&devices.dxgi_factory, &devices.device, hwnd, width, height)?
        } else {
            create_swap_chain_for_composition(
                &devices.dxgi_factory,
                &devices.device,
                width,
                height,
            )?
        };
        // KaminIDE patch (#76): waitable-гейт — Present больше НИКОГДА не
        // блокирует UI-поток. При занятом компоузере (RDP-энкодер 60-90мс на
        // кадр — diag.log юзера) draw() пропускает кадр и перерисовывается
        //低-приоритетным WM_PAINT ПОСЛЕ обработки ввода.
        let frame_gate = unsafe {
            use windows::core::Interface;
            swap_chain
                .cast::<IDXGISwapChain2>()
                .ok()
                .and_then(|sc2| {
                    sc2.SetMaximumFrameLatency(1).ok()?;
                    let h = sc2.GetFrameLatencyWaitableObject();
                    (!h.is_invalid()).then_some(FrameLatencyGate(h))
                })
        };

        let (
            render_target,
            render_target_view,
            path_intermediate_texture,
            path_intermediate_srv,
            path_intermediate_msaa_texture,
            path_intermediate_msaa_view,
            viewport,
        ) = create_resources(devices, &swap_chain, width, height)?;
        set_rasterizer_state(&devices.device, &devices.device_context)?;

        Ok(ManuallyDrop::new(Self {
            swap_chain,
            frame_gate,
            render_target,
            render_target_view,
            path_intermediate_texture,
            path_intermediate_msaa_texture,
            path_intermediate_msaa_view,
            path_intermediate_srv,
            viewport,
            width,
            height,
        }))
    }

    #[inline]
    fn recreate_resources(
        &mut self,
        devices: &DirectXRendererDevices,
        width: u32,
        height: u32,
    ) -> Result<()> {
        let (
            render_target,
            render_target_view,
            path_intermediate_texture,
            path_intermediate_srv,
            path_intermediate_msaa_texture,
            path_intermediate_msaa_view,
            viewport,
        ) = create_resources(devices, &self.swap_chain, width, height)?;
        self.render_target = render_target;
        self.render_target_view = render_target_view;
        self.path_intermediate_texture = path_intermediate_texture;
        self.path_intermediate_msaa_texture = path_intermediate_msaa_texture;
        self.path_intermediate_msaa_view = path_intermediate_msaa_view;
        self.path_intermediate_srv = path_intermediate_srv;
        self.viewport = viewport;
        Ok(())
    }
}

impl DirectXRenderPipelines {
    pub fn new(device: &ID3D11Device) -> Result<Self> {
        let shadow_pipeline = PipelineState::new(
            device,
            "shadow_pipeline",
            ShaderModule::Shadow,
            4,
            create_blend_state(device)?,
        )?;
        let quad_pipeline = PipelineState::new(
            device,
            "quad_pipeline",
            ShaderModule::Quad,
            64,
            create_blend_state(device)?,
        )?;
        let path_rasterization_pipeline = PipelineState::new(
            device,
            "path_rasterization_pipeline",
            ShaderModule::PathRasterization,
            32,
            create_blend_state_for_path_rasterization(device)?,
        )?;
        let path_sprite_pipeline = PipelineState::new(
            device,
            "path_sprite_pipeline",
            ShaderModule::PathSprite,
            4,
            create_blend_state_for_path_sprite(device)?,
        )?;
        let underline_pipeline = PipelineState::new(
            device,
            "underline_pipeline",
            ShaderModule::Underline,
            4,
            create_blend_state(device)?,
        )?;
        let mono_sprites = PipelineState::new(
            device,
            "monochrome_sprite_pipeline",
            ShaderModule::MonochromeSprite,
            512,
            create_blend_state(device)?,
        )?;
        let poly_sprites = PipelineState::new(
            device,
            "polychrome_sprite_pipeline",
            ShaderModule::PolychromeSprite,
            16,
            create_blend_state(device)?,
        )?;
        let blur_pipeline = PipelineState::new(
            device,
            "blur_pipeline",
            ShaderModule::Blur,
            4,
            create_blend_state(device)?,
        )?;

        Ok(Self {
            shadow_pipeline,
            quad_pipeline,
            path_rasterization_pipeline,
            path_sprite_pipeline,
            underline_pipeline,
            mono_sprites,
            poly_sprites,
            blur_pipeline,
        })
    }
}

impl DirectComposition {
    pub fn new(dxgi_device: &IDXGIDevice, hwnd: HWND) -> Result<Self> {
        let comp_device = get_comp_device(dxgi_device)?;
        let comp_target = unsafe { comp_device.CreateTargetForHwnd(hwnd, true) }?;
        let comp_visual = unsafe { comp_device.CreateVisual() }?;
        // KaminIDE patch (visual hosting): root = контейнер, gpui-визуал —
        // его ребёнок (underlay-эпоха WebView2 удалена; CEF рисуется через
        // external-текстуры атласа, дерево dcomp статично после setup).
        let comp_root = unsafe { comp_device.CreateVisual() }?;

        Ok(Self {
            comp_device,
            comp_target,
            comp_visual,
            comp_root,
        })
    }

    pub fn set_swap_chain(&self, swap_chain: &IDXGISwapChain1) -> Result<()> {
        unsafe {
            self.comp_visual.SetContent(swap_chain)?;
            self.comp_root.AddVisual(&self.comp_visual, false, None)?;
            self.comp_target.SetRoot(&self.comp_root)?;
            self.comp_device.Commit()?;
        }
        Ok(())
    }

}

impl DirectXGlobalElements {
    pub fn new(device: &ID3D11Device) -> Result<Self> {
        let global_params_buffer = unsafe {
            let desc = D3D11_BUFFER_DESC {
                ByteWidth: std::mem::size_of::<GlobalParams>() as u32,
                Usage: D3D11_USAGE_DYNAMIC,
                BindFlags: D3D11_BIND_CONSTANT_BUFFER.0 as u32,
                CPUAccessFlags: D3D11_CPU_ACCESS_WRITE.0 as u32,
                ..Default::default()
            };
            let mut buffer = None;
            device.CreateBuffer(&desc, None, Some(&mut buffer))?;
            [buffer]
        };

        let sampler = unsafe {
            let desc = D3D11_SAMPLER_DESC {
                Filter: D3D11_FILTER_MIN_MAG_MIP_LINEAR,
                AddressU: D3D11_TEXTURE_ADDRESS_WRAP,
                AddressV: D3D11_TEXTURE_ADDRESS_WRAP,
                AddressW: D3D11_TEXTURE_ADDRESS_WRAP,
                MipLODBias: 0.0,
                MaxAnisotropy: 1,
                ComparisonFunc: D3D11_COMPARISON_ALWAYS,
                BorderColor: [0.0; 4],
                MinLOD: 0.0,
                MaxLOD: D3D11_FLOAT32_MAX,
            };
            let mut output = None;
            device.CreateSamplerState(&desc, Some(&mut output))?;
            [output]
        };

        Ok(Self {
            global_params_buffer,
            sampler,
        })
    }
}

#[derive(Debug, Default)]
#[repr(C)]
struct GlobalParams {
    gamma_ratios: [f32; 4],
    viewport_size: [f32; 2],
    grayscale_enhanced_contrast: f32,
    _pad: u32,
}

struct PipelineState<T> {
    label: &'static str,
    vertex: ID3D11VertexShader,
    fragment: ID3D11PixelShader,
    buffer: ID3D11Buffer,
    buffer_size: usize,
    view: [Option<ID3D11ShaderResourceView>; 1],
    blend_state: ID3D11BlendState,
    _marker: std::marker::PhantomData<T>,
}

impl<T> PipelineState<T> {
    fn new(
        device: &ID3D11Device,
        label: &'static str,
        shader_module: ShaderModule,
        buffer_size: usize,
        blend_state: ID3D11BlendState,
    ) -> Result<Self> {
        let vertex = {
            let raw_shader = RawShaderBytes::new(shader_module, ShaderTarget::Vertex)?;
            create_vertex_shader(device, raw_shader.as_bytes())?
        };
        let fragment = {
            let raw_shader = RawShaderBytes::new(shader_module, ShaderTarget::Fragment)?;
            create_fragment_shader(device, raw_shader.as_bytes())?
        };
        let buffer = create_buffer(device, std::mem::size_of::<T>(), buffer_size)?;
        let view = create_buffer_view(device, &buffer)?;

        Ok(PipelineState {
            label,
            vertex,
            fragment,
            buffer,
            buffer_size,
            view,
            blend_state,
            _marker: std::marker::PhantomData,
        })
    }

    fn update_buffer(
        &mut self,
        device: &ID3D11Device,
        device_context: &ID3D11DeviceContext,
        data: &[T],
    ) -> Result<()> {
        if self.buffer_size < data.len() {
            let new_buffer_size = data.len().next_power_of_two();
            log::info!(
                "Updating {} buffer size from {} to {}",
                self.label,
                self.buffer_size,
                new_buffer_size
            );
            let buffer = create_buffer(device, std::mem::size_of::<T>(), new_buffer_size)?;
            let view = create_buffer_view(device, &buffer)?;
            self.buffer = buffer;
            self.view = view;
            self.buffer_size = new_buffer_size;
        }
        update_buffer(device_context, &self.buffer, data)
    }

    fn draw(
        &self,
        device_context: &ID3D11DeviceContext,
        viewport: &[D3D11_VIEWPORT],
        global_params: &[Option<ID3D11Buffer>],
        topology: D3D_PRIMITIVE_TOPOLOGY,
        vertex_count: u32,
        instance_count: u32,
    ) -> Result<()> {
        self.draw_blended(
            device_context,
            viewport,
            global_params,
            topology,
            vertex_count,
            instance_count,
            None,
        )
    }

    /// KaminIDE patch: та же отрисовка с ЧУЖИМ состоянием блендера.
    ///
    /// Штатный `draw` ставит своё состояние прямо перед вызовом отрисовки,
    /// поэтому режим смешивания, выставленный снаружи, затирался — и
    /// `mix-blend-mode` не делал ничего. Режим передаётся сюда явно.
    #[allow(clippy::too_many_arguments)]
    fn draw_blended(
        &self,
        device_context: &ID3D11DeviceContext,
        viewport: &[D3D11_VIEWPORT],
        global_params: &[Option<ID3D11Buffer>],
        topology: D3D_PRIMITIVE_TOPOLOGY,
        vertex_count: u32,
        instance_count: u32,
        blend_override: Option<&ID3D11BlendState>,
    ) -> Result<()> {
        set_pipeline_state(
            device_context,
            &self.view,
            topology,
            viewport,
            &self.vertex,
            &self.fragment,
            global_params,
            blend_override.unwrap_or(&self.blend_state),
        );
        unsafe {
            device_context.DrawInstanced(vertex_count, instance_count, 0, 0);
        }
        Ok(())
    }

    fn draw_with_texture(
        &self,
        device_context: &ID3D11DeviceContext,
        texture: &[Option<ID3D11ShaderResourceView>],
        viewport: &[D3D11_VIEWPORT],
        global_params: &[Option<ID3D11Buffer>],
        sampler: &[Option<ID3D11SamplerState>],
        instance_count: u32,
    ) -> Result<()> {
        self.draw_with_texture_blended(
            device_context,
            texture,
            viewport,
            global_params,
            sampler,
            instance_count,
            None,
        )
    }

    /// KaminIDE patch: то же со своим блендером — буферы групп несут цвет,
    /// уже умноженный на прозрачность.
    #[allow(clippy::too_many_arguments)]
    fn draw_with_texture_blended(
        &self,
        device_context: &ID3D11DeviceContext,
        texture: &[Option<ID3D11ShaderResourceView>],
        viewport: &[D3D11_VIEWPORT],
        global_params: &[Option<ID3D11Buffer>],
        sampler: &[Option<ID3D11SamplerState>],
        instance_count: u32,
        blend_override: Option<&ID3D11BlendState>,
    ) -> Result<()> {
        set_pipeline_state(
            device_context,
            &self.view,
            D3D_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP,
            viewport,
            &self.vertex,
            &self.fragment,
            global_params,
            blend_override.unwrap_or(&self.blend_state),
        );
        unsafe {
            device_context.PSSetSamplers(0, Some(sampler));
            device_context.VSSetShaderResources(0, Some(texture));
            device_context.PSSetShaderResources(0, Some(texture));

            device_context.DrawInstanced(4, instance_count, 0, 0);
        }
        Ok(())
    }
}

#[derive(Clone, Copy)]
#[repr(C)]
struct PathRasterizationSprite {
    xy_position: Point<ScaledPixels>,
    st_position: Point<f32>,
    color: Background,
    bounds: Bounds<ScaledPixels>,
}

#[derive(Clone, Copy)]
#[repr(C)]
struct PathSprite {
    bounds: Bounds<ScaledPixels>,
}

impl Drop for DirectXRenderer {
    fn drop(&mut self) {
        #[cfg(debug_assertions)]
        report_live_objects(&self.devices.device).ok();
        unsafe {
            ManuallyDrop::drop(&mut self.devices);
            ManuallyDrop::drop(&mut self.resources);
        }
    }
}

impl Drop for DirectXResources {
    fn drop(&mut self) {
        unsafe {
            ManuallyDrop::drop(&mut self.render_target);
        }
    }
}

#[inline]
fn get_comp_device(dxgi_device: &IDXGIDevice) -> Result<IDCompositionDevice> {
    Ok(unsafe { DCompositionCreateDevice(dxgi_device)? })
}

fn create_swap_chain_for_composition(
    dxgi_factory: &IDXGIFactory6,
    device: &ID3D11Device,
    width: u32,
    height: u32,
) -> Result<IDXGISwapChain1> {
    let desc = DXGI_SWAP_CHAIN_DESC1 {
        Width: width,
        Height: height,
        Format: RENDER_TARGET_FORMAT,
        Stereo: false.into(),
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
        BufferCount: BUFFER_COUNT as u32,
        // Composition SwapChains only support the DXGI_SCALING_STRETCH Scaling.
        Scaling: DXGI_SCALING_STRETCH,
        SwapEffect: DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
        AlphaMode: DXGI_ALPHA_MODE_PREMULTIPLIED,
        // KaminIDE patch (#76): waitable-гейт (см. FrameLatencyGate).
        Flags: DXGI_SWAP_CHAIN_FLAG_FRAME_LATENCY_WAITABLE_OBJECT.0 as u32,
    };
    Ok(unsafe { dxgi_factory.CreateSwapChainForComposition(device, &desc, None)? })
}

fn create_swap_chain(
    dxgi_factory: &IDXGIFactory6,
    device: &ID3D11Device,
    hwnd: HWND,
    width: u32,
    height: u32,
) -> Result<IDXGISwapChain1> {
    use windows::Win32::Graphics::Dxgi::DXGI_MWA_NO_ALT_ENTER;

    let desc = DXGI_SWAP_CHAIN_DESC1 {
        Width: width,
        Height: height,
        Format: RENDER_TARGET_FORMAT,
        Stereo: false.into(),
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
        BufferCount: BUFFER_COUNT as u32,
        Scaling: DXGI_SCALING_NONE,
        SwapEffect: DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
        AlphaMode: DXGI_ALPHA_MODE_IGNORE,
        // KaminIDE patch (#76): waitable-гейт (см. FrameLatencyGate).
        Flags: DXGI_SWAP_CHAIN_FLAG_FRAME_LATENCY_WAITABLE_OBJECT.0 as u32,
    };
    let swap_chain =
        unsafe { dxgi_factory.CreateSwapChainForHwnd(device, hwnd, &desc, None, None) }?;
    unsafe { dxgi_factory.MakeWindowAssociation(hwnd, DXGI_MWA_NO_ALT_ENTER) }?;
    Ok(swap_chain)
}

#[inline]
fn create_resources(
    devices: &DirectXRendererDevices,
    swap_chain: &IDXGISwapChain1,
    width: u32,
    height: u32,
) -> Result<(
    ManuallyDrop<ID3D11Texture2D>,
    [Option<ID3D11RenderTargetView>; 1],
    ID3D11Texture2D,
    [Option<ID3D11ShaderResourceView>; 1],
    ID3D11Texture2D,
    [Option<ID3D11RenderTargetView>; 1],
    [D3D11_VIEWPORT; 1],
)> {
    let (render_target, render_target_view) =
        create_render_target_and_its_view(swap_chain, &devices.device)?;
    let (path_intermediate_texture, path_intermediate_srv) =
        create_path_intermediate_texture(&devices.device, width, height)?;
    let (path_intermediate_msaa_texture, path_intermediate_msaa_view) =
        create_path_intermediate_msaa_texture_and_view(&devices.device, width, height)?;
    let viewport = set_viewport(&devices.device_context, width as f32, height as f32);
    Ok((
        render_target,
        render_target_view,
        path_intermediate_texture,
        path_intermediate_srv,
        path_intermediate_msaa_texture,
        path_intermediate_msaa_view,
        viewport,
    ))
}

#[inline]
fn create_render_target_and_its_view(
    swap_chain: &IDXGISwapChain1,
    device: &ID3D11Device,
) -> Result<(
    ManuallyDrop<ID3D11Texture2D>,
    [Option<ID3D11RenderTargetView>; 1],
)> {
    let render_target: ID3D11Texture2D = unsafe { swap_chain.GetBuffer(0) }?;
    let mut render_target_view = None;
    unsafe { device.CreateRenderTargetView(&render_target, None, Some(&mut render_target_view))? };
    Ok((
        ManuallyDrop::new(render_target),
        [Some(render_target_view.unwrap())],
    ))
}

#[inline]
fn create_path_intermediate_texture(
    device: &ID3D11Device,
    width: u32,
    height: u32,
) -> Result<(ID3D11Texture2D, [Option<ID3D11ShaderResourceView>; 1])> {
    let texture = unsafe {
        let mut output = None;
        let desc = D3D11_TEXTURE2D_DESC {
            Width: width,
            Height: height,
            MipLevels: 1,
            ArraySize: 1,
            Format: RENDER_TARGET_FORMAT,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: (D3D11_BIND_RENDER_TARGET.0 | D3D11_BIND_SHADER_RESOURCE.0) as u32,
            CPUAccessFlags: 0,
            MiscFlags: 0,
        };
        device.CreateTexture2D(&desc, None, Some(&mut output))?;
        output.unwrap()
    };

    let mut shader_resource_view = None;
    unsafe { device.CreateShaderResourceView(&texture, None, Some(&mut shader_resource_view))? };

    Ok((texture, [Some(shader_resource_view.unwrap())]))
}

#[inline]
fn create_path_intermediate_msaa_texture_and_view(
    device: &ID3D11Device,
    width: u32,
    height: u32,
) -> Result<(ID3D11Texture2D, [Option<ID3D11RenderTargetView>; 1])> {
    let msaa_texture = unsafe {
        let mut output = None;
        let desc = D3D11_TEXTURE2D_DESC {
            Width: width,
            Height: height,
            MipLevels: 1,
            ArraySize: 1,
            Format: RENDER_TARGET_FORMAT,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: PATH_MULTISAMPLE_COUNT,
                Quality: D3D11_STANDARD_MULTISAMPLE_PATTERN.0 as u32,
            },
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: D3D11_BIND_RENDER_TARGET.0 as u32,
            CPUAccessFlags: 0,
            MiscFlags: 0,
        };
        device.CreateTexture2D(&desc, None, Some(&mut output))?;
        output.unwrap()
    };
    let mut msaa_view = None;
    unsafe { device.CreateRenderTargetView(&msaa_texture, None, Some(&mut msaa_view))? };
    Ok((msaa_texture, [Some(msaa_view.unwrap())]))
}

#[inline]
fn set_viewport(
    device_context: &ID3D11DeviceContext,
    width: f32,
    height: f32,
) -> [D3D11_VIEWPORT; 1] {
    let viewport = [D3D11_VIEWPORT {
        TopLeftX: 0.0,
        TopLeftY: 0.0,
        Width: width,
        Height: height,
        MinDepth: 0.0,
        MaxDepth: 1.0,
    }];
    unsafe { device_context.RSSetViewports(Some(&viewport)) };
    viewport
}

#[inline]
fn set_rasterizer_state(device: &ID3D11Device, device_context: &ID3D11DeviceContext) -> Result<()> {
    let desc = D3D11_RASTERIZER_DESC {
        FillMode: D3D11_FILL_SOLID,
        CullMode: D3D11_CULL_NONE,
        FrontCounterClockwise: false.into(),
        DepthBias: 0,
        DepthBiasClamp: 0.0,
        SlopeScaledDepthBias: 0.0,
        DepthClipEnable: true.into(),
        ScissorEnable: false.into(),
        MultisampleEnable: true.into(),
        AntialiasedLineEnable: false.into(),
    };
    let rasterizer_state = unsafe {
        let mut state = None;
        device.CreateRasterizerState(&desc, Some(&mut state))?;
        state.unwrap()
    };
    unsafe { device_context.RSSetState(&rasterizer_state) };
    Ok(())
}

/// KaminIDE patch: блендер без смешивания — источник заменяет назначение.
fn create_blend_state_replace(device: &ID3D11Device) -> Result<ID3D11BlendState> {
    let mut desc = D3D11_BLEND_DESC::default();
    desc.RenderTarget[0].BlendEnable = false.into();
    desc.RenderTarget[0].RenderTargetWriteMask = D3D11_COLOR_WRITE_ENABLE_ALL.0 as u8;
    unsafe {
        let mut state = None;
        device.CreateBlendState(&desc, Some(&mut state))?;
        Ok(state.unwrap())
    }
}

/// KaminIDE patch: блендер для премультиплированного источника.
///
/// Цвет буфера группы уже умножен на прозрачность (так его сложил обычный
/// блендер при отрисовке детей), поэтому второй раз умножать нельзя.
fn create_blend_state_premultiplied(device: &ID3D11Device) -> Result<ID3D11BlendState> {
    let mut desc = D3D11_BLEND_DESC::default();
    desc.RenderTarget[0].BlendEnable = true.into();
    desc.RenderTarget[0].BlendOp = D3D11_BLEND_OP_ADD;
    desc.RenderTarget[0].BlendOpAlpha = D3D11_BLEND_OP_ADD;
    desc.RenderTarget[0].SrcBlend = D3D11_BLEND_ONE;
    desc.RenderTarget[0].SrcBlendAlpha = D3D11_BLEND_ONE;
    desc.RenderTarget[0].DestBlend = D3D11_BLEND_INV_SRC_ALPHA;
    desc.RenderTarget[0].DestBlendAlpha = D3D11_BLEND_INV_SRC_ALPHA;
    desc.RenderTarget[0].RenderTargetWriteMask = D3D11_COLOR_WRITE_ENABLE_ALL.0 as u8;
    unsafe {
        let mut state = None;
        device.CreateBlendState(&desc, Some(&mut state))?;
        Ok(state.unwrap())
    }
}

// https://learn.microsoft.com/en-us/windows/win32/api/d3d11/ns-d3d11-d3d11_blend_desc
#[inline]
fn create_blend_state(device: &ID3D11Device) -> Result<ID3D11BlendState> {
    // If the feature level is set to greater than D3D_FEATURE_LEVEL_9_3, the display
    // device performs the blend in linear space, which is ideal.
    let mut desc = D3D11_BLEND_DESC::default();
    desc.RenderTarget[0].BlendEnable = true.into();
    desc.RenderTarget[0].BlendOp = D3D11_BLEND_OP_ADD;
    desc.RenderTarget[0].BlendOpAlpha = D3D11_BLEND_OP_ADD;
    desc.RenderTarget[0].SrcBlend = D3D11_BLEND_SRC_ALPHA;
    desc.RenderTarget[0].SrcBlendAlpha = D3D11_BLEND_ONE;
    desc.RenderTarget[0].DestBlend = D3D11_BLEND_INV_SRC_ALPHA;
    desc.RenderTarget[0].DestBlendAlpha = D3D11_BLEND_ONE;
    desc.RenderTarget[0].RenderTargetWriteMask = D3D11_COLOR_WRITE_ENABLE_ALL.0 as u8;
    unsafe {
        let mut state = None;
        device.CreateBlendState(&desc, Some(&mut state))?;
        Ok(state.unwrap())
    }
}

fn create_blend_state_for_path_rasterization(device: &ID3D11Device) -> Result<ID3D11BlendState> {
    // If the feature level is set to greater than D3D_FEATURE_LEVEL_9_3, the display
    // device performs the blend in linear space, which is ideal.
    let mut desc = D3D11_BLEND_DESC::default();
    desc.RenderTarget[0].BlendEnable = true.into();
    desc.RenderTarget[0].BlendOp = D3D11_BLEND_OP_ADD;
    desc.RenderTarget[0].BlendOpAlpha = D3D11_BLEND_OP_ADD;
    desc.RenderTarget[0].SrcBlend = D3D11_BLEND_ONE;
    desc.RenderTarget[0].SrcBlendAlpha = D3D11_BLEND_ONE;
    desc.RenderTarget[0].DestBlend = D3D11_BLEND_INV_SRC_ALPHA;
    desc.RenderTarget[0].DestBlendAlpha = D3D11_BLEND_INV_SRC_ALPHA;
    desc.RenderTarget[0].RenderTargetWriteMask = D3D11_COLOR_WRITE_ENABLE_ALL.0 as u8;
    unsafe {
        let mut state = None;
        device.CreateBlendState(&desc, Some(&mut state))?;
        Ok(state.unwrap())
    }
}

#[inline]
fn create_blend_state_for_path_sprite(device: &ID3D11Device) -> Result<ID3D11BlendState> {
    // If the feature level is set to greater than D3D_FEATURE_LEVEL_9_3, the display
    // device performs the blend in linear space, which is ideal.
    let mut desc = D3D11_BLEND_DESC::default();
    desc.RenderTarget[0].BlendEnable = true.into();
    desc.RenderTarget[0].BlendOp = D3D11_BLEND_OP_ADD;
    desc.RenderTarget[0].BlendOpAlpha = D3D11_BLEND_OP_ADD;
    desc.RenderTarget[0].SrcBlend = D3D11_BLEND_ONE;
    desc.RenderTarget[0].SrcBlendAlpha = D3D11_BLEND_ONE;
    desc.RenderTarget[0].DestBlend = D3D11_BLEND_INV_SRC_ALPHA;
    desc.RenderTarget[0].DestBlendAlpha = D3D11_BLEND_ONE;
    desc.RenderTarget[0].RenderTargetWriteMask = D3D11_COLOR_WRITE_ENABLE_ALL.0 as u8;
    unsafe {
        let mut state = None;
        device.CreateBlendState(&desc, Some(&mut state))?;
        Ok(state.unwrap())
    }
}

#[inline]
fn create_vertex_shader(device: &ID3D11Device, bytes: &[u8]) -> Result<ID3D11VertexShader> {
    unsafe {
        let mut shader = None;
        device.CreateVertexShader(bytes, None, Some(&mut shader))?;
        Ok(shader.unwrap())
    }
}

#[inline]
fn create_fragment_shader(device: &ID3D11Device, bytes: &[u8]) -> Result<ID3D11PixelShader> {
    unsafe {
        let mut shader = None;
        device.CreatePixelShader(bytes, None, Some(&mut shader))?;
        Ok(shader.unwrap())
    }
}

#[inline]
fn create_buffer(
    device: &ID3D11Device,
    element_size: usize,
    buffer_size: usize,
) -> Result<ID3D11Buffer> {
    let desc = D3D11_BUFFER_DESC {
        ByteWidth: (element_size * buffer_size) as u32,
        Usage: D3D11_USAGE_DYNAMIC,
        BindFlags: D3D11_BIND_SHADER_RESOURCE.0 as u32,
        CPUAccessFlags: D3D11_CPU_ACCESS_WRITE.0 as u32,
        MiscFlags: D3D11_RESOURCE_MISC_BUFFER_STRUCTURED.0 as u32,
        StructureByteStride: element_size as u32,
    };
    let mut buffer = None;
    unsafe { device.CreateBuffer(&desc, None, Some(&mut buffer)) }?;
    Ok(buffer.unwrap())
}

#[inline]
fn create_buffer_view(
    device: &ID3D11Device,
    buffer: &ID3D11Buffer,
) -> Result<[Option<ID3D11ShaderResourceView>; 1]> {
    let mut view = None;
    unsafe { device.CreateShaderResourceView(buffer, None, Some(&mut view)) }?;
    Ok([view])
}

#[inline]
fn update_buffer<T>(
    device_context: &ID3D11DeviceContext,
    buffer: &ID3D11Buffer,
    data: &[T],
) -> Result<()> {
    unsafe {
        let mut dest = std::mem::zeroed();
        device_context.Map(buffer, 0, D3D11_MAP_WRITE_DISCARD, 0, Some(&mut dest))?;
        std::ptr::copy_nonoverlapping(data.as_ptr(), dest.pData as _, data.len());
        device_context.Unmap(buffer, 0);
    }
    Ok(())
}

#[inline]
fn set_pipeline_state(
    device_context: &ID3D11DeviceContext,
    buffer_view: &[Option<ID3D11ShaderResourceView>],
    topology: D3D_PRIMITIVE_TOPOLOGY,
    viewport: &[D3D11_VIEWPORT],
    vertex_shader: &ID3D11VertexShader,
    fragment_shader: &ID3D11PixelShader,
    global_params: &[Option<ID3D11Buffer>],
    blend_state: &ID3D11BlendState,
) {
    unsafe {
        device_context.VSSetShaderResources(1, Some(buffer_view));
        device_context.PSSetShaderResources(1, Some(buffer_view));
        device_context.IASetPrimitiveTopology(topology);
        device_context.RSSetViewports(Some(viewport));
        device_context.VSSetShader(vertex_shader, None);
        device_context.PSSetShader(fragment_shader, None);
        device_context.VSSetConstantBuffers(0, Some(global_params));
        device_context.PSSetConstantBuffers(0, Some(global_params));
        device_context.OMSetBlendState(blend_state, None, 0xFFFFFFFF);
    }
}

#[cfg(debug_assertions)]
fn report_live_objects(device: &ID3D11Device) -> Result<()> {
    let debug_device: ID3D11Debug = device.cast()?;
    unsafe {
        debug_device.ReportLiveDeviceObjects(D3D11_RLDO_DETAIL)?;
    }
    Ok(())
}

const BUFFER_COUNT: usize = 3;

pub(crate) mod shader_resources {
    use anyhow::Result;

    #[cfg(debug_assertions)]
    use windows::{
        Win32::Graphics::Direct3D::{
            Fxc::{D3DCOMPILE_DEBUG, D3DCOMPILE_SKIP_OPTIMIZATION, D3DCompileFromFile},
            ID3DBlob,
        },
        core::{HSTRING, PCSTR},
    };

    #[derive(Copy, Clone, Debug, Eq, PartialEq)]
    pub(crate) enum ShaderModule {
        Quad,
        Shadow,
        Underline,
        PathRasterization,
        PathSprite,
        MonochromeSprite,
        PolychromeSprite,
        EmojiRasterization,
        /// KaminIDE patch: backdrop blur (тосты).
        Blur,
    }

    #[derive(Copy, Clone, Debug, Eq, PartialEq)]
    pub(crate) enum ShaderTarget {
        Vertex,
        Fragment,
    }

    pub(crate) struct RawShaderBytes<'t> {
        inner: &'t [u8],

        #[cfg(debug_assertions)]
        _blob: ID3DBlob,
    }

    impl<'t> RawShaderBytes<'t> {
        pub(crate) fn new(module: ShaderModule, target: ShaderTarget) -> Result<Self> {
            #[cfg(not(debug_assertions))]
            {
                Ok(Self::from_bytes(module, target))
            }
            #[cfg(debug_assertions)]
            {
                let blob = build_shader_blob(module, target)?;
                let inner = unsafe {
                    std::slice::from_raw_parts(
                        blob.GetBufferPointer() as *const u8,
                        blob.GetBufferSize(),
                    )
                };
                Ok(Self { inner, _blob: blob })
            }
        }

        pub(crate) fn as_bytes(&'t self) -> &'t [u8] {
            self.inner
        }

        #[cfg(not(debug_assertions))]
        fn from_bytes(module: ShaderModule, target: ShaderTarget) -> Self {
            let bytes = match module {
                ShaderModule::Quad => match target {
                    ShaderTarget::Vertex => QUAD_VERTEX_BYTES,
                    ShaderTarget::Fragment => QUAD_FRAGMENT_BYTES,
                },
                ShaderModule::Shadow => match target {
                    ShaderTarget::Vertex => SHADOW_VERTEX_BYTES,
                    ShaderTarget::Fragment => SHADOW_FRAGMENT_BYTES,
                },
                ShaderModule::Underline => match target {
                    ShaderTarget::Vertex => UNDERLINE_VERTEX_BYTES,
                    ShaderTarget::Fragment => UNDERLINE_FRAGMENT_BYTES,
                },
                ShaderModule::PathRasterization => match target {
                    ShaderTarget::Vertex => PATH_RASTERIZATION_VERTEX_BYTES,
                    ShaderTarget::Fragment => PATH_RASTERIZATION_FRAGMENT_BYTES,
                },
                ShaderModule::PathSprite => match target {
                    ShaderTarget::Vertex => PATH_SPRITE_VERTEX_BYTES,
                    ShaderTarget::Fragment => PATH_SPRITE_FRAGMENT_BYTES,
                },
                ShaderModule::MonochromeSprite => match target {
                    ShaderTarget::Vertex => MONOCHROME_SPRITE_VERTEX_BYTES,
                    ShaderTarget::Fragment => MONOCHROME_SPRITE_FRAGMENT_BYTES,
                },
                ShaderModule::PolychromeSprite => match target {
                    ShaderTarget::Vertex => POLYCHROME_SPRITE_VERTEX_BYTES,
                    ShaderTarget::Fragment => POLYCHROME_SPRITE_FRAGMENT_BYTES,
                },
                ShaderModule::EmojiRasterization => match target {
                    ShaderTarget::Vertex => EMOJI_RASTERIZATION_VERTEX_BYTES,
                    ShaderTarget::Fragment => EMOJI_RASTERIZATION_FRAGMENT_BYTES,
                },
                ShaderModule::Blur => match target {
                    ShaderTarget::Vertex => BLUR_VERTEX_BYTES,
                    ShaderTarget::Fragment => BLUR_FRAGMENT_BYTES,
                },
            };
            Self { inner: bytes }
        }
    }

    #[cfg(debug_assertions)]
    pub(super) fn build_shader_blob(entry: ShaderModule, target: ShaderTarget) -> Result<ID3DBlob> {
        unsafe {
            use windows::Win32::Graphics::{
                Direct3D::ID3DInclude, Hlsl::D3D_COMPILE_STANDARD_FILE_INCLUDE,
            };

            let shader_name = if matches!(entry, ShaderModule::EmojiRasterization) {
                "color_text_raster.hlsl"
            } else {
                "shaders.hlsl"
            };

            let entry = format!(
                "{}_{}\0",
                entry.as_str(),
                match target {
                    ShaderTarget::Vertex => "vertex",
                    ShaderTarget::Fragment => "fragment",
                }
            );
            let target = match target {
                ShaderTarget::Vertex => "vs_4_1\0",
                ShaderTarget::Fragment => "ps_4_1\0",
            };

            let mut compile_blob = None;
            let mut error_blob = None;
            let shader_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join(&format!("src/platform/windows/{}", shader_name))
                .canonicalize()?;

            let entry_point = PCSTR::from_raw(entry.as_ptr());
            let target_cstr = PCSTR::from_raw(target.as_ptr());

            // really dirty trick because winapi bindings are unhappy otherwise
            let include_handler = &std::mem::transmute::<usize, ID3DInclude>(
                D3D_COMPILE_STANDARD_FILE_INCLUDE as usize,
            );

            let ret = D3DCompileFromFile(
                &HSTRING::from(shader_path.to_str().unwrap()),
                None,
                include_handler,
                entry_point,
                target_cstr,
                D3DCOMPILE_DEBUG | D3DCOMPILE_SKIP_OPTIMIZATION,
                0,
                &mut compile_blob,
                Some(&mut error_blob),
            );
            if ret.is_err() {
                let Some(error_blob) = error_blob else {
                    return Err(anyhow::anyhow!("{ret:?}"));
                };

                let error_string =
                    std::ffi::CStr::from_ptr(error_blob.GetBufferPointer() as *const i8)
                        .to_string_lossy();
                log::error!("Shader compile error: {}", error_string);
                return Err(anyhow::anyhow!("Compile error: {}", error_string));
            }
            Ok(compile_blob.unwrap())
        }
    }

    #[cfg(not(debug_assertions))]
    include!(concat!(env!("OUT_DIR"), "/shaders_bytes.rs"));

    #[cfg(debug_assertions)]
    impl ShaderModule {
        pub fn as_str(&self) -> &str {
            match self {
                ShaderModule::Quad => "quad",
                ShaderModule::Shadow => "shadow",
                ShaderModule::Underline => "underline",
                ShaderModule::PathRasterization => "path_rasterization",
                ShaderModule::PathSprite => "path_sprite",
                ShaderModule::MonochromeSprite => "monochrome_sprite",
                ShaderModule::PolychromeSprite => "polychrome_sprite",
                ShaderModule::EmojiRasterization => "emoji_rasterization",
                ShaderModule::Blur => "blur",
            }
        }
    }
}

mod nvidia {
    use std::{
        ffi::CStr,
        os::raw::{c_char, c_int, c_uint},
    };

    use anyhow::Result;
    use windows::{Win32::System::LibraryLoader::GetProcAddress, core::s};

    use crate::with_dll_library;

    // https://github.com/NVIDIA/nvapi/blob/7cb76fce2f52de818b3da497af646af1ec16ce27/nvapi_lite_common.h#L180
    const NVAPI_SHORT_STRING_MAX: usize = 64;

    // https://github.com/NVIDIA/nvapi/blob/7cb76fce2f52de818b3da497af646af1ec16ce27/nvapi_lite_common.h#L235
    #[allow(non_camel_case_types)]
    type NvAPI_ShortString = [c_char; NVAPI_SHORT_STRING_MAX];

    // https://github.com/NVIDIA/nvapi/blob/7cb76fce2f52de818b3da497af646af1ec16ce27/nvapi_lite_common.h#L447
    #[allow(non_camel_case_types)]
    type NvAPI_SYS_GetDriverAndBranchVersion_t = unsafe extern "C" fn(
        driver_version: *mut c_uint,
        build_branch_string: *mut NvAPI_ShortString,
    ) -> c_int;

    pub(super) fn get_driver_version() -> Result<String> {
        #[cfg(target_pointer_width = "64")]
        let nvidia_dll_name = s!("nvapi64.dll");
        #[cfg(target_pointer_width = "32")]
        let nvidia_dll_name = s!("nvapi.dll");

        with_dll_library(nvidia_dll_name, |nvidia_dll| unsafe {
            let nvapi_query_addr = GetProcAddress(nvidia_dll, s!("nvapi_QueryInterface"))
                .ok_or_else(|| anyhow::anyhow!("Failed to get nvapi_QueryInterface address"))?;
            let nvapi_query: extern "C" fn(u32) -> *mut () = std::mem::transmute(nvapi_query_addr);

            // https://github.com/NVIDIA/nvapi/blob/7cb76fce2f52de818b3da497af646af1ec16ce27/nvapi_interface.h#L41
            let nvapi_get_driver_version_ptr = nvapi_query(0x2926aaad);
            if nvapi_get_driver_version_ptr.is_null() {
                anyhow::bail!("Failed to get NVIDIA driver version function pointer");
            }
            let nvapi_get_driver_version: NvAPI_SYS_GetDriverAndBranchVersion_t =
                std::mem::transmute(nvapi_get_driver_version_ptr);

            let mut driver_version: c_uint = 0;
            let mut build_branch_string: NvAPI_ShortString = [0; NVAPI_SHORT_STRING_MAX];
            let result = nvapi_get_driver_version(
                &mut driver_version as *mut c_uint,
                &mut build_branch_string as *mut NvAPI_ShortString,
            );

            if result != 0 {
                anyhow::bail!(
                    "Failed to get NVIDIA driver version, error code: {}",
                    result
                );
            }
            let major = driver_version / 100;
            let minor = driver_version % 100;
            let branch_string = CStr::from_ptr(build_branch_string.as_ptr());
            Ok(format!(
                "{}.{} {}",
                major,
                minor,
                branch_string.to_string_lossy()
            ))
        })
    }
}

mod amd {
    use std::os::raw::{c_char, c_int, c_void};

    use anyhow::Result;
    use windows::{Win32::System::LibraryLoader::GetProcAddress, core::s};

    use crate::with_dll_library;

    // https://github.com/GPUOpen-LibrariesAndSDKs/AGS_SDK/blob/5d8812d703d0335741b6f7ffc37838eeb8b967f7/ags_lib/inc/amd_ags.h#L145
    const AGS_CURRENT_VERSION: i32 = (6 << 22) | (3 << 12);

    // https://github.com/GPUOpen-LibrariesAndSDKs/AGS_SDK/blob/5d8812d703d0335741b6f7ffc37838eeb8b967f7/ags_lib/inc/amd_ags.h#L204
    // This is an opaque type, using struct to represent it properly for FFI
    #[repr(C)]
    struct AGSContext {
        _private: [u8; 0],
    }

    #[repr(C)]
    pub struct AGSGPUInfo {
        pub driver_version: *const c_char,
        pub radeon_software_version: *const c_char,
        pub num_devices: c_int,
        pub devices: *mut c_void,
    }

    // https://github.com/GPUOpen-LibrariesAndSDKs/AGS_SDK/blob/5d8812d703d0335741b6f7ffc37838eeb8b967f7/ags_lib/inc/amd_ags.h#L429
    #[allow(non_camel_case_types)]
    type agsInitialize_t = unsafe extern "C" fn(
        version: c_int,
        config: *const c_void,
        context: *mut *mut AGSContext,
        gpu_info: *mut AGSGPUInfo,
    ) -> c_int;

    // https://github.com/GPUOpen-LibrariesAndSDKs/AGS_SDK/blob/5d8812d703d0335741b6f7ffc37838eeb8b967f7/ags_lib/inc/amd_ags.h#L436
    #[allow(non_camel_case_types)]
    type agsDeInitialize_t = unsafe extern "C" fn(context: *mut AGSContext) -> c_int;

    pub(super) fn get_driver_version() -> Result<String> {
        #[cfg(target_pointer_width = "64")]
        let amd_dll_name = s!("amd_ags_x64.dll");
        #[cfg(target_pointer_width = "32")]
        let amd_dll_name = s!("amd_ags_x86.dll");

        with_dll_library(amd_dll_name, |amd_dll| unsafe {
            let ags_initialize_addr = GetProcAddress(amd_dll, s!("agsInitialize"))
                .ok_or_else(|| anyhow::anyhow!("Failed to get agsInitialize address"))?;
            let ags_deinitialize_addr = GetProcAddress(amd_dll, s!("agsDeInitialize"))
                .ok_or_else(|| anyhow::anyhow!("Failed to get agsDeInitialize address"))?;

            let ags_initialize: agsInitialize_t = std::mem::transmute(ags_initialize_addr);
            let ags_deinitialize: agsDeInitialize_t = std::mem::transmute(ags_deinitialize_addr);

            let mut context: *mut AGSContext = std::ptr::null_mut();
            let mut gpu_info: AGSGPUInfo = AGSGPUInfo {
                driver_version: std::ptr::null(),
                radeon_software_version: std::ptr::null(),
                num_devices: 0,
                devices: std::ptr::null_mut(),
            };

            let result = ags_initialize(
                AGS_CURRENT_VERSION,
                std::ptr::null(),
                &mut context,
                &mut gpu_info,
            );
            if result != 0 {
                anyhow::bail!("Failed to initialize AMD AGS, error code: {}", result);
            }

            // Vulkan actually returns this as the driver version
            let software_version = if !gpu_info.radeon_software_version.is_null() {
                std::ffi::CStr::from_ptr(gpu_info.radeon_software_version)
                    .to_string_lossy()
                    .into_owned()
            } else {
                "Unknown Radeon Software Version".to_string()
            };

            let driver_version = if !gpu_info.driver_version.is_null() {
                std::ffi::CStr::from_ptr(gpu_info.driver_version)
                    .to_string_lossy()
                    .into_owned()
            } else {
                "Unknown Radeon Driver Version".to_string()
            };

            ags_deinitialize(context);
            Ok(format!("{} ({})", software_version, driver_version))
        })
    }
}

mod dxgi {
    use windows::{
        Win32::Graphics::Dxgi::{IDXGIAdapter1, IDXGIDevice},
        core::Interface,
    };

    pub(super) fn get_driver_version(adapter: &IDXGIAdapter1) -> anyhow::Result<String> {
        let number = unsafe { adapter.CheckInterfaceSupport(&IDXGIDevice::IID as _) }?;
        Ok(format!(
            "{}.{}.{}.{}",
            number >> 48,
            (number >> 32) & 0xFFFF,
            (number >> 16) & 0xFFFF,
            number & 0xFFFF
        ))
    }
}
