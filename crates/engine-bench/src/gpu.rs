//! Каталог DXGI для CLI и разрешения имени адаптера в браузерном стенде.

use gpui_windows::{GpuAdapterInfo, GpuAdapterPreference, gpu_adapters};
use serde_json::{Value, json};

pub(super) fn catalog() -> Result<(), Box<dyn std::error::Error>> {
    let mut policies = serde_json::Map::new();
    for name in ["default", "high-performance", "minimum-power"] {
        let adapters = gpu_adapters(&GpuAdapterPreference::parse(name)?)?;
        policies.insert(
            name.into(),
            json!(adapters.iter().map(info).collect::<Vec<_>>()),
        );
    }
    println!("{}", serde_json::to_string_pretty(&policies)?);
    Ok(())
}

fn info(adapter: &GpuAdapterInfo) -> Value {
    json!({"device_name": adapter.device_name, "driver_version": adapter.driver_version,
        "vendor_id": adapter.vendor_id, "device_id": adapter.device_id,
        "luid": {"high_part": adapter.luid_high_part, "low_part": adapter.luid_low_part,
            "hex": format!("{:08X}:{:08X}", adapter.luid_high_part as u32, adapter.luid_low_part)}})
}
