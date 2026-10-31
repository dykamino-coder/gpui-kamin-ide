//! Политика GPU шелла: конфигурация процесса и журнал фактического устройства.

#[cfg(windows)]
#[allow(
    clippy::disallowed_methods,
    reason = "shell launch reads GPU configuration"
)]
fn preference() -> anyhow::Result<gpui_windows::GpuAdapterPreference> {
    policy(std::env::var("KAMIN_GPU_ADAPTER").ok().as_deref())
}

#[cfg(windows)]
fn policy(value: Option<&str>) -> anyhow::Result<gpui_windows::GpuAdapterPreference> {
    gpui_windows::GpuAdapterPreference::parse(value.unwrap_or("minimum-power"))
}

pub fn run(on_launch: impl FnOnce(&mut gpui::App) + 'static) {
    #[cfg(windows)]
    let (app, platform, requested) = {
        let requested = preference().unwrap_or_else(|error| fail(error));
        let platform = std::rc::Rc::new(
            gpui_windows::WindowsPlatform::with_gpu_adapter(false, requested.clone())
                .unwrap_or_else(|error| fail(error)),
        );
        (
            gpui::Application::with_platform(platform.clone()),
            platform,
            requested,
        )
    };
    #[cfg(not(windows))]
    let app = gpui_platform::application();
    app.with_instrumentation(super::instrumentation())
        .with_assets(super::assets::Assets)
        .run(move |cx| {
            #[cfg(windows)]
            match platform.gpu_device_context() {
                Ok(actual) => crate::web::startup_log(format!(
                    "[boot] adapter selected policy={requested:?} name={:?} luid={:08X}:{:08X} vendor={:04X} device={:04X} umd={} feature_level={:X} flags={:X} generation={}",
                    actual.device_name, actual.luid_high_part as u32, actual.luid_low_part,
                    actual.vendor_id, actual.device_id, actual.driver_version,
                    actual.feature_level, actual.creation_flags, actual.generation,
                )),
                Err(error) => fail(error),
            }
            on_launch(cx);
        });
}

#[cfg(windows)]
fn fail(error: anyhow::Error) -> ! {
    crate::web::startup_log(format!("[boot] adapter error: {error:#}"));
    std::process::exit(2)
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use gpui_windows::GpuAdapterPreference as Gpu;
    #[test]
    fn product_defaults_to_minimum_power_and_accepts_explicit_configuration() {
        assert_eq!(policy(None).unwrap(), Gpu::MinimumPower);
        assert_eq!(
            policy(Some("high-performance")).unwrap(),
            Gpu::HighPerformance
        );
        assert_eq!(
            policy(Some("luid:00000000:000150BD")).unwrap(),
            Gpu::ByLuid {
                high_part: 0,
                low_part: 86205
            }
        );
        assert!(policy(Some("")).is_err());
    }
}
