//! KaminIDE patch: OLE drop target of a window, revoked while its HWND is alive.
//!
//! `RegisterDragDrop` keeps a reference to the `IDropTarget`, and through it
//! the `Rc` of the window, until `RevokeDragDrop`, which only works on a live
//! HWND. A window destroyed by the system (`WM_CLOSE` → `DefWindowProc`) is
//! gone before `WindowsWindow::drop` runs its deferred revoke; the revoke
//! failed, the `Rc` was never released and the shutdown drain waited forever
//! (INC-2026-0122). The window therefore revokes in `WM_DESTROY`, once.

use std::cell::Cell;

use anyhow::{Context as _, Result};
use gpui_util::ResultExt;
use windows::Win32::{
    Foundation::HWND,
    System::Ole::{IDropTarget, RegisterDragDrop, RevokeDragDrop},
};

/// Whether this window's drop target is registered with OLE.
#[derive(Default)]
pub(crate) struct DropTarget(Cell<bool>);

impl DropTarget {
    /// Registers `target` for `hwnd`; OLE holds it until [`Self::revoke`].
    pub(crate) fn register(&self, hwnd: HWND, target: &IDropTarget) -> Result<()> {
        unsafe { RegisterDragDrop(hwnd, target) }.context("unable to register drag-drop event")?;
        self.0.set(true);
        Ok(())
    }

    /// Releases the target. Must run while `hwnd` is alive; later calls do nothing.
    pub(crate) fn revoke(&self, hwnd: HWND) {
        if self.0.replace(false) {
            unsafe { RevokeDragDrop(hwnd) }.log_err();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::rc::Rc;
    use windows::Win32::{
        Foundation::{LPARAM, LRESULT, POINTL, WPARAM},
        System::{
            Com::IDataObject,
            LibraryLoader::GetModuleHandleW,
            Ole::{DROPEFFECT, OleInitialize},
            SystemServices::MODIFIERKEYS_FLAGS,
        },
        UI::WindowsAndMessaging::*,
    };
    use windows::core::{Ref, implement, w};

    /// A drop target that owns a share of `Rc`, as the window's handler does.
    #[implement(IDropTarget)]
    struct Probe(#[allow(dead_code)] Rc<()>);

    #[allow(non_snake_case)]
    impl windows::Win32::System::Ole::IDropTarget_Impl for Probe_Impl {
        fn DragEnter(
            &self,
            _: Ref<IDataObject>,
            _: MODIFIERKEYS_FLAGS,
            _: &POINTL,
            _: *mut DROPEFFECT,
        ) -> windows::core::Result<()> {
            Ok(())
        }
        fn DragOver(
            &self,
            _: MODIFIERKEYS_FLAGS,
            _: &POINTL,
            _: *mut DROPEFFECT,
        ) -> windows::core::Result<()> {
            Ok(())
        }
        fn DragLeave(&self) -> windows::core::Result<()> {
            Ok(())
        }
        fn Drop(
            &self,
            _: Ref<IDataObject>,
            _: MODIFIERKEYS_FLAGS,
            _: &POINTL,
            _: *mut DROPEFFECT,
        ) -> windows::core::Result<()> {
            Ok(())
        }
    }

    thread_local! {
        /// Registration of the test window, revoked by its `WM_DESTROY` when enabled.
        static TARGET: (DropTarget, Cell<bool>) = (DropTarget::default(), Cell::new(false));
    }

    unsafe extern "system" fn proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        if msg == WM_DESTROY {
            TARGET.with(|(target, enabled)| {
                if enabled.get() {
                    target.revoke(hwnd);
                }
            });
        }
        unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
    }

    /// Registers a target on a real window, closes it from outside with
    /// `WM_CLOSE` (default handling destroys it) and returns the `Rc` count left.
    fn strong_count_after_external_close(revoke_in_destroy: bool) -> usize {
        unsafe { OleInitialize(None) }.ok();
        TARGET.with(|(_, enabled)| enabled.set(revoke_in_destroy));
        let class = w!("KaminDropTargetTest");
        let instance = unsafe { GetModuleHandleW(None) }.expect("module handle");
        let wc = WNDCLASSW {
            lpfnWndProc: Some(proc),
            hInstance: instance.into(),
            lpszClassName: class,
            ..Default::default()
        };
        unsafe { RegisterClassW(&wc) };
        let hwnd = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE(0),
                class,
                w!(""),
                WS_OVERLAPPED,
                0,
                0,
                10,
                10,
                None,
                None,
                Some(instance.into()),
                None,
            )
        }
        .expect("test window");
        let owner = Rc::new(());
        let target: IDropTarget = Probe(owner.clone()).into();
        TARGET
            .with(|(registration, _)| registration.register(hwnd, &target))
            .expect("register");
        drop(target);
        unsafe { SendMessageW(hwnd, WM_CLOSE, None, None) };
        assert!(
            !unsafe { IsWindow(Some(hwnd)) }.as_bool(),
            "WM_CLOSE destroys the window"
        );
        // What `WindowsWindow::drop` does afterwards on the dead HWND.
        TARGET.with(|(registration, _)| registration.revoke(hwnd));
        Rc::strong_count(&owner)
    }

    #[test]
    fn window_closed_from_outside_releases_its_drop_target() {
        assert_eq!(strong_count_after_external_close(true), 1);
    }

    #[test]
    fn revoke_after_the_system_destroyed_the_window_keeps_the_target_alive() {
        // The defect this module removes: without the revoke in `WM_DESTROY`
        // OLE keeps the target, and with it the window, alive.
        assert_eq!(strong_count_after_external_close(false), 2);
    }
}
