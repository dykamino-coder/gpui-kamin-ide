//! Граница Windows host: разрешить policy один раз и передать платформе строгий LUID.

use gpui_windows::{GpuAdapterPreference as Preference, WindowsPlatform, gpu_adapters};
use serde_json::json;
use std::rc::Rc;

pub(super) struct Host {
    platform: Rc<WindowsPlatform>,
    pub(super) requested: String,
    pub(super) resolved: Preference,
}

impl Host {
    pub(super) fn new(args: &mut Vec<String>) -> Result<Self, Box<dyn std::error::Error>> {
        let catalog = args.iter().any(|arg| arg == "--gpu-catalog");
        let mut value = std::env::var("WPT_GPU").unwrap_or_else(|_| "minimum-power".into());
        args.retain(|arg| {
            if let Some(option) = arg.strip_prefix("--gpu=") {
                value = option.into();
                false
            } else {
                arg != "--gpu-catalog"
            }
        });
        let preference = Preference::parse(&value)?;
        let adapters = gpu_adapters(&preference)?;
        if catalog {
            let rows: Vec<_> = adapters.iter().map(|info| json!({
                "name": info.device_name, "vendor_id": info.vendor_id, "device_id": info.device_id,
                "driver_version": info.driver_version,
                "selector": format!("luid:{:08X}:{:08X}", info.luid_high_part as u32, info.luid_low_part),
            })).collect();
            println!("{}", json!({"adapters": rows}));
            std::process::exit(0);
        }
        let first = adapters
            .first()
            .ok_or("GPU policy matched no hardware adapter")?;
        let resolved = Preference::ByLuid {
            high_part: first.luid_high_part,
            low_part: first.luid_low_part,
        };
        let platform = Rc::new(WindowsPlatform::with_gpu_adapter(false, resolved.clone())?);
        Ok(Self {
            platform,
            requested: value,
            resolved,
        })
    }

    pub(super) fn application(&self) -> gpui::Application {
        gpui::Application::with_platform(self.platform.clone())
    }
}
