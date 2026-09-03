//! Сайдбар sessions-mode (SessionsMode/ProjectGroup/SessionItem 1:1):
//!   [actions: No folder session / New session]
//!   [PROJECTS header]
//!   [группы проектов: chevron + folder-icon + name + count-badge, коллапс]
//!     [active-сессии: dot 4px + label; active = градиент tab-color]
//!     [N inactive sessions toggle → inactive-сессии]
//! Hover-поповеры (deferred pill): сессия=rename/disconnect/delete (+ pin инлайн),
//! группа=add/delete. RMB=полное контекст-меню. time/drag — след. итерация.

pub use crate::ui::sessions::actions::{activate_session, menu_data};
pub use crate::ui::sessions::pill::{anchor_for, overlay_pill};
pub use crate::ui::sessions::pill_menu::{project_actions_pill, session_actions_pill};

use crate::ui::sessions::actions::new_no_folder_session;
use crate::ui::sessions::header::{inactive_toggle, project_header};
use crate::ui::sessions::pill_menu::action_row;
use crate::ui::sessions::row::session_row;

use std::collections::HashSet;

use gpui::prelude::*;
use gpui::{AnyElement, Entity, div, px};
use gpui_component::input::InputState;
use gpui_component::scroll::ScrollableElement as _;
use kamin_metrics as m;
use kamin_model::{Session, SessionsSnapshot};
use kamin_theme::Palette;
use smol::channel::Sender;

use crate::colors::rgba;
use crate::host_link::ShellEvent;

