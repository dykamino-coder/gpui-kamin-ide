//! Native path prompts; the Win32 file prompt supplies filters absent from GPUI's API.

use crate::app::Browser;
use gpui::{Context, PathPromptOptions, Window};
use std::path::PathBuf;

type Selection = Result<Option<PathBuf>, String>;

impl Browser {
    pub fn prompt(&mut self, folder: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.dialog_open {
            return;
        }
        self.dialog_open = true;
        #[cfg(windows)]
        if !folder {
            match file_prompt(window) {
                Ok(result) => cx
                    .spawn_in(window, async move |this, cx| {
                        let result = result
                            .await
                            .map_err(|error| format!("File dialog stopped: {error}"))
                            .and_then(|result| result);
                        let _ = this.update_in(cx, |this, window, cx| {
                            this.finish_prompt(result, window, cx);
                        });
                    })
                    .detach(),
                Err(error) => self.finish_prompt(Err(error), window, cx),
            }
            return;
        }
        let result = cx.prompt_for_paths(PathPromptOptions {
            files: !folder,
            directories: folder,
            multiple: false,
            prompt: Some(if folder { "Open folder" } else { "Open file" }.into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            let result = result
                .await
                .map_err(|error| error.to_string())
                .and_then(|result| result.map_err(|error| error.to_string()))
                .map(|paths| paths.and_then(|paths| paths.into_iter().next()));
            let _ = this.update_in(cx, |this, window, cx| {
                this.finish_prompt(result, window, cx);
            });
        })
        .detach();
    }

    fn finish_prompt(&mut self, result: Selection, window: &mut Window, cx: &mut Context<Self>) {
        self.dialog_open = false;
        match result {
            Ok(Some(path)) => self.open(path, window, cx),
            Ok(None) => {}
            Err(error) => self.error = Some(format!("Cannot open path dialog: {error}")),
        }
        cx.notify();
    }
}

#[cfg(windows)]
fn file_prompt(window: &Window) -> Result<futures::channel::oneshot::Receiver<Selection>, String> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx, CoUninitialize};

    let handle = HasWindowHandle::window_handle(window).map_err(|error| error.to_string())?;
    let RawWindowHandle::Win32(handle) = handle.as_raw() else {
        return Err("No native Windows handle".into());
    };
    let owner = handle.hwnd.get();
    let (sender, receiver) = futures::channel::oneshot::channel();
    // Native modal loops synchronously message the owner; keep GPUI pumping.
    std::thread::Builder::new()
        .name("viewer-file-dialog".into())
        .spawn(move || {
            let initialized = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok() };
            let result = initialized
                .map_err(|error| error.to_string())
                .and_then(|()| {
                    let result = file(owner);
                    unsafe { CoUninitialize() };
                    result
                });
            let _ = sender.send(result);
        })
        .map_err(|error| error.to_string())?;
    Ok(receiver)
}

#[cfg(windows)]
fn file(owner: isize) -> Selection {
    use std::{ffi::OsString, os::windows::ffi::OsStringExt};
    use windows::{
        Win32::{
            Foundation::HWND,
            UI::Controls::Dialogs::{
                CommDlgExtendedError, GetOpenFileNameW, OFN_EXPLORER, OFN_FILEMUSTEXIST,
                OFN_NOCHANGEDIR, OFN_PATHMUSTEXIST, OPENFILENAMEW,
            },
        },
        core::{PCWSTR, PWSTR},
    };

    let mut path = vec![0u16; 32768];
    let filter: Vec<u16> =
        "HTML pages (*.html;*.htm;*.xhtml;*.svg)\0*.html;*.htm;*.xhtml;*.svg\0\0"
            .encode_utf16()
            .collect();
    let mut options = OPENFILENAMEW {
        lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
        hwndOwner: HWND(owner as *mut _),
        lpstrFilter: PCWSTR(filter.as_ptr()),
        lpstrFile: PWSTR(path.as_mut_ptr()),
        nMaxFile: path.len() as u32,
        Flags: OFN_EXPLORER | OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST | OFN_NOCHANGEDIR,
        ..Default::default()
    };
    // The owner and both UTF-16 buffers outlive the synchronous modal call.
    if unsafe { GetOpenFileNameW(&mut options) }.as_bool() {
        let end = path.iter().position(|&ch| ch == 0).unwrap_or(path.len());
        Ok(Some(OsString::from_wide(&path[..end]).into()))
    } else {
        // The error belongs to the immediately preceding dialog on this thread.
        let error = unsafe { CommDlgExtendedError() };
        if error.0 == 0 {
            Ok(None)
        } else {
            Err(format!("Cannot open file dialog: {}", error.0))
        }
    }
}
