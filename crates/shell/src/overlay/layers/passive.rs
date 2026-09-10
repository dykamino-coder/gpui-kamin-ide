//! Пассивные слои: живой тултип, тосты, hover-пилюля.
//!
//! Слой вынесен из `OverlayWindow::render` как есть (`plan/100-refactor-250.md`).

use crate::host_link::ShellEvent;
use crate::overlay::hit_area;
use crate::root::RootView;
use gpui::div;
use gpui::prelude::*;

use gpui::Div;

pub(crate) fn add_passive(
    mut layer: Div,
    r: &RootView,
    p: &'static kamin_theme::Palette,
    tx: &smol::channel::Sender<ShellEvent>,
    vw: f32,
    vh: f32,
    window: &mut gpui::Window,
) -> Div {
    // Живой тултип рисует overlay_stack ПОСЛЕДНИМ ребёнком: тултип элемента
    // ВНУТРИ поповера/меню обязан лечь поверх самого поповера (скрин юзера:
    // подсказка кнопки layout-поповера пряталась под его картой).
    let _ = window;

    if !r.toasts.is_empty() {
        // Тосты интерактивны точечно (кнопка dismiss) — hit-регион на
        // контейнере тостов, остальное окно прозрачно для ввода.
        // Обёртка НЕСЁТ размер (нулевой anchor давал интринсик-сжатие
        // карт и нулевой hit-rect — тосты не кликались)
        // Якорим ОТ ЛЕВОГО-ВЕРХНЕГО угла по размеру MAIN-вьюпорта:
        // `bottom/right` резолвятся к вьюпорту overlay-окна, который
        // после win32-ресайза бывает больше main — стопка уезжала за
        // пределы окна и не показывалась вовсе (ревью ц.14 + замер).
        let stack_w = 360.0_f32;
        let stack_h = 260.0_f32.min(vh - 48.0);
        layer = layer.child(
            div()
                .absolute()
                .left(gpui::px((vw - kamin_metrics::SPACE_4 - stack_w).max(8.0)))
                .top(gpui::px((vh - 36.0 - stack_h).max(8.0)))
                .h(gpui::px(stack_h))
                .flex()
                .flex_col()
                .justify_end()
                // `.stack { right: var(--space-4) }`: бокс схлопнут по
                // контенту и прижат ВПРАВО — узкие карты держатся
                // правого края, а не левого края слота (ревью ц.15)
                .items_end()
                // Слот ФИКСИРОВАННОЙ ширины 360: у absolute-бокса без
                // ширины она схлопывается по контенту и `items_end`
                // выравнивать нечему — карта оставалась у левого края
                // (замер ц.15: правый зазор 245 вместо 16)
                .w(gpui::px(stack_w))
                // БЕЗ .relative(): он перезаписывал position=Absolute
                // (последний вызов выигрывает) — тосты улетали в
                // левый-верх потоком
                .child(hit_area())
                .child(crate::ui::toasts::toasts(&r.toasts, &r.toast_timers, tx, p)),
        );
    }

    // Пилюля только по якорю ТОЙ ЖЕ строки и никогда для строки в inline
    // rename: инпут не должен быть перекрыт actions overlay (BR-29).
    if let Some(hp) = r.hover_pill.clone()
        && r.renaming_session.as_deref() != Some(hp.as_str())
        && let Some(a) = crate::ui::sessions_list::anchor_for(&hp)
    {
        // Hover-пилюля строки сессии/группы (fly-out за сайдбар → поверх
        // вебвью возможен только в overlay)
        // (пилюля, её ширина для клампа)
        let inner: Option<(gpui::AnyElement, f32)> = if let Some(pid) = hp.strip_prefix("grp:") {
            r.sessions.as_ref().and_then(|snap| {
                snap.projects.iter().find(|pr| pr.id == pid).map(|pr| {
                    let count = snap.sessions.iter().filter(|s| s.project_id == pid).count();
                    let name = pr
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
                    (
                        crate::ui::sessions_list::project_actions_pill(pid, &name, count, tx, p),
                        // 2 кнопки 24 + gap 2 + p 3×2 + border 1×2
                        58.0_f32,
                    )
                })
            })
        } else {
            r.sessions.as_ref().and_then(|snap| {
                snap.sessions.iter().find(|s| s.id == hp).map(|s| {
                    // rename + [disconnect у открытой] + delete
                    let n = if s.open { 3.0 } else { 2.0 };
                    (
                        crate::ui::sessions_list::session_actions_pill(s, tx, p),
                        // buttons + gaps + padding 3×2 + border 1×2
                        n * 24.0 + (n - 1.0) * 2.0 + 8.0,
                    )
                })
            })
        };
        if let Some((inner, pill_w)) = inner {
            layer = layer.child(crate::ui::sessions_list::overlay_pill(
                inner,
                a,
                (vw, vh),
                pill_w,
                &hp,
                tx,
            ));
        }
    }
    layer
}