pub fn sessions_sidebar(
    snap: Option<&SessionsSnapshot>,
    collapsed: &HashSet<String>,
    inactive_open: &HashSet<String>,
    renaming: Option<(&str, &Entity<InputState>)>,
    hover_pill: Option<&str>,
    tx: &Sender<ShellEvent>,
    p: &Palette,
) -> AnyElement {
    // Ширина сайдбара прошлого кадра — бюджет усечения меток строк
    let sidebar_w = crate::probe::registry::bounds_of("sidebar")
        .map(|[_, _, w, _]| w)
        .unwrap_or(240.0);
    // Инпут для строки id (если она в переименовании).
    let rename_for = |sid: &str| renaming.filter(|(rid, _)| *rid == sid).map(|(_, inp)| inp);
    let sess_hovered = |sid: &str| hover_pill == Some(sid);
    let grp_hovered = |pid: &str| hover_pill == Some(format!("grp:{pid}").as_str());
    // .root: padding-top space-2
    let mut root = div()
        .id("sidebar")
        .relative()
        .size_full()
        .flex()
        .flex_col()
        .pt(px(m::SPACE_2))
        .text_size(px(m::FS_SM));

    // actions
    root = root.child(
        div()
            .flex()
            .flex_col()
            .px(px(m::SPACE_2))
            .pb(px(m::SPACE_2))
            .pt(px(4.0))
            .child(action_row("No folder session", p, new_no_folder_session))
            // «New session» = сессия В ПАПКЕ: нативный пикер (как «+» титлбара)
            .child(action_row("New session", p, {
                let tx = tx.clone();
                move || {
                    let _ = tx.try_send(ShellEvent::NewSessionInFolderPrompt);
                }
            })),
    );
    // PROJECTS header
    root = root.child(
        div()
            // `.header { flex-shrink: 0 }`
            .flex_shrink_0()
            .pl(px(12.0))
            .pr(px(8.0))
            .py(px(8.0))
            .text_size(px(m::FS_XS))
            // `letter-spacing: 0.08em` (`SessionsMode.module.css:49`) —
            // трекинг появился вендорным патчем ц.31
            .letter_spacing(px(m::FS_XS * 0.08))
            .font(crate::ui::typo::ss01(gpui::FontWeight::MEDIUM))
            .text_color(rgba(p.text_muted))
            .child("PROJECTS"),
    );

    let Some(snap) = snap else {
        // Пока снапшот не пришёл, рисуем ТО ЖЕ пустое состояние, что и при
        // нулевом списке: отдельного «Loading sessions…» у оригинала нет
        // (`SessionsMode.tsx:23-25`) — ревью ц.13
        return root
            .child(
                // `.empty` — ребёнок `.list` (`SessionsMode.tsx:22-24`):
                // его левый инсет 4 (`.list`) + 12 = 16 (ревью ц.16)
                div().pl(px(m::SPACE_1)).pr(px(m::SPACE_1)).child(
                    div()
                        .px(px(m::SPACE_3))
                        .py(px(m::SPACE_3))
                        .text_size(px(m::FS_SM))
                        .text_color(rgba(p.text_muted))
                        .child("No projects yet. Open a folder or start a session."),
                ),
            )
            .into_any_element();
    };
    let active = snap.active_session_id.clone().unwrap_or_default();

    // list: `padding: 0 var(--space-1) var(--space-2)` (оригинал)
    let mut list = div()
        .flex_1()
        .min_h(px(0.))
        // Ширина по КОНТЕЙНЕРУ, а не по контенту: в вертикальном скроллере
        // дети иначе берут max-content, и при сужении сайдбара строки
        // оставались широкими — бейджи «уезжали» от правого края
        // (баг, пойман юзером)
        .w_full()
        .min_w(px(0.))
        .flex()
        .flex_col()
        .pl(px(m::SPACE_1))
        // Шире справа: скроллбар лежит поверх контента и прилипал к
        // бейджам-кружочкам счётчиков — отделяем зазором (юзер).
        .pr(px(m::SPACE_3))
        .pb(px(m::SPACE_2))
        .overflow_y_scrollbar();

    let mut any = false;
    // Проекты — по времени создания (`sessions.ts:62`); порядок доставки
    // хоста не гарантирован (ревью ц.16)
    let mut projects: Vec<&kamin_model::Project> = snap.projects.iter().collect();
    projects.sort_by(|a, b| a.created_at.total_cmp(&b.created_at));
    for project in projects {
        let mut act: Vec<&Session> = snap
            .sessions
            .iter()
            .filter(|s| s.project_id == project.id && s.open)
            .collect();
        let mut inact: Vec<&Session> = snap
            .sessions
            .iter()
            .filter(|s| s.project_id == project.id && !s.open)
            .collect();
        let project_empty = act.is_empty() && inact.is_empty();
        any = true;
        // Активные — в порядке СОЗДАНИЯ (`sessions.ts:65`: «selecting a
        // session must NOT reorder the list»); алфавит был нашей отсебятиной
        // (ревью ц.16). Неактивные — по дате, свежие сверху.
        act.sort_by(|a, b| a.created_at.total_cmp(&b.created_at));
        inact.sort_by(|a, b| b.last_opened.total_cmp(&a.last_opened));
        let name = project
            .folder_path
            .as_deref()
            .map(|f| {
                f.replace('\\', "/")
                    .rsplit('/')
                    .next()
                    .unwrap_or(f)
                    .to_string()
            })
            .unwrap_or_else(|| "No folder".into());
        let is_collapsed = collapsed.contains(&project.id);
        let total = act.len() + inact.len();

        let mut group = div().flex().flex_col().w_full().min_w(px(0.));
        group = group.child(project_header(
            &name,
            sidebar_w,
            total,
            is_collapsed,
            project.id.clone(),
            project.folder_path.as_deref(),
            grp_hovered(&project.id),
            tx,
            p,
        ));
        if !is_collapsed {
            let mut sessions = div().flex().flex_col().gap(px(2.0));
            // «No sessions yet." ТОЛЬКО у полностью пустого проекта
            // (при inactive-only плашка шумела — пойман юзером)
            if project_empty {
                sessions = sessions.child(
                    div()
                        .pl(px(18.0))
                        .py(px(2.0))
                        .text_size(px(m::FS_XS))
                        .text_color(rgba(p.text_muted))
                        .child("No sessions yet."),
                );
            }
            for s in &act {
                sessions = sessions.child(session_row(
                    s,
                    sidebar_w,
                    s.id == active,
                    rename_for(&s.id),
                    sess_hovered(&s.id),
                    tx,
                    p,
                ));
            }
            if !inact.is_empty() {
                let show = inactive_open.contains(&project.id);
                sessions = sessions.child(inactive_toggle(
                    inact.len(),
                    show,
                    project.id.clone(),
                    tx,
                    p,
                ));
                if show {
                    for s in &inact {
                        sessions = sessions.child(session_row(
                            s,
                            sidebar_w,
                            s.id == active,
                            rename_for(&s.id),
                            sess_hovered(&s.id),
                            tx,
                            p,
                        ));
                    }
                }
            }
            group = group.child(sessions);
        }
        list = list.child(group);
    }
    if !any {
        list = list.child(
            div()
                .px(px(12.0))
                .py(px(m::SPACE_3))
                .text_color(rgba(p.text_muted))
                .child("No projects yet. Open a folder or start a session."),
        );
    }
    root.child(list).into_any_element()
}
