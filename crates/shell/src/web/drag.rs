//! Перетаскивание В страницу: внешний drag ОС и внутренний drag шелла.
//!
//! Вью у нас windowless: CEF не видит OLE-сессию окна, и страница без этого
//! модуля не получала ни `dragenter`, ни `drop` (файл из Проводника в чат
//! «работал только через скрепку»). Как в cefclient (osr_dragdrop_win): хост
//! ловит drag сам и отдаёт его вью штатными `drag_target_drag_enter/over/
//! leave/drop` с `DragData` (пути файлов) и координатами вью. Страница
//! получает обычные события и `dataTransfer.files`, любая вью одинаково.
//!
//! Источники: gpui переводит OLE-drag окна во внутренний drag `ExternalPaths`,
//! а дерево файлов тащит `DraggedFile` — оба приходят сюда одним путём.

use cef::{CefString, DragOperationsMask, ImplBrowserHost, ImplDragData, MouseEvent};

/// Что сделать с вью на очередном шаге перетаскивания.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Step {
    Enter(&'static str),
    Over(&'static str),
    Leave(&'static str),
    Drop(&'static str),
}

/// Вью, над которой сейчас идёт drag (вход уже отдан CEF).
static OVER: std::sync::Mutex<Option<&'static str>> = std::sync::Mutex::new(None);

/// Переход по движению: `inside` — курсор над вью `id`. Вход в другую вью
/// сначала закрывает прежнюю, как у ОС при переходе между окнами.
pub(crate) fn on_move(
    over: &mut Option<&'static str>,
    id: &'static str,
    inside: bool,
) -> Vec<Step> {
    match (*over, inside) {
        (Some(cur), true) if cur == id => vec![Step::Over(id)],
        (prev, true) => {
            *over = Some(id);
            prev.map(Step::Leave)
                .into_iter()
                .chain([Step::Enter(id)])
                .collect()
        }
        (Some(cur), false) if cur == id => {
            *over = None;
            vec![Step::Leave(id)]
        }
        _ => Vec::new(),
    }
}

/// Отпускание над вью: без входа (drop пришёл раньше первого движения) вход
/// отдаётся тут же — CEF принимает drop только после enter.
pub(crate) fn on_drop(over: &mut Option<&'static str>, id: &'static str) -> Vec<Step> {
    let mut steps = on_move(over, id, true);
    steps.retain(|s| !matches!(s, Step::Over(_)));
    *over = None;
    steps.push(Step::Drop(id));
    steps
}

/// Drag ушёл из окна или закончился мимо вью.
pub(crate) fn on_exit(over: &mut Option<&'static str>) -> Vec<Step> {
    over.take().map(Step::Leave).into_iter().collect()
}

fn copy() -> DragOperationsMask {
    DragOperationsMask::from(cef::sys::cef_drag_operations_mask_t::DRAG_OPERATION_COPY)
}

/// Выполнить шаги: `paths` нужны только входу, `x/y` — в CSS px вью.
fn run(steps: Vec<Step>, paths: &[String], x: f32, y: f32) {
    let at = || MouseEvent {
        x: x.round() as i32,
        y: y.round() as i32,
        modifiers: 0,
    };
    for step in steps {
        let event = at();
        match step {
            Step::Enter(id) => {
                let paths = paths.to_vec();
                super::input::on_browser(id, move |host| {
                    let Some(mut data) = cef::drag_data_create() else {
                        return;
                    };
                    for p in &paths {
                        let name = std::path::Path::new(p)
                            .file_name()
                            .map(|n| n.to_string_lossy().into_owned())
                            .unwrap_or_default();
                        data.add_file(
                            Some(&CefString::from(p.as_str())),
                            Some(&CefString::from(name.as_str())),
                        );
                    }
                    host.drag_target_drag_enter(Some(&mut data), Some(&event), copy());
                });
            }
            Step::Over(id) => {
                super::input::on_browser(id, move |host| {
                    host.drag_target_drag_over(Some(&event), copy());
                });
            }
            Step::Leave(id) => {
                super::input::on_browser(id, |host| host.drag_target_drag_leave());
            }
            Step::Drop(id) => {
                super::input::on_browser(id, move |host| host.drag_target_drop(Some(&event)));
            }
        }
    }
}

/// Курсор с drag'ом двигается: `inside` — над вью `id`.
pub(crate) fn moved(id: &'static str, inside: bool, paths: &[String], x: f32, y: f32) {
    let steps = match OVER.lock() {
        Ok(mut over) => on_move(&mut over, id, inside),
        Err(_) => return,
    };
    run(steps, paths, x, y);
}

/// Отпустили над вью `id`.
pub(crate) fn dropped(id: &'static str, paths: &[String], x: f32, y: f32) {
    let steps = match OVER.lock() {
        Ok(mut over) => on_drop(&mut over, id),
        Err(_) => return,
    };
    run(steps, paths, x, y);
}

/// Внешний drag покинул окно.
pub(crate) fn exited() {
    let steps = match OVER.lock() {
        Ok(mut over) => on_exit(&mut over),
        Err(_) => return,
    };
    run(steps, &[], 0.0, 0.0);
}

/// Пути внешнего drag'а строками для `DragData`.
fn external_paths(paths: &gpui::ExternalPaths) -> Vec<String> {
    paths
        .paths()
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect()
}

/// Точка окна → CSS px вью с началом `origin`.
fn local(pos: gpui::Point<gpui::Pixels>, origin: gpui::Point<gpui::Pixels>) -> (f32, f32) {
    (f32::from(pos.x - origin.x), f32::from(pos.y - origin.y))
}

/// Принимать drag на вью `id`: внешние файлы ОС и файлы из дерева шелла.
/// `on_drag_move` gpui зовёт у ВСЕХ элементов на каждое движение (фаза
/// capture) — по нему видно и вход, и уход курсора с вью.
pub(crate) trait AcceptDrag {
    fn accept_drag(self, id: &'static str) -> Self;
}

impl AcceptDrag for gpui::Stateful<gpui::Div> {
    fn accept_drag(self, id: &'static str) -> Self {
        use crate::ui::file_list::DraggedFile;
        use gpui::{DragMoveEvent, ExternalPaths, InteractiveElement as _};
        self.on_drag_move::<ExternalPaths>(move |ev: &DragMoveEvent<ExternalPaths>, _w, cx| {
            let pos = ev.event.position;
            let (x, y) = local(pos, ev.bounds.origin);
            let inside = ev.bounds.contains(&pos);
            moved(id, inside, &external_paths(ev.drag(cx)), x, y);
        })
        .on_drag_move::<DraggedFile>(move |ev: &DragMoveEvent<DraggedFile>, _w, cx| {
            let pos = ev.event.position;
            let (x, y) = local(pos, ev.bounds.origin);
            moved(id, ev.bounds.contains(&pos), &ev.drag(cx).paths, x, y);
        })
        .on_drop::<ExternalPaths>(move |paths, w, _cx| {
            let (x, y) = super::element::view_point(id, w.mouse_position());
            dropped(id, &external_paths(paths), x, y);
        })
        .on_drop::<DraggedFile>(move |file, w, _cx| {
            let (x, y) = super::element::view_point(id, w.mouse_position());
            dropped(id, &file.paths, x, y);
        })
        .on_file_drop_exit(|_, _, _| exited())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enter_once_then_over_then_leave() {
        let mut over = None;
        assert_eq!(on_move(&mut over, "chat", true), [Step::Enter("chat")]);
        assert_eq!(on_move(&mut over, "chat", true), [Step::Over("chat")]);
        assert_eq!(on_move(&mut over, "chat", false), [Step::Leave("chat")]);
        assert_eq!(on_move(&mut over, "chat", false), []);
        assert_eq!(over, None);
    }

    #[test]
    fn moving_to_another_view_leaves_the_first() {
        let mut over = None;
        on_move(&mut over, "chat", true);
        // Другая вью видит движение вне себя: ничего не меняется.
        assert_eq!(on_move(&mut over, "tools", false), []);
        assert_eq!(
            on_move(&mut over, "tools", true),
            [Step::Leave("chat"), Step::Enter("tools")]
        );
        assert_eq!(over, Some("tools"));
    }

    #[test]
    fn drop_without_a_move_enters_first_and_ends_the_session() {
        let mut over = None;
        assert_eq!(
            on_drop(&mut over, "chat"),
            [Step::Enter("chat"), Step::Drop("chat")]
        );
        assert_eq!(over, None);
        on_move(&mut over, "chat", true);
        assert_eq!(on_drop(&mut over, "chat"), [Step::Drop("chat")]);
        assert_eq!(on_exit(&mut over), []);
    }

    #[test]
    fn leaving_the_window_leaves_the_view() {
        let mut over = None;
        on_move(&mut over, "chat", true);
        assert_eq!(on_exit(&mut over), [Step::Leave("chat")]);
        assert_eq!(on_exit(&mut over), []);
    }
}
