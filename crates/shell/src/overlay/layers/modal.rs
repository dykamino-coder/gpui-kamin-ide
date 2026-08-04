//! Слой модалки: ленивое создание prompt-инпута, фокус и сама коробка.
//!
//! Блок перенесён из `OverlayWindow::render` как есть (`plan/100-refactor-250.md`).

use crate::host_link::ShellEvent;
use crate::overlay::region::input_area;
use crate::state::model::RootView;
use crate::ui::modal::render_modal;
use gpui::prelude::*;
use gpui::{Context, Focusable as _, div};

use gpui::Div;

impl RootView {
    // Компонент дизайн-системы: аргументы — его пропсы.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn add_modal(
        &mut self,
        mut layer: Div,
        modal: Option<crate::ui::modal::Modal>,
        p: &'static kamin_theme::Palette,
        tx: &smol::channel::Sender<ShellEvent>,
        modal_age_ms: u128,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) -> Div {
        if let Some(modal) = modal {
            // Prompt-модалка: лениво создать инпут в ЭТОМ окне + фокус.
            if let Some(seed) = &modal.prompt {
                if self.modal_input.is_none() {
                    let seed = seed.clone();
                    let ph = modal.placeholder.clone();
                    let input = cx.new(|cx| {
                        let mut st = gpui_component::input::InputState::new(window, cx);
                        if let Some(ph) = ph {
                            st = st.placeholder(ph);
                        }
                        st.set_value(seed, window, cx);
                        st
                    });
                    // Enter в поле = Confirm: у оригинала кнопка Confirm
                    // автофокусится и Enter её нажимает (`ConfirmModal.tsx:46-56`)
                    let tx_enter = tx.clone();
                    let sub = cx.subscribe(&input, move |_, inp, ev, cx| {
                        if matches!(ev, gpui_component::input::InputEvent::PressEnter { .. }) {
                            let value = inp.read(cx).value().to_string();
                            let _ = tx_enter.try_send(ShellEvent::ConfirmModalInput(value));
                        }
                    });
                    self.modal_input_sub = Some(sub);
                    window.focus(&input.read(cx).focus_handle(cx));
                    // `inputRef.current?.select()` при открытии
                    // (`PromptModal.tsx:42-45`): предложенное имя выделено
                    // целиком, ввод сразу заменяет его (ревью ц.20/22)
                    window.dispatch_action(Box::new(gpui_component::input::SelectAll), cx);
                    self.modal_input = Some(input);
                }
            } else if self.modal_input.is_some() {
                self.modal_input = None;
                self.modal_input_sub = None;
            }
            // Модалка — модальная: hit-регион на всё окно (скрим свой).
            let tx_cancel = tx.clone();
            let tx_confirm = tx.clone();
            let input_for_confirm = self.modal_input.clone();
            layer = layer.child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full()
                    .child(input_area()),
            );
            // `validate` гоняется на каждый ввод (`PromptModal.tsx:64`)
            let modal_error = modal.validate.and_then(|f| {
                let value = self
                    .modal_input
                    .as_ref()
                    .map(|i| i.read(cx).value().to_string())
                    .unwrap_or_default();
                f(&value).map(gpui::SharedString::from)
            });
            layer = layer.child(render_modal(
                &modal,
                self.modal_input.as_ref(),
                p,
                modal_error,
                // `.input:focus` — читаем состояние фокуса самого инпута
                self.modal_input
                    .as_ref()
                    .is_some_and(|inp| inp.read(cx).focus_handle(cx).is_focused(window)),
                modal_age_ms,
                window.text_style().font(),
                move |_, _| {
                    let _ = tx_cancel.try_send(ShellEvent::CloseModal);
                },
                move |_, cx| {
                    // Prompt → значение инпута; confirm → как раньше
                    match &input_for_confirm {
                        Some(input) => {
                            let value = input.read(cx).value().to_string();
                            let _ = tx_confirm.try_send(ShellEvent::ConfirmModalInput(value));
                        }
                        None => {
                            let _ = tx_confirm.try_send(ShellEvent::ConfirmModal);
                        }
                    }
                },
            ));
        } else if self.modal_input.is_some() {
            self.modal_input = None;
        }
        layer
    }
}
