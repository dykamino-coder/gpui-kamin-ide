//! Драг сплиттеров раскладки: пределы роста панели, старт/движение/конец
//! перетаскивания, показ грипа и «раскрыть файловую панель».
//!
//! Вынесено из `root.rs` без изменения поведения.

use kamin_metrics as m;

use crate::root::{DragKind, DragState, PanelSide, RootView};

impl RootView {
    pub(crate) fn max_panel_w(&self, viewport_w: f32, side: PanelSide) -> f32 {
        let sidebar_w = if self.sidebar_visible {
            (self.layout.sidebar_width_px as f32).round()
        } else {
            0.0
        };
        let file_w = if self.layout.file_panel_visible {
            self.file_w_live.unwrap_or_else(|| {
                m::layout_math::width_from_ratio(
                    self.layout.file_panel_width_ratio as f32,
                    m::PANEL_MIN_SIZE,
                    viewport_w,
                )
            })
        } else {
            0.0
        };
        let right_w = if self.layout.right_panel_visible {
            (self.layout.right_panel_width_px as f32).round()
        } else {
            0.0
        };
        let others = match side {
            PanelSide::Sidebar => file_w + right_w,
            PanelSide::File => sidebar_w + right_w,
        };
        m::layout_math::max_growth_width(viewport_w, others)
    }

    /// Начало драга сплиттера: снять стартовые размеры пары.
    pub(crate) fn begin_drag(&mut self, kind: DragKind, pos: f32, viewport_w: f32) {
        crate::state::drag_flag::set(true);
        // Открытый тултип/пилюля не должны пережить начало драга.
        self.tooltip_live = None;
        self.hover_pill = None;
        let file_w = self.file_w_live.unwrap_or_else(|| {
            m::layout_math::width_from_ratio(
                self.layout.file_panel_width_ratio as f32,
                m::PANEL_MIN_SIZE,
                viewport_w,
            )
        });
        let init = match kind {
            DragKind::Sidebar => (self.layout.sidebar_width_px as f32, 0.0),
            DragKind::MainFile => (file_w, 0.0),
            DragKind::FileRight => (file_w, self.layout.right_panel_width_px as f32),
            DragKind::FileBottom => (self.layout.file_panel_bottom_height_ratio as f32, 0.0),
            DragKind::MainBottom => (self.layout.main_split as f32, 0.0),
            DragKind::RightSplit => (self.right_split, 0.0),
        };
        self.drag = Some(DragState {
            kind,
            start: pos,
            init,
        });
    }

    /// Движение мыши при активном драге.
    ///
    /// Все размеры квантуются к ЦЕЛЫМ логическим px (`round` дельты, не
    /// результата): при dpi 1.25 один шаг мыши = 0.8 логического px, и
    /// округление дробного результата прыгало туда-сюда — сплиттеры дрожали.
    /// Пары (file↔right) двигаются с СОХРАНЕНИЕМ СУММЫ, иначе центральная
    /// колонка (flex-остаток) дёргалась от чужого драга.
    pub(crate) fn drag_move(&mut self, x: f32, y: f32, viewport_w: f32, body_h: f32) {
        let Some(drag) = &self.drag else { return };
        let (kind, start, init) = (drag.kind, drag.start, drag.init);
        match kind {
            DragKind::Sidebar => {
                let d = (x - start).round();
                // clampGrowth оригинала (centre-column-width.ts): расти можно,
                // пока центру остаётся >= MAIN_MIN_WIDTH_PX (100), а НЕ 550
                let max_w = self.max_panel_w(viewport_w, PanelSide::Sidebar);
                self.layout.sidebar_width_px =
                    f64::from((init.0.round() + d).clamp(m::PANEL_MIN_SIZE, max_w));
            }
            DragKind::MainFile => {
                // Граница main|file: вправо → main шире, file уже.
                // ЖИВАЯ ширина — px (как filePanelWidth оригинала); ratio
                // пишем лишь для персиста, поэтому кламп 0.6 не мешает драгу
                let d = (x - start).round();
                let max_f = self.max_panel_w(viewport_w, PanelSide::File);
                let nf = (init.0.round() - d).clamp(m::PANEL_MIN_SIZE, max_f);
                self.file_w_live = Some(nf);
                self.layout.file_panel_width_ratio =
                    f64::from(m::layout_math::ratio_from_width(nf, viewport_w));
            }
            DragKind::FileRight => {
                // Трейд file↔right с СОХРАНЕНИЕМ СУММЫ: main-остаток
                // остаётся неподвижным (RightPanel.tsx)
                let d = (x - start).round();
                let total = (init.0 + init.1).round();
                let max_f = (total - m::PANEL_MIN_SIZE).max(m::PANEL_MIN_SIZE);
                // file — ЖИВЫЕ px, поэтому кламп ratio 0.6 не рвёт инвариант
                let nf = (init.0.round() + d).clamp(m::PANEL_MIN_SIZE, max_f);
                let nr = (total - nf).max(m::PANEL_MIN_SIZE);
                self.file_w_live = Some(nf);
                self.layout.file_panel_width_ratio =
                    f64::from(m::layout_math::ratio_from_width(nf, viewport_w));
                self.layout.right_panel_width_px = f64::from(nr);
            }
            DragKind::FileBottom => {
                // `next = max(BOTTOM_PANE_MIN_HEIGHT_PX, startHeight − deltaY)`
                // (`FilePanel.tsx:75`): драг ведёт ПИКСЕЛЯМИ, минимум жёсткий
                // 100; ratio — только для персиста, база — высота ОКНА
                // (`layout-autosave.ts:100`), а не body (ревью ц.19)
                let vh = self.main_viewport.1.max(1.0);
                let d = (y - start).round();
                let max_bottom =
                    (body_h - m::BOTTOM_PANE_MIN_HEIGHT).max(m::BOTTOM_PANE_MIN_HEIGHT);
                let px_bottom =
                    ((init.0 * vh).round() - d).clamp(m::BOTTOM_PANE_MIN_HEIGHT, max_bottom);
                self.layout.file_panel_bottom_height_ratio =
                    f64::from((px_bottom / vh).clamp(m::BOTTOM_RATIO_MIN, m::BOTTOM_RATIO_MAX));
            }
            DragKind::MainBottom => {
                let h = body_h.max(1.0);
                let d = (y - start).round();
                let px_top = (init.0 * h).round() + d;
                self.layout.main_split =
                    f64::from((px_top.round() / h).clamp(m::MAIN_SPLIT_MIN, m::MAIN_SPLIT_MAX));
            }
            DragKind::RightSplit => {
                // База — высота БЕЗ 10px ручки, как у отрисовки (`right.rs`:
                // верх = split·(col_h − 10)): с полной body_h ручка отставала
                // от курсора на ~2 % и прыгала на split·10 на старте (аудит).
                let h = (body_h - 10.0).max(1.0);
                let d = (y - start).round();
                let px_top = (init.0 * h).round() + d;
                self.right_split =
                    (px_top.round() / h).clamp(m::RIGHT_SPLIT_MIN, m::RIGHT_SPLIT_MAX);
            }
        }
    }

