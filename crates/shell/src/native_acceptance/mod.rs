//! Explicit development-only acceptance controls; no Bridge credentials required.

#[cfg(all(debug_assertions, feature = "probe"))]
mod commands;
pub(crate) mod config;
#[cfg(all(debug_assertions, windows))]
pub(crate) mod device;
#[cfg(debug_assertions)]
pub(crate) mod faults;

#[cfg(all(debug_assertions, feature = "probe"))]
pub(crate) use commands::handle;

#[cfg(debug_assertions)]
#[derive(Clone)]
pub(crate) enum Action {
    Loaders,
    Waiting,
    Ready,
    Error,
    Switch,
    Clear,
    DeviceLoss,
}

#[cfg(debug_assertions)]
pub(crate) const VIEW: &str = "kaminDebugAcceptance";
#[cfg(debug_assertions)]
pub(crate) const TOOL: &str = "kaminDebugAcceptanceTool";
#[cfg(debug_assertions)]
pub(crate) const LOADERS: &str = "kaminDebugLoaders";

#[cfg(debug_assertions)]
static ACTIVE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
#[cfg(debug_assertions)]
pub(crate) fn enable() {
    ACTIVE.store(true, std::sync::atomic::Ordering::Relaxed);
}
#[cfg(debug_assertions)]
pub(crate) fn active() -> bool {
    ACTIVE.load(std::sync::atomic::Ordering::Relaxed)
}

#[cfg(debug_assertions)]
pub(crate) fn tool(id: &str) -> Option<crate::activity::DynTool> {
    (id == TOOL).then(|| crate::activity::DynTool {
        id: TOOL.into(),
        label: "Native acceptance".into(),
        icon: "beaker".into(),
        location: "activitybar".into(),
        views: vec![crate::activity::DynView {
            id: VIEW.into(),
            name: "Synthetic provider".into(),
            webview: true,
        }],
    })
}

#[cfg(debug_assertions)]
pub(crate) fn startup() {
    let config = config::Config::from_env();
    if config.loaders {
        enable();
        if let Some(tx) = crate::host_link::event_tx() {
            let _ = tx.try_send(crate::host_link::ShellEvent::NativeAcceptance(
                Action::Loaders,
            ));
        }
    }
}
