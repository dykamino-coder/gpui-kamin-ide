//! Тело активного тула слота: сессии, дерево файлов, терминал, браузер,
//! проблемы, расширения, contributed-вью и заглушка для неизвестного тула.
//!
//! Вынесено из `root.rs` без изменения поведения.

use gpui::prelude::*;
use gpui::{AnyElement, Context, Window, div, px};
use kamin_theme::Palette;

use crate::root::RootView;
use crate::ui::sessions_list::sessions_sidebar;

impl RootView {
    /// `onFocus={(e) => … e.currentTarget.select()}` адресной строки
    /// (`BrowserPane.tsx:96`): при получении фокуса адрес выделяется целиком,
    /// один раз на фокус.
    pub(crate) fn browser_select_all_on_focus(
        &mut self,
        focused: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if focused && !self.browser_addr_selected {
            self.browser_addr_selected = true;
            window.dispatch_action(Box::new(gpui_component::input::SelectAll), cx);
        } else if !focused && self.browser_addr_selected {
            self.browser_addr_selected = false;
        }
    }

    /// `.logoWrap::before` резолвит `var(--accent-primary)` живьём, поэтому
    /// при смене темы свечение welcome надо перепечь — иначе остаётся акцент
    /// старой палитры (ревью ц.23).
    pub(crate) fn rebake_welcome_glow(&mut self) {
        self.welcome_glow = crate::ui::radial_bg::bake_glow_edge(
            220,
            self.theme.palette().accent_primary,
            0.26,
            0.68,
        );
        // Радиальный фон тоже зависит от акцентов палитры — без перепечки
        // после смены темы висели эллипсы прежней (пеклись только в init).
        self.radial_bg = crate::ui::radial_bg::RadialBg::bake(self.theme.palette());
    }

    pub(crate) fn tool_body(
        &mut self,
        slot: crate::activity::PanelSlot,
        p: &'static Palette,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let active = self.activity.state(slot).active.clone()?;
        let el = match active.as_str() {
            id if crate::activity::dyn_tool(id).is_some() => {
                // Contributed контейнер: ВСЕ его вью стопкой, у каждого свой
                // хедер и тело — вебвью либо дерево провайдера
                // (`ContributedContainerBody.tsx:30-36`). Пустой контейнер —
                // плейсхолдер «No views».
                let tool = crate::activity::dyn_tool(id).unwrap();
                if tool.views.is_empty() {
                    return Some(crate::ui::panel_placeholder::activity_placeholder(
                        "circle-large",
                        "No views",
                        p,
                    ));
                }
                let mut stack = div().flex().flex_col().size_full().min_h(px(0.));
                for v in tool.views.clone() {
                    // `.view { flex: 1; min-height: 0 }` — вью делят высоту
                    stack = stack.child(div().flex().flex_col().flex_1().min_h(px(0.)).child(
                        if v.webview {
                            self.contributed_webview_section(&v, p)
                        } else {
                            self.contributed_tree_section(&v, p)
                        },
                    ));
                }
                stack.into_any_element()
            }
            "tree" => self.file_tree_body(slot.as_str(), cx),
            "terminal" => return self.terminal_tool_body(p, cx),
            "projects" => sessions_sidebar(
                self.sessions.as_ref(),
                &self.collapsed_projects,
                &self.inactive_open,
                self.renaming_session
                    .as_deref()
                    .zip(self.rename_input.as_ref()),
                self.hover_pill.as_deref(),
                &self.tx,
                p,
            ),
            "extensions" => crate::ui::extensions_panel::extensions_panel(
                self.cz.extensions.as_ref(),
                &self.cz.ext_icons,
                &self.cz.ext_status,
                &self.tx,
                p,
            ),
            "problems" => crate::ui::problems::problems_panel(
                &self.diags,
                self.problems_filter,
                &self.problems_collapsed,
                self.problems_file_cap,
                &self.tx,
                p,
            ),
            other => {
                // Тул без нативного тела — ActivityPlaceholder оригинала
                // (ToolIcon 36 + «Nothing to show here yet.», без пикера)
                let (label, icon) = crate::activity::lookup_any(other)
                    .map(|(l, i)| (crate::activity::intern(&l), crate::activity::intern(&i)))
                    .unwrap_or((other, "wrench"));
                crate::ui::panel_placeholder::activity_placeholder(icon, label, p)
            }
        };
        Some(el)
    }
}
