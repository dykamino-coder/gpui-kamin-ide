//! Мышь окна: драг сплиттеров, плиток и чипов.
//!
//! Перенесено без изменения поведения (`plan/100-refactor-250.md`).

use crate::host_link::ShellEvent;
use crate::state::drag::move_item;
use crate::state::model::RootView;
use gpui::{Context, MouseMoveEvent, MouseUpEvent, Window};
use kamin_metrics as m;

impl RootView {
    /// Мышь окна: драг сплиттеров, плиток и чипов.
    // Драг сплиттеров: движение/отпускание ловим на корне
    pub(crate) fn bind_mouse<E>(&self, root: E, _window: &mut Window, cx: &mut Context<Self>) -> E
    where
        E: gpui::InteractiveElement,
    {
        root.on_mouse_move(cx.listener(move |this, e: &MouseMoveEvent, window, cx| {
            this.last_mouse = (f32::from(e.position.x), f32::from(e.position.y));
            if this.drag.is_some() {
                let vw = f32::from(window.viewport_size().width);
                let vh = f32::from(window.viewport_size().height);
                let bh = vh - m::TITLEBAR_HEIGHT - m::STATUS_BAR_HEIGHT;
                this.drag_move(f32::from(e.position.x), f32::from(e.position.y), vw, bh);
                cx.notify();
            }
            if let Some(td) = this.ed.tab_drag.as_mut() {
                let (x, y) = (f32::from(e.position.x), f32::from(e.position.y));
                if !td.started && ((x - td.start.0).abs() >= 4.0 || (y - td.start.1).abs() >= 4.0) {
                    td.started = true;
                    cx.notify();
                }
            }
            if let Some(cd) = this.chip_drag.as_mut() {
                let (x, y) = (f32::from(e.position.x), f32::from(e.position.y));
                if !cd.started && ((x - cd.start.0).abs() >= 4.0 || (y - cd.start.1).abs() >= 4.0) {
                    cd.started = true;
                    cx.notify();
                }
            }
            if let Some(td) = this.tool_drag.as_mut() {
                let (x, y) = (f32::from(e.position.x), f32::from(e.position.y));
                td.pos = (x, y);
                if !td.started && ((x - td.start.0).abs() >= 4.0 || (y - td.start.1).abs() >= 4.0) {
                    td.started = true; // порог 4px (activity-dnd)
                }
                if td.started {
                    // Дроп-зона = ВСЯ карта панели (и её рейл/бар), а не
                    // только полоска табов (`activity-dnd.ts:22-31`)
                    use crate::activity::PanelSlot as S;
                    const ZONES: [(&str, crate::activity::PanelSlot); 9] = [
                        ("sidebar", S::Sidebar),
                        ("activity-bar", S::Sidebar),
                        ("main", S::Main),
                        ("main-bottom", S::MainBottom),
                        ("central-bottom", S::CentralBottom),
                        ("right-top", S::RightTop),
                        ("rail-right-top", S::RightTop),
                        ("right-bottom", S::RightBottom),
                        ("rail-right-bottom", S::RightBottom),
                    ];
                    let zone = ZONES
                        .iter()
                        .find(|(key, _)| crate::probe::registry::hit(key, x, y))
                        .map(|(_, s)| *s);
                    // Липкая цель: над зазором/чужой хромотой зона
                    // остаётся прежней, иначе дроп «мимо на пиксель»
                    // терял цель (`onMove`, «Sticky target»)
                    if let Some(zone) = zone
                        && Some(zone) != td.over
                    {
                        td.over = Some(zone);
                        td.over_index = None; // смена зоны → индекс невалиден
                    }
                    cx.notify();
                }
            }
        }))
        .on_mouse_up(
            gpui::MouseButton::Left,
            cx.listener(move |this, _e: &MouseUpEvent, window, cx| {
                this.end_drag();
                // Доводчик после перетаскивания: через 150мс кадр, на
                // котором размер «замер», — страница подгонит свой размер.
                {
                    cx.spawn_in(window, async move |view, cx| {
                        cx.background_executor()
                            .timer(std::time::Duration::from_millis(150))
                            .await;
                        let _ = view.update_in(cx, |_, window, _| window.refresh());
                    })
                    .detach();
                }
                this.commit_chip_drag();
                if let Some(td) = this.ed.tab_drag.take() {
                    if !td.started {
                        // Клик без драга = выбор таба
                        if td.src < this.ed.editor_tabs.len() {
                            this.ed.editor_active = td.src;
                        }
                    } else if let Some(over) = td.over
                            && td.src < this.ed.editor_tabs.len()
                            // `over == from || over == from + 1` — перестановка
                            // никуда не двигает (`FileViewerTabs.tsx:117`)
                            && over != td.src
                            && over != td.src + 1
                            // `over == len` — вставка ПОСЛЕ последнего
                            && over <= this.ed.editor_tabs.len()
                    {
                        // `next.splice(over > from ? over − 1 : over, …)`
                        // (`FileViewerTabs.tsx:98-100`): после удаления
                        // элемента индексы правее сдвигаются на 1
                        let dst = if over > td.src { over - 1 } else { over };
                        // Активный таб следует за содержимым
                        let active_path = this.ed.editor_tabs[this.ed.editor_active].path.clone();
                        move_item(&mut this.ed.editor_tabs, td.src, dst);
                        if let Some(i) = this
                            .ed
                            .editor_tabs
                            .iter()
                            .position(|t| t.path == active_path)
                        {
                            this.ed.editor_active = i;
                        }
                        // Инвариант pinned-first после ручного reorder
                        this.sort_tabs_pinned_first();
                    }
                }
                if let Some(td) = this.tool_drag.take() {
                    if td.started {
                        if let Some(dst) = td.over {
                            // Индекс вставки. Компенсацию «src уехал влево»
                            // делает САМА модель (`move_activity`
                            // вычитает 1 при `dst_index > from`) — второй
                            // раз здесь её делать нельзя: reorder внутри
                            // слота промахивался на одну позицию
                            // (ревью ц.14).
                            let idx = td.over_index.unwrap_or(usize::MAX);
                            // Дроп «на своё место» (перед собой или сразу
                            // после) — не перемещение: подсветку мы уже
                            // гасили, а движение всё равно выполнялось
                            // (`activity-dnd.ts:68-72`, `:97`).
                            let self_drop = dst == td.src
                                && idx != usize::MAX
                                && this
                                    .tab_index_of(td.src, &td.id)
                                    .is_some_and(|from| idx == from || idx == from + 1);
                            if !self_drop {
                                this.activity.move_activity(td.src, &td.id, dst, idx);
                            }
                        }
                    } else {
                        // Клик без драга = активация плитки
                        // (`activity-dnd.ts:88-100` → `activateActivity`).
                        // ВАЖНО: у сайдбара тело читает `sidebar_activity`
                        // и выход из Customize делает `ActivityClicked` —
                        // одного `set_active` мало (плитка переставала
                        // переключать сайдбар, ревью ц.13).
                        if td.src == crate::activity::PanelSlot::Sidebar {
                            let id = crate::activity::static_id(&td.id);
                            let _ = this.tx.try_send(ShellEvent::ActivityClicked(id));
                        } else {
                            this.activity.set_active(td.src, &td.id);
                        }
                    }
                    this.save_activity();
                }
                cx.notify();
            }),
        )
        // Скрим контекст-меню: overlay-окно пропускает клики вне меню
        // сюда (per-pixel hit-test) — клик-мимо закрывает меню.
        .on_mouse_down(
            gpui::MouseButton::Left,
            cx.listener(|this, _: &gpui::MouseDownEvent, _, cx| {
                if let Some(qp) = this.quick_pick.take_if(|q| !q.ignore_focus_out) {
                    this.qp_input = None;
                    let _ = this
                        .tx
                        .try_send(ShellEvent::QuickPickResolve(qp.req_id, None));
                    cx.notify();
                }
                if this.session_menu.is_some()
                    || this.file_menu.is_some()
                    || this.tool_picker.is_some()
                    || this.ed.editor_tab_menu.is_some()
                    || this.layout_popover
                    || this.appearance_popover
                    || this.tabs_overflow_open.is_some()
                    || this.ed.file_tabs_overflow_open
                    || this.new_session_menu.is_some()
                    || this.tool_tab_menu.is_some()
                {
                    this.session_menu = None;
                    this.file_menu = None;
                    this.tool_picker = None;
                    this.ed.editor_tab_menu = None;
                    this.layout_popover = false;
                    this.appearance_popover = false;
                    this.tabs_overflow_open = None;
                    this.ed.file_tabs_overflow_open = false;
                    this.new_session_menu = None;
                    this.tool_tab_menu = None;
                    cx.notify();
                }
            }),
        )
    }
}
