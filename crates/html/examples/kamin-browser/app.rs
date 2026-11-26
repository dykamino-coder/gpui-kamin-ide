//! Viewer state, window lifecycle, folder selection, and bounded file polling.

use crate::{paths, viewport};
use gpui::{
    Context, Entity, FocusHandle, Focusable, ScrollHandle, Subscription, Task, Window, px, size,
};
use kamin_html::{host::document_view::DocumentView, metrics::FontContext};
use std::{
    path::PathBuf,
    time::{Duration, SystemTime},
};

pub struct Browser {
    pub fonts: FontContext,
    pub page: Option<Entity<DocumentView>>,
    pub error: Option<String>,
    pub dialog_open: bool,
    pub current: Option<PathBuf>,
    pub folder: Option<PathBuf>,
    pub files: Vec<PathBuf>,
    pub all_files: Vec<PathBuf>,
    pub base_only: bool,
    pub selected: usize,
    pub list_focus: FocusHandle,
    pub list_scroll: ScrollHandle,
    pub selector_open: bool,
    pub selector_focus: FocusHandle,
    pub selector_scroll: ScrollHandle,
    pub preset_width: Option<u32>,
    pub viewport: (f32, f32),
    pub stamp: Option<(SystemTime, u64)>,
    #[cfg(feature = "viewer-timing")]
    pub timing: Option<crate::timing::Timing>,
    #[cfg(feature = "viewer-timing")]
    pub timing_plain: bool,
    _bounds: Subscription,
    _poll: Task<()>,
}

impl Browser {
    pub fn new(
        path: Option<PathBuf>,
        fonts: FontContext,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let bounds = cx.observe_window_bounds(window, |this, window, cx| this.resize(window, cx));
        let poll = cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(500))
                    .await;
                if this
                    .update_in(cx, |this, window, cx| {
                        if this
                            .current
                            .as_ref()
                            .is_some_and(|path| paths::stamp(path) != this.stamp)
                        {
                            this.reload(window, cx);
                        }
                    })
                    .is_err()
                {
                    break;
                }
            }
        });
        let mut this = Self {
            fonts,
            page: None,
            error: None,
            dialog_open: false,
            current: None,
            folder: None,
            files: Vec::new(),
            all_files: Vec::new(),
            base_only: false,
            selected: 0,
            list_focus: cx.focus_handle(),
            list_scroll: ScrollHandle::new(),
            selector_open: false,
            selector_focus: cx.focus_handle(),
            selector_scroll: ScrollHandle::new(),
            preset_width: None,
            viewport: (0., 0.),
            stamp: None,
            #[cfg(feature = "viewer-timing")]
            timing: None,
            #[cfg(feature = "viewer-timing")]
            timing_plain: false,
            _bounds: bounds,
            _poll: poll,
        };
        this.resize(window, cx);
        if let Some(path) = path {
            this.open(path, window, cx);
        }
        this
    }

    pub fn open(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        let path = match paths::absolute(&path) {
            Ok(path) => path,
            Err(error) => {
                self.error = Some(error.to_string());
                cx.notify();
                return;
            }
        };
        // Changing file/folder mode changes chrome width. Drop the outgoing page
        // before resizing, so it cannot restyle for a viewport it will never use.
        self.page = None;
        self.error = None;
        self.selector_open = false;
        self.preset_width = None;
        self.folder = path.is_dir().then(|| path.clone());
        self.files.clear();
        self.all_files.clear();
        self.base_only = false;
        self.selected = 0;
        if self.folder.is_some() {
            match paths::list(&path) {
                Ok(files) => {
                    self.base_only = files.iter().any(|path| crate::folder::is_base(path));
                    self.all_files = files;
                    self.files = crate::folder::filtered(&self.all_files, self.base_only);
                }
                Err(error) => self.error = Some(format!("Cannot list {}: {error}", path.display())),
            }
            self.current = self.files.first().cloned();
            eprintln!(
                "Opened folder: {} ({} pages)",
                path.display(),
                self.files.len()
            );
        } else {
            self.current = Some(path);
        }
        self.resize(window, cx);
        self.reload(window, cx);
        if self.folder.is_some() {
            self.list_focus.focus(window, cx);
        }
    }

    pub fn navigate(&mut self, delta: isize, window: &mut Window, cx: &mut Context<Self>) {
        if self.files.is_empty() {
            return;
        }
        self.selected = self
            .files
            .iter()
            .position(|path| Some(path) == self.current.as_ref())
            .unwrap_or(self.selected);
        self.selected = self
            .selected
            .saturating_add_signed(delta)
            .min(self.files.len() - 1);
        self.activate_selected(window, cx);
    }

    pub fn activate_selected(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.selector_open = false;
        if let Some(path) = self.files.get(self.selected).cloned() {
            self.current = Some(path);
            self.list_scroll.scroll_to_item(self.selected);
            self.reload(window, cx);
        }
    }

    pub fn resize(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let window_size = window.viewport_size();
        self.viewport = viewport::content(
            (window_size.width.into(), window_size.height.into()),
            self.folder.is_some(),
        );
        if let Some(width) = self.preset_width {
            if viewport::matches_preset(
                window_size.width.into(),
                width,
                self.folder.is_some(),
                window.scale_factor(),
            ) {
                self.viewport.0 = width as f32;
            }
        }
        #[cfg(feature = "viewer-timing")]
        if let Some(timing) = &self.timing {
            self.viewport = timing.viewport;
        }
        if let Some(page) = &self.page {
            let result = page.update(cx, |page, cx| {
                page.resize(size(px(self.viewport.0), px(self.viewport.1)), cx)
            });
            if let Err(error) = result {
                self.error = Some(format!("Cannot resize page: {error}"));
            }
        }
        cx.notify();
    }

    pub fn preset(&mut self, width: u32, window: &mut Window, cx: &mut Context<Self>) {
        self.preset_width = Some(width);
        if window.is_maximized() || window.is_fullscreen() {
            crate::window::maximize_or_restore(window);
        }
        let dims = viewport::preset_window(
            width,
            window.viewport_size().height.into(),
            self.folder.is_some(),
            window.scale_factor(),
        );
        window.resize(size(px(dims.0), px(dims.1)));
        self.resize(window, cx);
    }
}

impl Focusable for Browser {
    fn focus_handle(&self, _: &gpui::App) -> FocusHandle {
        self.list_focus.clone()
    }
}
