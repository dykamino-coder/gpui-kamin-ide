//! Сессии: пины, переименование, активация, создание, порядок чипов,
//! ховер пилюли проекта.
//!
//! Кого сюда зовут — решает `state/events/dispatch.rs`; связку
//! «вариант события → модуль» проверяет `scripts/check_event_routing.py`.

use crate::host_link::{self, ShellEvent};
use crate::state::hover_pill::{HoverPillUpdate, update_hover_pill_state};
use gpui::Context;
use kamin_model::SessionsSnapshot;

use crate::root::{ChipDrag, RootView};

impl RootView {
    /// Сессии: пины, переименование, активация, создание, порядок чипов.
    pub(crate) fn apply_sessions(&mut self, event: ShellEvent, cx: &mut Context<Self>) {
        let _ = cx;
        match event {
            ShellEvent::LocalSessionPinned(id, pinned) => {
                if let Some(snap) = self.sessions.as_mut()
                    && let Some(s) = snap.sessions.iter_mut().find(|s| s.id == id)
                {
                    s.pinned = pinned;
                }
            }
            ShellEvent::LocalSessionClosed(id) => {
                if let Some(snap) = self.sessions.as_mut()
                    && let Some(s) = snap.sessions.iter_mut().find(|s| s.id == id)
                {
                    s.open = false;
                }
            }
            ShellEvent::Sessions(value) => {
                match serde_json::from_value::<SessionsSnapshot>(value) {
                    Ok(snap) => {
                        // Сессия поднялась → гасим switching-спиннер чипа
                        if let Some(id) = self.switching_to.as_deref()
                            && snap.sessions.iter().any(|s| s.id == id && s.open)
                        {
                            self.switching_to = None;
                        }
                        // Сессия подтверждена хостом — дорезолвить только вью,
                        // которые ЕЩЁ НЕ подняты (первая активация из пустого
                        // состояния). Ре-резолв уже живого вью заставляет
                        // расширение прислать HTML заново → iframe перегружается
                        // с нуля — ЭТО и была «перезагрузка чата на каждом
                        // переключении»: тёплый свитч не должен трогать вебвью.
                        if self.pending_view_resolve && snap.active_session_id.is_some() {
                            self.pending_view_resolve = false;
                            for view in crate::host_link::KNOWN_WEBVIEWS {
                                let resolved = crate::host_link::resolved_views()
                                    .lock()
                                    .unwrap()
                                    .contains(*view);
                                if !resolved {
                                    crate::host_link::resolve_webview((*view).to_string());
                                }
                            }
                        }
                        // Протокол шторки — как chat-switch-cover.ts оригинала:
                        // расширение публикует bridgeShowing на активной сессии.
                        // false = свитч принят (гейт открыт); true ПОСЛЕ false =
                        // чат реально показывает новую сессию → шторка гаснет.
                        // Свежий true отличаем от залежавшегося (сессия могла
                        // остаться с true с прошлого показа) флагом expect_fresh.
                        let showing = snap
                            .active_session_id
                            .as_deref()
                            .and_then(|id| snap.sessions.iter().find(|s| s.id == id))
                            .and_then(|s| s.metadata.as_ref())
                            .and_then(|m| m.get("bridgeShowing"))
                            .and_then(serde_json::Value::as_bool);
                        match showing {
                            Some(false) => self.cover_expect_fresh = false,
                            Some(true) if !self.cover_expect_fresh => {
                                if let Some((_, cleared @ None)) = self.chat_cover.as_mut() {
                                    *cleared = Some(std::time::Instant::now());
                                }
                            }
                            _ => {}
                        }
                        // БУТ: starred preset сильнее снапшота активной сессии,
                        // затем идёт уже загруженный layout.json/factory default.
                        // Один порядок исключает поздний откат после первого
                        // кадра, когда session snapshot приходит от хоста.
                        let mut sync_boot_layout = false;
                        if !self.layout_booted {
                            self.layout_booted = true;
                            let session_layout = snap
                                .active_session_id
                                .as_ref()
                                .and_then(|id| snap.sessions.iter().find(|s| s.id == *id))
                                .and_then(|s| s.layout.clone());
                            self.layout = crate::layout_store::select_startup_layout(
                                crate::layout_store::load_default_preset(),
                                session_layout,
                                Some(self.layout.clone()),
                            );
                            self.sidebar_visible = self.layout.sidebar_visible;
                            sync_boot_layout = snap.active_session_id.is_some();
                            cx.notify();
                        }
                        // Хост шлёт снапшот на каждый чих; одинаковый —
                        // не повод пересобирать сайдбар.
                        if self.sessions.as_ref() != Some(&snap) {
                            self.sessions = Some(snap)
                        }
                        if sync_boot_layout {
                            self.persist_active_session_layout();
                        }
                    }
                    Err(e) => eprintln!("sessions snapshot parse failed: {e}"),
                }
            }
            ShellEvent::ToggleProjectCollapse(pid) => {
                if !self.collapsed_projects.remove(&pid) {
                    self.collapsed_projects.insert(pid);
                }
            }
            ShellEvent::ActivateSession(id) => {
                self.cz.customize_open = false;
                // Индикатор переключения: закрытая сессия поднимается хостом
                // секунды — спиннер на чипе, пока снапшот не скажет open
                let is_closed = self
                    .sessions
                    .as_ref()
                    .and_then(|s| s.sessions.iter().find(|x| x.id == id))
                    .is_some_and(|s| !s.open);
                if is_closed {
                    self.switching_to = Some(id.clone());
                }
                // Шторка — на ЛЮБОЙ переход между сессиями, а не только на
                // закрытую цель (`chat-switch-cover.ts:57-64`)
                let prev = self
                    .sessions
                    .as_ref()
                    .and_then(|s| s.active_session_id.clone());
                if prev.as_ref().is_some_and(|pv| *pv != id) {
                    self.chat_cover = Some((std::time::Instant::now(), None));
                }
                // Пер-сессионный layout (как session-editor-sync.ts у Tauri):
                // текущую раскладку сохраняем за ПРЕЖНЕЙ сессией, у НОВОЙ
                // применяем её сохранённую (если есть). На буте арм не
                // срабатывает — активная сессия живёт глобальным layout.json
                // («как оставил»), поэтому её снапшот не может его откатить.
                if let Some(prev_id) = prev.filter(|p| *p != id) {
                    if let Ok(mut snap_v) = serde_json::to_value(&self.layout) {
                        snap_v["sidebarVisible"] = serde_json::Value::Bool(self.sidebar_visible);
                        std::thread::spawn(move || {
                            if let Some(c) = host_link::client() {
                                let _ = c.request(
                                    "kamin:sessions:setState",
                                    vec![
                                        serde_json::json!(prev_id),
                                        serde_json::json!({ "layout": snap_v }),
                                    ],
                                );
                            }
                        });
                    }
                    if let Some(snap) = self
                        .sessions
                        .as_ref()
                        .and_then(|s| s.sessions.iter().find(|x| x.id == id))
                        .and_then(|s| s.layout.clone())
                    {
                        self.sidebar_visible = snap.sidebar_visible;
                        self.layout = snap;
                    } else {
                        // НОВАЯ сессия (снапшота нет): default-пресет, иначе
                        // заводской — а не лейаут прежней сессии (запрос юзера).
                        let fresh = crate::layout_store::load_presets()
                            .into_iter()
                            .find(|p| p.default)
                            .and_then(|p| serde_json::from_value(p.snapshot).ok())
                            .unwrap_or_default();
                        self.layout = fresh;
                        self.sidebar_visible = self.layout.sidebar_visible;
                    }
                    // Зеркалим ПОЛНЫЙ снапшот в layout.json: частичные патчи
                    // (ширины end_drag, одиночные флаги) поверх лейаута другой
                    // сессии собирали на диске химеру, и перезагрузка
                    // восстанавливала «случайный момент» (дыры №1/№2 аудита).
                    if let Ok(mut v) = serde_json::to_value(&self.layout) {
                        v["sidebarVisible"] = serde_json::Value::Bool(self.sidebar_visible);
                        crate::layout_store::save_patch(v);
                    }
                }
                // Оптимистично: подсветка/переключение мгновенно, снапшот хоста
                // затем подтвердит/скорректирует (round-trip не блокирует UI).
                if let Some(snap) = self.sessions.as_mut() {
                    snap.active_session_id = Some(id.clone());
                }
                // Шторка ждёт СВЕЖЕГО подтверждения от чата (bridgeShowing
                // false→true через снапшоты) — см. арм Sessions выше.
                eprintln!(
                    "[switch] activate {} t+{:.2}s",
                    &id[..8.min(id.len())],
                    crate::host_link::t0().elapsed().as_secs_f32()
                );
                self.cover_expect_fresh = true;
                self.pending_view_resolve = true;
                // Пульс кадров на жизнь шторки: грейс/фейд/таймаут считаются в
                // рендере, а без событий кадров нет (точные заказы) — шторка
                // зависала навсегда, если подтверждение не пришло.
                if let Some(tx) = crate::host_link::event_tx() {
                    std::thread::spawn(move || {
                        for _ in 0..30 {
                            std::thread::sleep(std::time::Duration::from_millis(120));
                            if tx.try_send(ShellEvent::CoverTick).is_err() {
                                break;
                            }
                        }
                    });
                }
                crate::ui::sessions_list::activate_session(id);
            }
            // Пульс шторки: состояние не меняет, кадр заказывает сам факт
            // события (needs_frame=true) — рендер пересчитает грейс/фейд/таймаут.
            ShellEvent::CoverTick => {}
            // Чат нарисовал свой активный таб — шторку можно снимать сразу.
            // Отчёт свежий по построению: bound ставится только на ТЕКУЩИЙ
            // активный таб вебвью, так что залежаться он не может дольше
            // времени доставки (десятки мс); таймаут остаётся страховкой.
            ShellEvent::ChatBound => {
                self.cover_expect_fresh = false;
                if let Some((_, cleared @ None)) = self.chat_cover.as_mut() {
                    *cleared = Some(std::time::Instant::now());
                }
            }
            ShellEvent::OpenSessionMenu(data, x, y) => {
                self.close_popovers_except("session");
                self.session_menu = Some(crate::ui::context_menu::SessionMenu { data, x, y });
            }
            ShellEvent::CloseSessionMenu => self.session_menu = None,
            ShellEvent::OpenRenamePresetPrompt(old) => {
                self.modal = Some(crate::ui::modal::Modal {
                    title: "Rename layout".into(),
                    body: "".into(),
                    // `showPrompt({title, placeholder})` — кнопка всегда «OK»
                    confirm_label: "OK".into(),
                    danger: false,
                    prompt: Some(old.clone()),
                    placeholder: Some("Layout name".into()),
                    validate: None,
                    cancel_label: None,
                    action: crate::ui::modal::ModalAction::RenamePreset { old },
                });
            }
            ShellEvent::ToggleNewSessionMenu(x, y) => {
                self.close_popovers_except("newsession");
                self.new_session_menu = if self.new_session_menu.is_some() {
                    None
                } else {
                    Some((x, y))
                };
            }
            ShellEvent::NewEmptySession => {
                self.new_session_menu = None;
                std::thread::spawn(|| {
                    if let Some(c) = host_link::client() {
                        let _ = c.request("kamin:sessions:newNoFolderSession", vec![]);
                    }
                });
            }
            ShellEvent::NewSessionInFolderPrompt => {
                self.new_session_menu = None;
                let rx = cx.prompt_for_paths(gpui::PathPromptOptions {
                    files: false,
                    directories: true,
                    multiple: false,
                    prompt: None,
                });
                cx.spawn(async move |_, _| {
                    if let Ok(Ok(Some(paths))) = rx.await
                        && let Some(path) = paths.first()
                    {
                        let path = path.to_string_lossy().to_string();
                        std::thread::spawn(move || {
                            if let Some(c) = host_link::client() {
                                let _ = c.request(
                                    "kamin:sessions:newSessionInFolder",
                                    vec![serde_json::json!(path)],
                                );
                            }
                        });
                    }
                })
                .detach();
            }
            ShellEvent::BeginRename(id) => self.begin_rename(id),
            ShellEvent::CommitRename => {
                if let (Some(id), Some(input)) = (&self.renaming_session, &self.rename_input) {
                    let name = input.read(cx).value().trim().to_string();
                    if !name.is_empty() {
                        let id = id.clone();
                        std::thread::spawn(move || {
                            if let Some(client) = host_link::client() {
                                let _ = client.request(
                                    "kamin:sessions:rename",
                                    vec![serde_json::json!(id), serde_json::json!(name)],
                                );
                            }
                        });
                    }
                }
                self.renaming_session = None;
                self.rename_input = None;
                self.rename_sub = None;
            }
            ShellEvent::CancelRename => {
                self.renaming_session = None;
                self.rename_input = None;
                self.rename_sub = None;
            }
            ShellEvent::ChipPress(id, x, y) => {
                self.chip_drag = Some(ChipDrag {
                    src: id,
                    start: (x, y),
                    started: false,
                    over: None,
                });
            }
            ShellEvent::ChipDragOver(id) => {
                // occlude() чипов глотает move и для root-порога — курсор над
                // ЧУЖИМ чипом при зажатой ЛКМ сам по себе доказывает драг
                if let Some(cd) = self.chip_drag.as_mut()
                    && id != cd.src
                {
                    cd.started = true;
                    cd.over = Some(id);
                }
            }
            ShellEvent::ChipRelease => self.commit_chip_drag(),
            ShellEvent::HoverPill {
                id,
                source,
                hovered,
            } => {
                // Драг ручки: пилюля не всплывает (мерцала под мышью).
                // Строка в inline rename: enter игнорируется (BR-29).
                if hovered && crate::state::drag_flag::active()
                    || !crate::state::rename_transition::hover_allowed(
                        self.renaming_session.as_deref(),
                        &id,
                        hovered,
                    )
                {
                    return;
                }

                if let HoverPillUpdate::Inactive { id, generation } = update_hover_pill_state(
                    &mut self.hover_pill,
                    &mut self.hover_pill_anchor,
                    &mut self.hover_pill_panel,
                    &mut self.hover_pill_gen,
                    id,
                    source,
                    hovered,
                ) {
                    // Grace остаётся страховкой для редких пропущенных
                    // mouse-move, но зазор теперь покрыт реальным hitbox.
                    cx.spawn(async move |this, cx| {
                        smol::Timer::after(std::time::Duration::from_millis(160)).await;
                        let _ = this.update(cx, |this, cx| {
                            let still_hovered = this.hover_pill_anchor.as_deref()
                                == Some(id.as_str())
                                || this.hover_pill_panel.as_deref() == Some(id.as_str());
                            if this.hover_pill_gen == generation
                                && this.hover_pill.as_deref() == Some(id.as_str())
                                && !still_hovered
                            {
                                this.hover_pill = None;
                                cx.notify();
                            }
                        });
                    })
                    .detach();
                }
            }
            ShellEvent::DismissHoverPill => {
                self.dismiss_hover_pill();
            }
            // Сюда диспетчер чужого не пришлёт
            _ => {}
        }
    }
}
