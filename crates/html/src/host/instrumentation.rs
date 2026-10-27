//! Чтение настроек диагностического запуска на границе host.

mod debug_device;

use debug_device::DeviceOverride;
use gpui::{DebugDevice, InstrumentationConfig};

#[allow(
    clippy::disallowed_methods,
    reason = "host launch reads process configuration"
)]
pub fn from_env() -> InstrumentationConfig {
    InstrumentationConfig {
        prepaint: std::env::var("KAMIN_PREPAINT_PROF").as_deref() == Ok("1"),
        gpu_timing: std::env::var("KAMIN_DIAG_GPU_TIMING").as_deref() == Ok("1"),
        scale_factor: std::env::var("KAMIN_FORCE_SCALE")
            .ok()
            .and_then(|v| v.parse().ok()),
        debug_device: match DeviceOverride::from_env() {
            DeviceOverride::None => DebugDevice::Default,
            DeviceOverride::Warp => DebugDevice::Warp,
            DeviceOverride::D3D11On12 => DebugDevice::D3D11On12,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn explicit_instances_isolate_prepaint_and_batch_measurements() {
        let a = gpui::Instrumentation::new(InstrumentationConfig {
            prepaint: true,
            ..Default::default()
        });
        let b = gpui::Instrumentation::new(a.config());
        let clone = a.clone();
        {
            let _guard = clone.prepaint.enter("test::OwnedElement");
        }
        assert!(b.prepaint.take_top(10).is_empty());
        let top = a.prepaint.take_top(10);
        assert_eq!(top.len(), 1);
        assert_eq!((&*top[0].0, top[0].2), ("OwnedElement", 1));
        a.batches.frame();
        a.batches.count(1, 3, 12.);
        assert!(b.batches.take_line().is_none());
        let line = clone.batches.take_line().unwrap();
        assert!(line.contains("frames=1"));
        assert!(line.contains("quads: n=3"));
        assert!(a.batches.take_line().is_none());
        assert!(
            gpui::Instrumentation::default()
                .prepaint
                .enter("disabled")
                .is_none()
        );
    }

    #[test]
    fn prepaint_unwind_restores_its_own_stack() {
        let profile = gpui::Instrumentation::new(InstrumentationConfig {
            prepaint: true,
            ..Default::default()
        })
        .prepaint;
        let result = std::panic::catch_unwind(|| {
            let _outer = profile.enter("outer");
            let _inner = profile.enter("inner");
            panic!("cancel prepaint");
        });
        assert!(result.is_err());
        {
            let _next = profile.enter("next");
        }
        let top = profile.take_top(10);
        assert_eq!(top.len(), 3);
        assert!(top.iter().all(|entry| entry.2 == 1));
    }
}
