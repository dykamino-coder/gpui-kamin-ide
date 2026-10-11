//! Build replacements off to the side: failed allocations drop only local owners.

use super::*;

pub(super) struct RecoveryResources {
    pub(super) devices: DirectXRendererDevices,
    pub(super) resources: DirectXResources,
    pub(super) globals: DirectXGlobalElements,
    pub(super) pipelines: DirectXRenderPipelines,
    pub(super) direct_composition: Option<DirectComposition>,
}

impl RecoveryResources {
    pub(super) fn new(
        directx_devices: &DirectXDevices,
        width: u32,
        height: u32,
        hwnd: HWND,
        disable_direct_composition: bool,
    ) -> Result<Self> {
        let devices = DirectXRendererDevices::new(directx_devices, disable_direct_composition)
            .context("Recreating DirectX devices")?;
        let resources =
            DirectXResources::new(&devices, width, height, hwnd, disable_direct_composition)
                .context("Creating DirectX resources")?;
        let globals = DirectXGlobalElements::new(&devices.device)
            .context("Creating DirectXGlobalElements")?;
        let pipelines = DirectXRenderPipelines::new(&devices.device)
            .context("Creating DirectXRenderPipelines")?;
        let direct_composition = if disable_direct_composition {
            None
        } else {
            let composition = DirectComposition::new(devices.dxgi_device.as_ref().unwrap(), hwnd)?;
            composition.set_swap_chain(&resources.swap_chain)?;
            Some(composition)
        };
        Ok(Self {
            devices,
            resources,
            globals,
            pipelines,
            direct_composition,
        })
    }
}

impl DirectXRenderer {
    pub(super) fn handle_device_lost_impl(
        &mut self,
        directx_devices: &DirectXDevices,
    ) -> Result<()> {
        let (width, height, hwnd) = (self.width, self.height, self.hwnd);
        self.recover_with(|disabled| {
            RecoveryResources::new(directx_devices, width, height, hwnd, disabled)
        })
    }

    pub(super) fn recover_with(
        &mut self,
        recreate: impl FnOnce(bool) -> Result<RecoveryResources>,
    ) -> Result<()> {
        // Failed allocation must leave drawing disabled, including after retry exhaustion.
        self.skip_draws = true;
        self.clear_effect_resources();
        unsafe {
            #[cfg(debug_assertions)]
            if let Some(devices) = &self.devices {
                report_live_objects(&devices.device)
                    .context("Failed to report live objects after device lost")
                    .log_err();
            }

            self.resources.take();
            if let Some(devices) = &self.devices {
                devices.device_context.OMSetRenderTargets(None, None);
                devices.device_context.ClearState();
                devices.device_context.Flush();
                #[cfg(debug_assertions)]
                report_live_objects(&devices.device)
                    .context("Failed to report live objects after device lost")
                    .log_err();
            }

            self.direct_composition.take();
            self.devices.take();
        }

        let RecoveryResources {
            devices,
            resources,
            globals,
            pipelines,
            direct_composition,
        } = recreate(self.disable_direct_composition)?;

        self.atlas
            .handle_device_lost(&devices.device, &devices.device_context);

        unsafe {
            devices
                .device_context
                .OMSetRenderTargets(Some(slice::from_ref(&resources.render_target_view)), None);
        }
        self.devices = Some(devices);
        self.resources = Some(resources);
        self.globals = globals;
        self.pipelines = pipelines;
        self.direct_composition = direct_composition;
        Ok(())
    }

    pub(super) fn clear_effect_resources(&mut self) {
        // Every effect target, mask and blend state belongs to the old device.
        self.blur = BlurScratch::default();
        self.group_target = None;
        self.group_target_tex = None;
        self.blend_premultiplied = None;
        self.blend_replace = None;
    }

    pub(super) fn ensure_drawable_resources(&self) -> Result<()> {
        let resources = self.resources.as_ref().context("resources missing")?;
        anyhow::ensure!(self.devices.is_some(), "devices missing");
        anyhow::ensure!(
            resources.render_target.is_some() && resources.render_target_view.is_some(),
            "render target unavailable after failed resize"
        );
        Ok(())
    }
}