    /// Конец драга: персист снапшота одним патчем.
    pub(crate) fn end_drag(&mut self) {
        crate::state::drag_flag::set(false);
        if self.drag.take().is_some() {
            // px-поля храним ЦЕЛЫМИ: дробные значения из persist давали
            // ±1px скачок на первом кадре после рестарта
            self.layout.sidebar_width_px = self.layout.sidebar_width_px.round();
            self.layout.right_panel_width_px = self.layout.right_panel_width_px.round();
            crate::layout_store::save_patch(serde_json::json!({
                "sidebarWidthPx": self.layout.sidebar_width_px,
                "filePanelWidthRatio": self.layout.file_panel_width_ratio,
                "rightPanelWidthPx": self.layout.right_panel_width_px,
                "filePanelBottomHeightRatio": self.layout.file_panel_bottom_height_ratio,
                "mainSplit": self.layout.main_split,
            }));
            self.persist_active_session_layout();
        }
    }

    /// Снапшот текущей раскладки — за АКТИВНОЙ сессией. Зовётся после каждого
    /// изменения лейаута (драг, тумблеры): раньше снапшот обновлялся только
    /// при УХОДЕ с сессии, и рестарт применял к восстановленной сессии
    /// устаревшую раскладку («лейаут не тот, каким оставил» — жалоба юзера).
    pub(crate) fn persist_active_session_layout(&self) {
        let Some(active) = self
            .sessions
            .as_ref()
            .and_then(|s| s.active_session_id.clone())
        else {
            return;
        };
        if let Ok(mut snap_v) = serde_json::to_value(&self.layout) {
            snap_v["sidebarVisible"] = serde_json::Value::Bool(self.sidebar_visible);
            std::thread::spawn(move || {
                if let Some(c) = crate::host_link::client() {
                    let _ = c.request(
                        "kamin:sessions:setState",
                        vec![
                            serde_json::json!(active),
                            serde_json::json!({ "layout": snap_v }),
                        ],
                    );
                }
            });
        }
    }

    /// Тело активного тула слота (None → placeholder). tree → живое дерево;
    /// terminal → локальный PTY-грид; прочие — заглушка тула.
    /// Хедер contributed-вью (`ContributedContainerBody.module.css .title`):
    /// 4/12, fs-xs uppercase text-muted; титул предпочитает `meta.title`,
    /// рядом — `.viewDescription`, справа — `.viewBadge`.
    /// Гарантировать видимую file-панель в режиме Files (открытие файла из
    /// дерева/поиска/симболов): панель вкл + Web→Files, с персистом.
    pub(crate) fn reveal_file_panel(&mut self) {
        if !self.layout.file_panel_visible {
            self.layout.file_panel_visible = true;
            crate::layout_store::save_patch(serde_json::json!({ "filePanelVisible": true }));
        }
        if self.layout.file_panel_mode != "files" {
            self.layout.file_panel_mode = "files".to_string();
            crate::layout_store::save_patch(serde_json::json!({ "filePanelMode": "files" }));
            // Вне веб-режима строка адреса не нужна (см. SetFileMode).
            self.browser_input = None;
        }
    }

    pub(crate) fn dragging(&self, kind: DragKind) -> bool {
        self.drag.as_ref().is_some_and(|d| d.kind == kind)
    }

    /// Показ грипа: ховер ручки ИЛИ активный драг этого сплиттера.
    pub(crate) fn handle_show(&self, id: &'static str, kind: DragKind) -> bool {
        self.dragging(kind) || (self.drag.is_none() && self.hovered_handle == Some(id))
    }
}
