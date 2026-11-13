//! Enter корня: приоритет оверлеев и подтверждение inline rename.

use crate::actions::PressEnter;
use crate::host::events::EdEvent;
use crate::host_link::ShellEvent;
use crate::state::model::RootView;
use gpui::{Context, Window};

impl RootView {
    pub(super) fn press_enter(&mut self, _: &PressEnter, _: &mut Window, cx: &mut Context<Self>) {
        // Confirm-модалка без инпута: у оригинала кнопка Confirm
        // автофокусится, и Enter её нажимает (`ConfirmModal.tsx:46-56`).
        // Prompt-вариант ловит Enter сам — в своём инпуте (overlay).
        if self.modal.as_ref().is_some_and(|m| m.prompt.is_none()) {
            let _ = self.tx.try_send(ShellEvent::ConfirmModal);
            cx.notify();
            return;
        }
        if let Some(qp) = self.quick_pick.take() {
            // Ответ QuickPick — ИНДЕКСЫ выбранных пунктов.
            // multi → отмеченные, single → первый ОТФИЛЬТРОВАННЫЙ
            // не-сепаратор (`QuickPickModal.tsx:83-87`); раньше здесь
            // безусловно уезжал `[0]` — мимо фильтра, мимо чекбоксов
            // и с риском вернуть сепаратор (ревью ц.25)
            let query = self
                .qp_input
                .as_ref()
                .map(|inp| inp.read(cx).value().to_string())
                .unwrap_or_default();
            let pick = qp.enter_pick(&query);
            self.qp_input = None;
            let _ = self
                .tx
                .try_send(ShellEvent::QuickPickResolve(qp.req_id, pick));
            cx.notify();
            return;
        }
        if self.sov.ws_open {
            if let Some(hit) = self.sov.ws_results.get(self.ws_active_idx()) {
                let ev = match hit.line {
                    Some(l) => ShellEvent::Ed(EdEvent::OpenFileAt(hit.uri.clone(), l)),
                    None => ShellEvent::Ed(EdEvent::OpenFile(hit.uri.clone())),
                };
                let _ = self.tx.try_send(ev);
                let _ = self
                    .tx
                    .try_send(ShellEvent::Ed(EdEvent::SetFileMode("files")));
            }
        } else if self.sov.fif_open {
            if let Some(hit) = self.sov.fif_results.get(self.fif_active_idx()) {
                let _ = self.tx.try_send(ShellEvent::Ed(EdEvent::OpenFileAt(
                    hit.abs.clone(),
                    hit.line,
                )));
                let _ = self
                    .tx
                    .try_send(ShellEvent::Ed(EdEvent::SetFileMode("files")));
            }
        } else if self.sov.quickopen_open {
            if let Some(hit) = self.sov.quickopen_results.get(self.qo_active_idx()) {
                let _ = self
                    .tx
                    .try_send(ShellEvent::Ed(EdEvent::OpenFile(hit.abs.clone())));
                let _ = self
                    .tx
                    .try_send(ShellEvent::Ed(EdEvent::SetFileMode("files")));
                let _ = self.tx.try_send(ShellEvent::CloseQuickOpen);
            }
        } else if self.palette_open {
            // Первый пункт отфильтрованного списка — ровно то, что
            // выполняет Enter у оригинала (`CommandPalette.tsx:47-54`)
            let q = self
                .palette_input
                .as_ref()
                .map(|inp| inp.read(cx).value().to_string())
                .unwrap_or_default();
            if let Some(first) =
                crate::ui::command_palette::filter_gated(&self.commands, &q, &self.palette_gate())
                    .first()
            {
                let _ = self.tx.try_send(ShellEvent::RunCommand(first.id.clone()));
            }
        } else if let Some(id) = &self.renaming_session {
            // Root keymap потребляет Enter раньше on_key_down строки.
            let _ = self.tx.try_send(ShellEvent::CommitRename);
            // Хэндл инпута умирает вместе с ним: без возврата фокус остаётся
            // на мёртвом id, и Tab/F2 не доходят до Root до первого клика.
            self.modal_focus_return = Some(format!("srow:{id}"));
        }
        cx.notify();
    }
}
