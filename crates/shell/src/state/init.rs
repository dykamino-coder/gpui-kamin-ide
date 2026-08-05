//! Сборка RootView при старте: восстановление раскладки и подписки.
//!
//! Методы перенесены из `root.rs` дословно (`plan/100-refactor-250.md`).

use crate::host_link::ShellEvent;
use crate::state::fs_ops::request_extensions;
use crate::state::model::RootView;
use crate::ui::file_list::TreeState;
use crate::ui::radial_bg::RadialBg;
use gpui::{AppContext as _, Context};
use kamin_metrics as m;
use kamin_theme::ThemeKind;
impl RootView {
    pub fn new(tx: smol::channel::Sender<ShellEvent>, cx: &mut Context<Self>) -> Self {
        // Активный тул сайдбара — из ТОЙ ЖЕ восстановленной модели, что ниже
        // уходит в поле `activity` (иначе два источника истины, ревью ц.8).
        let restored_sidebar_tool: &'static str = {
            let model = match crate::layout_store::load_raw_key("activityModel") {
                Some(v) => crate::activity::ActivityModel::from_json(&v),
                None => crate::activity::ActivityModel::new(),
            };
            match model
                .state(crate::activity::PanelSlot::Sidebar)
                .active
                .as_deref()
            {
                // Нужен именно &'static str — берём id из builtin-таблицы;
                // contributed-тулы (динамические id) остаются на «projects»,
                // их тело всё равно резолвит `tool_body` по слоту.
                Some(id) => crate::activity::BUILTIN_ACTIVITIES
                    .iter()
                    .find(|a| a.id == id)
                    .map(|a| a.id)
                    .unwrap_or("projects"),
                None => "projects",
            }
        };
        // Расширения поднимаются НЕЗАВИСИМО от сессий (как в VS Code:
        // активация по `onStartupFinished`, а не по открытию чего-либо):
        // тянем список сразу при старте, загрузчик сам ретраит до ответа.
        request_extensions(tx.clone());
        let theme_choice: &'static str = match crate::layout_store::load_raw_key("themeChoice")
            .and_then(|v| v.as_str().map(str::to_string))
            .as_deref()
        {
            Some("light") => "light",
            Some("system") => "system",
            _ => "dark",
        };
        // Contributed-тема применяется на буте из КЭША (main.rs, до окна) —
        // здесь только отражаем её в стейте: без contrib_theme_id ветка
        // «system» в canvas_layers перезаписала бы палитру на первом же
        // кадре, а Appearance не показал бы галку.
        let cached_contrib_id: Option<String> =
            crate::layout_store::load_raw_key("contributedThemeId")
                .and_then(|v| v.as_str().map(str::to_string))
                .filter(|_| crate::theme_sync::theme_cache_path().exists());
        let contrib_dark_ui = crate::layout_store::load_raw_key("contributedThemeDarkUi")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        // system резолвится на первом render (нужен window_appearance)
        let theme = if cached_contrib_id.is_some() {
            if contrib_dark_ui {
                ThemeKind::Dark
            } else {
                ThemeKind::Light
            }
        } else if theme_choice == "light" {
            ThemeKind::Light
        } else {
            ThemeKind::Dark
        };
        let layout = crate::layout_store::load();
        Self {
            focus_handle: cx.focus_handle(),
            theme,
            theme_choice,
            file_sub_gen: 0,
            rename_sub: None,
            tree_synced_path: None,
            view_resolve_at: std::collections::HashMap::new(),
            view_resolve_tries: std::collections::HashMap::new(),
            view_resolve_start: std::collections::HashMap::new(),
            icon_themes: Vec::new(),
            icon_theme_id: None,
            sessions: None,
            ws_connected: false,
            host_port: None,
            workspace: None,
            tree_store: cx.new(|_| TreeState::default()),
            file_tree_panels: std::collections::HashMap::new(),
            // Источник истины один: активный тул слота Sidebar из layout.json
            // (хардкод «projects» расходился с восстановленным состоянием —
            // подсветка плитки и тело сайдбара уезжали друг от друга, ц.8)
            sidebar_activity: restored_sidebar_tool,
            sidebar_visible: layout.sidebar_visible,
            tabs_overflow_open: None,
            layout,
            right_split: m::RIGHT_SPLIT_DEFAULT,
            drag: None,
            hovered_handle: None,
            collapsed_projects: std::collections::HashSet::new(),
            inactive_open: std::collections::HashSet::new(),
            webview_scrim: false,
            tooltip_live: None,
            last_mouse: (0.0, 0.0),
            last_frame_viewport: (0.0, 0.0),
            pending_html: std::collections::HashMap::new(),
            modal: None,
            modal_input: None,
            modal_input_sub: None,
            modal_at: None,
            modal_focus_return: None,
            modal_autofocus_pending: false,
            session_menu: None,
            file_menu: None,
            tool_menu_sub: false,
            fs_clipboard: None,
            activity: {
                // Persist панельной модели: restore из layout.json
                let raw = crate::layout_store::load_raw_key("activityModel");
                match raw {
                    Some(v) => crate::activity::ActivityModel::from_json(&v),
                    None => crate::activity::ActivityModel::new(),
                }
            },
            tool_picker: None,
            layout_popover: false,
            appearance_popover: false,
            tool_drag: None,
            chip_drag: None,
            chip_order: crate::layout_store::load_string_list("sessionOrder"),
            new_session_menu: None,
            tool_tab_menu: None,
            fs_undo: Vec::new(),
            switching_to: None,
            chat_cover: None,
            webviews_alive: std::collections::HashSet::new(),
            webview_cover: std::collections::HashMap::new(),
            status_items: std::collections::HashMap::new(),
            diags: std::collections::HashMap::new(),
            explorer_menu: Vec::new(),
            contrib_themes: Vec::new(),
            contrib_theme_id: cached_contrib_id,
            quick_pick: None,
            pending_focus_step: None,
            pending_quit: false,
            layout_booted: false,
            update_progress: None,
            qp_input: None,
            qp_sub: None,
            contrib_keys: std::collections::HashMap::new(),
            update_available: None,
            main_viewport: (1400.0, 900.0),
            pending_view_resolve: false,
            cover_expect_fresh: false,
            output: crate::output_log::OutputChannels::default(),
            renaming_session: None,
            rename_input: None,
            palette_menu: Vec::new(),
            context_keys: crate::when::ContextValues::new(),
            palette_open: false,
            cz: crate::state::model_customize::CustomizeState {
                // Не-Default стартовые значения (пустая строка сломала бы нав)
                customize_panel: "settings",
                syslog_level: "all",
                ext_status: "Connecting to the extension host…".to_string(),
                logs_scroll: gpui::ScrollHandle::new(),
                ..Default::default()
            },
            ed: crate::state::model_editor::EditorState {
                // usize::MAX — «кэш пуст»: 0 совпал бы с первой видимой строкой
                sticky_cache: (String::new(), usize::MAX, Vec::new()),
                ..Default::default()
            },
            term: crate::state::model_term::TerminalState {
                // Persist-дефолтный шелл («звёздочка» в пикере)
                term_default_shell: crate::layout_store::load_raw_key("defaultShellId")
                    .and_then(|v| v.as_str().map(str::to_string)),
                ..Default::default()
            },
            terminal_focus: cx.focus_handle(),
            sov: crate::state::model_search::SearchOverlays::default(),
            palette_input: None,
            palette_sub: None,
            commands: Vec::new(),
            status_counts: crate::ui::status_bar::StatusCounts::default(),
            toasts: Vec::new(),
            toast_timers: std::collections::HashMap::new(),
            web_menu: None,
            hover_pill: None,
            hover_pill_anchor: None,
            hover_pill_panel: None,
            hover_pill_gen: 0,
            // ::before welcome: 220×220, accent 26% → transparent (ревью ц.1)
            welcome_glow: crate::ui::radial_bg::bake_glow_edge(
                220,
                theme.palette().accent_primary,
                0.26,
                // `.logoWrap::before`: `transparent 68%` — свечение гаснет на
                // 68 % радиуса, а не у самого края (ревью ц.15)
                0.68,
            ),

            browser_url_stash: std::collections::HashMap::new(),
            file_w_live: None,
            last_viewport_w: 0.0,
            viewport_settle: None,
            problems_filter: None,
            problems_collapsed: std::collections::HashSet::new(),
            problems_file_cap: 100,
            browser_input: None,
            browser_addr_selected: false,
            design: crate::ui::design_samples::DesignState::default(),
            trees: std::collections::HashMap::new(),
            tree_all_collapsed: false,
            probe_query: None,
            probe_editor_find: false,
            radial_bg: RadialBg::bake(theme.palette()),
            tx: {
                crate::updater::check_in_background(tx.clone());
                tx
            },
        }
    }
}
