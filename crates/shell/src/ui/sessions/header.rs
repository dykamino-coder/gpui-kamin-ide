//! Шапка проекта и переключатель неактивных сессий.
//!
//! Перенесено без изменения поведения (`plan/100-refactor-250.md`).

use crate::colors::rgba;
use crate::host_link::ShellEvent;
use crate::ui::icon::{CHEVRON_DOWN, CHEVRON_RIGHT, codicon};
use crate::ui::sessions::pill::anchor_probe;
use gpui::prelude::*;
use gpui::{AnyElement, SharedString, div, px};
use kamin_metrics as m;
use kamin_theme::Palette;
use smol::channel::Sender;

/// Строка "N inactive sessions" toggle.
pub(crate) fn inactive_toggle(
    count: usize,
    open: bool,
    id: String,
    tx: &Sender<ShellEvent>,
    p: &Palette,
) -> AnyElement {
    let hf = rgba(p.text_secondary);
    let tx = tx.clone();
    div()
        .id(SharedString::from(format!("inact-{id}")))
        .flex()
        .items_center()
        .gap(px(6.0))
        .w_full()
        .pl(px(18.0))
        .pr(px(8.0))
        .py(px(3.0))
        .text_size(px(m::FS_SM))
        .text_color(rgba(p.text_disabled))
        .cursor_pointer()
        .hover(move |s| s.text_color(hf))
        .on_mouse_down(gpui::MouseButton::Left, move |_, _, _| {
            let _ = tx.try_send(ShellEvent::ToggleInactive(id.clone()));
        })
        .child(codicon(
            if open { CHEVRON_DOWN } else { CHEVRON_RIGHT },
            12.0,
        ))
        .child(format!(
            "{count} inactive session{}",
            if count == 1 { "" } else { "s" }
        ))
        .into_any_element()
}
// Компонент дизайн-системы: аргументы — его пропсы.
#[allow(clippy::too_many_arguments)]
/// Заголовок группы проекта — 26px, chevron + folder-icon + name + count-badge.
pub(crate) fn project_header(
    name: &str,
    // Ширина сайдбара прошлого кадра — бюджет усечения имени
    sidebar_w: f32,
    count: usize,
    collapsed: bool,
    pid: String,
    // Путь папки проекта: он идёт в тултип (`data-tooltip={folderPath}`),
    // а `pid` — это UUID из хоста (ревью ц.14)
    folder_path: Option<&str>,
    hovered: bool,
    tx: &Sender<ShellEvent>,
    p: &Palette,
) -> AnyElement {
    let hf = rgba(p.text_primary);
    let tx = tx.clone();
    let hover_key = format!("grp:{pid}");
    // `data-tooltip={folderPath ?? "Sessions without a folder"}`
    // (`ProjectGroup.tsx:49`) — усечённое эллипсисом имя иначе не прочитать
    let title_tip: SharedString = match folder_path {
        Some(fp) if !fp.is_empty() => fp.to_string().into(),
        _ => "Sessions without a folder".into(),
    };
    // Hover-пилюля группы: add-session + delete-project (вылетает вправо);
    // рендерится только при hovered (см. pill_wrap).
    // Якорь hover-пилюли: абсолютный ребёнок БЕЗ `deferred` — обёртка
    // участвовала в раскладке строки и уводила бейдж-счётчик от правого
    // края при сужении сайдбара (баг, пойман юзером)
    let actions_pill = hovered.then(anchor_probe);
    div()
        .id(SharedString::from(format!("grp-{pid}")))
        .relative()
        .flex()
        .items_center()
        .gap(px(6.0))
        // ЖЁСТКАЯ ширина, не w_full: внутри вертикального скроллера percent
        // изредка резолвился по контенту (у группы с выбранной сессией), и
        // строка схлопывалась — бейдж «прилипал» к имени (скрины юзера;
        // репро плавало от длины метки времени соседней карточки). Ширина
        // известна: сайдбар минус инсеты (root SPACE_1 + list SPACE_1 с
        // КАЖДОЙ стороны; правый инсет списка теперь SPACE_3 (зазор от
        // скроллбара) → сумма 24. Со старым «−16» строка была на 8 шире
        // контейнера и бейджи резались краем (поймано юзером дважды).
        .w(px((sidebar_w - 24.0).max(60.0)))
        .min_w(px(0.))
        .h(px(26.0))
        .tooltip(crate::ui::tooltip::tooltip(title_tip))
        .pl(px(6.0))
        .pr(px(4.0))
        .text_size(px(m::FS_SM))
        .font_weight(gpui::FontWeight::MEDIUM)
        .text_color(rgba(p.text_secondary))
        .cursor_pointer()
        .overflow_hidden()
        .hover(move |s| s.text_color(hf))
        .on_hover({
            let tx = tx.clone();
            move |h: &bool, _, _| {
                let _ = tx.try_send(ShellEvent::HoverPill(h.then(|| hover_key.clone())));
            }
        })
        .on_mouse_down(gpui::MouseButton::Right, {
            let tx = tx.clone();
            let pid = pid.clone();
            let name = name.to_string();
            move |_e: &gpui::MouseDownEvent, _, cx| {
                cx.stop_propagation();
                let _ = tx.try_send(ShellEvent::OpenModal(crate::ui::modal::Modal {
                    title: "Delete project?".into(),
                    // Пустой проект — ОТДЕЛЬНЫЙ текст без «This cannot be
                    // undone» (`sessions-ui.ts:28`, ревью ц.21)
                    body: if count == 0 {
                        format!("Empty project “{name}” will be removed.").into()
                    } else {
                        format!(
                            "Project “{name}” and its {count} session{} will be removed. This cannot be undone.",
                            if count == 1 { "" } else { "s" }
                        )
                        .into()
                    },
                    confirm_label: "Delete".into(),
                    danger: true,
                    prompt: None,
                    placeholder: None,
                    validate: None,
                    cancel_label: None,
                    action: crate::ui::modal::ModalAction::DeleteProject(pid.clone()),
                }));
            }
        })
        .on_mouse_down(gpui::MouseButton::Left, move |_, _, _| {
            let _ = tx.try_send(ShellEvent::ToggleProjectCollapse(pid.clone()));
        })
        .child(
            // `.chevron` — width 16, text-align center, font-size 13
            div()
                .flex_shrink_0()
                .w(px(16.0))
                .flex()
                .justify_center()
                .text_color(rgba(p.text_muted))
                .child(codicon(
                    if collapsed {
                        CHEVRON_RIGHT
                    } else {
                        CHEVRON_DOWN
                    },
                    // `.chevron{13px}` на самом `.codicon` → база 16 (ревью ц.14)
                    16.0,
                )),
        )
        .child(
            // Catppuccin folder-иконка ПО ИМЕНИ папки (TreeIcon оригинала:
            // Downloads со стрелкой, test с карандашом и т.д.)
            crate::icon_theme::folder_img(name, !collapsed, false)
                .flex_shrink_0()
                .w(px(16.0))
                .h(px(16.0)),
        )
        .child(
            div()
                .flex_1()
                .min_w(px(0.))
                .overflow_hidden()
                .text_ellipsis()
                .whitespace_nowrap()
                // Многоточие дописывает `text_fit` (ревью ц.20/21); бюджет —
                // ширина сайдбара минус pl 6 / pr 4, шеврон 16, иконка 16,
                // два гэпа 6 и бейдж-счётчик 22
                .mr(px(26.0))
                .child(crate::ui::text_fit::fit_approx(
                    name,
                    // Замер ц.23 по кадру: реальный бокс имени группы при
                    // сайдбаре 212.8 = 120.0 → вычет 92.8; +8 к вычету при
                    // расширении правого инсета списка до SPACE_3.
                    sidebar_w - 100.8,
                    m::FS_SM,
                )),
        )
        .child(
            // count-badge: bg-surface, radius 9, min-w 16, h 16.
            // АБСОЛЮТНО у правого края: прижим через flex-1 имени иногда
            // флачил — min-content соседней карточки сессии (метка «58m» на
            // символ шире «1h») изредка глушил растяжение, и кружок прилипал
            // к имени (поймано юзером на «определённой ширине» сайдбара;
            // репро зависело от ВРЕМЕНИ сессии). Абсолюту флex безразличен.
            div()
                .absolute()
                .right(px(4.0))
                .top(px(5.0))
                .flex_shrink_0()
                .min_w(px(16.0))
                .h(px(16.0))
                .px(px(5.0))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(9.0))
                .bg(rgba(p.bg_surface))
                .text_size(px(m::FS_XS))
                .text_color(rgba(p.text_muted))
                .child(format!("{count}")),
        )
        .when_some(actions_pill, |row, pill| row.child(pill))
        .into_any_element()
}
