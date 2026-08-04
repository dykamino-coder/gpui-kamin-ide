//! Контекст-меню дерева файлов (file-context-menu.ts 1:1, builtin-часть):
//! папка: New File… / New Folder… | Cut / Copy / Paste | Rename… / Delete |
//! Copy Path / Copy Relative Path; файл — то же без New-группы.
//! (Open In-каскад и contributed explorer/context — фаза расширений.)
//! Рендерится в overlay-окне (единый слой), hit_area в корне.

use crate::host::events::EdEvent;
use crate::host::events::ShellEvent;
pub use crate::ui::fmenu::model::{ContribMenuItem, FileMenu};

use crate::ui::fmenu::items::{FA_OPEN_IN, MARGIN, MENU_W, divider, icon_slot};
use gpui::prelude::*;
use gpui::{AnyElement, div, px};

/// Грация закрытия каскада «Open In» (оригинал: close-delay 250ms).
const CASCADE_CLOSE_DELAY_MS: u64 = 250;
/// Поколение каскада: bump отменяет отложенное закрытие (см. close_sub).
static CASCADE_GEN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
use gpui_component::scroll::ScrollableElement as _;
use kamin_metrics as m;
use kamin_theme::Palette;
use smol::channel::Sender;

use crate::colors::{rgba, tint};

/// Рендер меню (FileContextMenu 1:1): Open In ▸ | [dir: New File/Folder] |
/// Cut/Copy/Paste | Rename…/Delete | Copy Path/Copy Relative Path |
/// contributed. Каскад — отдельный бокс справа (влево при нехватке места),
/// открывается по ховеру, закрывается ховером других пунктов.
pub fn file_menu(
    menu: &FileMenu,
    contributed: &[ContribMenuItem],
    tx: &Sender<ShellEvent>,
    viewport_w: f32,
    viewport_h: f32,
    p: &Palette,
) -> AnyElement {
    // Измеренная коробка прошлого кадра; до первого замера — оценка
    let [_, _, meas_w, meas_h] =
        crate::probe::registry::bounds_of("file-menu").unwrap_or([0.0, 0.0, MENU_W, 0.0]);
    // `visibility: hidden` до первого замера (`FileContextMenu.tsx:137`):
    // иначе первый кадр рисуется по ОЦЕНКЕ и меню видимо «прыгает»
    let measured = meas_w > 1.0 && meas_h > 1.0;
    let menu_w = meas_w.max(MENU_W);
    let est_h = if meas_h > 1.0 {
        meas_h
    } else if menu.is_dir {
        380.0
    } else {
        330.0
    };
    // `probe_area` меряет content-box; внешняя коробка шире на паддинги 4×2
    // и рамки 1×2 — по ней и клампим (ревью ц.18: меню свисало на 10 px)
    // `probe_area` меряет PADDING-box: паддинги 4×2 УЖЕ внутри, снаружи
    // остаются только рамки 1×2 (ревью ц.21: двойной учёт уводил меню на
    // 5.2 px влево и на 9 px раньше переворачивал вверх)
    let outer_w = menu_w + 2.0;
    let outer_h = est_h + 2.0;
    // `clampToViewport(side: "bottom")` с НУЛЕВЫМ якорем в курсоре
    // (`clamp-popup.ts:99-101`): по горизонтали меню центрируется на курсоре,
    // по вертикали — переворачивается вверх, если снизу не влезает, а сверху
    // влезает (ревью ц.18: было «левый край на курсоре» + съезд, Δ 94 и 106 px)
    let x = (menu.x - outer_w / 2.0).clamp(MARGIN, (viewport_w - outer_w - MARGIN).max(MARGIN));
    // `clampToViewport` для side bottom клампит ТОЛЬКО поперечную ось:
    // при нехватке места снизу меню ПЕРЕВОРАЧИВАЕТСЯ, иначе top остаётся на
    // курсоре, а хвост уходит в собственный скролл (ревью ц.21)
    let y = if menu.y + outer_h > viewport_h - MARGIN && menu.y - outer_h >= MARGIN {
        menu.y - outer_h
    } else {
        menu.y.max(MARGIN)
    };
    let path = menu.path.clone();
    let is_dir = menu.is_dir;
    // Каталог-цель вставки: сама папка ИЛИ родитель файла
    let paste_dir = if is_dir {
        path.clone()
    } else {
        std::path::Path::new(&path)
            .parent()
            .map(|d| d.to_string_lossy().to_string())
            .unwrap_or_else(|| path.clone())
    };

    let mut col = div()
        .id("file-menu")
        .occlude()
        // До первого замера коробка невидима, но место занимает — ровно
        // `visibility: hidden` оригинала (ревью ц.21)
        .when(!measured, |d| d.invisible())
        // Замер собственной коробки для анкора СЛЕДУЮЩЕГО кадра
        .child(crate::probe::registry::probe_area("file-menu"))
        .absolute()
        .left(px(x))
        .top(px(y))
        .min_w(px(MENU_W))
        // `.menu { max-height: calc(100vh - 16px) }`
        .max_h(px((viewport_h - 16.0).max(80.0)))
        // `.menu { max-width: calc(100vw - 16px); overflow-y: auto }` —
        // переполнение скроллится, а не обрезается (ревью ц.13)
        .max_w(px((viewport_w - 16.0).max(MENU_W)))
        .overflow_y_scroll()
        .flex()
        .flex_col()
        .gap(px(1.0))
        .p(px(m::SPACE_1))
        .rounded(px(m::RADIUS_MD))
        .bg(rgba(p.bg_surface))
        .border_1()
        .border_color(tint(rgba(p.text_primary), 0.06))
        .shadow(crate::overlay::dropdown_shadow())
        .child(crate::overlay::hit_area());

    // ── Open In ▸ (первым, group "open"; каскад по ховеру)
    {
        let hover_bg = tint(rgba(p.text_primary), 0.10);
        let tx_open = tx.clone();
        let mut row = div()
            .id("fm-openin")
            .flex()
            .items_center()
            .gap(px(m::SPACE_2))
            .px(px(m::SPACE_3))
            .py(px(m::SPACE_2))
            .rounded(px(m::RADIUS_SM))
            .text_size(px(m::FS_SM))
            .text_color(rgba(p.text_primary))
            .cursor_pointer()
            .hover(move |s| s.bg(hover_bg))
            .on_hover(move |hovered: &bool, _, _| {
                if *hovered {
                    // Возврат на строку отменяет отложенное закрытие каскада
                    // (bump поколения — см. close_sub ниже).
                    CASCADE_GEN.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let _ = tx_open.try_send(ShellEvent::Ed(EdEvent::FileMenuOpenIn(true)));
                }
            })
            // Якорь каскада = rect ЭТОЙ строки (`e.currentTarget
            // .getBoundingClientRect()`), поэтому меряем её саму, а не
            // выводим из коробки меню (ревью ц.23: вывод промахивался)
            .child(crate::probe::registry::probe_area("file-menu-openin-row"))
            .child(icon_slot(FA_OPEN_IN, false, p))
            .child(div().flex_1().whitespace_nowrap().child("Open In"))
            .child(
                // `.chevron{font-size:12px}` стоит на самом `<i class="codicon
                // …">` (0,1,0) и проигрывает вендорной базе (0,2,0) → 16;
                // плюс `margin-left: var(--space-2)` (ревью ц.13)
                crate::ui::icon::codicon("\u{eab6}", 16.0)
                    .ml(px(m::SPACE_2)) // chevron-right
                    .text_color(rgba(p.text_muted)),
            );
        if menu.open_in {
            row = row.bg(tint(rgba(p.text_primary), 0.10));
        }
        col = col.child(row).child(divider(p));
    }

    // Ховер прочих корневых пунктов закрывает каскад С ГРАЦИЕЙ 250мс
    // (оригинал: close-delay — диагональный путь мыши к подменю задевает
    // соседние пункты и не должен ронять каскад). Отмена — bump поколения
    // при возврате на строку «Open In».
    let close_sub = {
        let tx = tx.clone();
        move || {
            let my_gen =
                CASCADE_GEN.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
            let tx = tx.clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(CASCADE_CLOSE_DELAY_MS));
                if CASCADE_GEN.load(std::sync::atomic::Ordering::Relaxed) == my_gen {
                    let _ = tx.try_send(ShellEvent::Ed(EdEvent::FileMenuOpenIn(false)));
                }
            });
        }
    };

    // Остальные пункты живут в своей группе: ховер по ней закрывает каскад
    // (раньше `close_sub` был мёртвой привязкой)
    let mut rest = div()
        .id("file-menu-rest")
        .flex()
        .flex_col()
        .gap(px(1.0))
        .on_hover(move |h: &bool, _, _| {
            if *h {
                close_sub();
            }
        });

    rest = crate::ui::fmenu::sections::builtin_items(rest, p, tx, &path, is_dir, paste_dir, menu);
    rest = crate::ui::fmenu::sections::contributed_items(rest, p, tx, &path, is_dir, contributed);

    // `overflow-y: auto` (`FileContextMenu.module.css:19`) с ВИДИМЫМ
    // ползунком — на КОЛОНКЕ ПУНКТОВ. Оборачивать саму коробку меню нельзя:
    // `Scrollable` тянет обёртку на всю доступную ширину, и меню растягивалось
    // на весь экран (баг найден юзером, ц.35)
    col = col.child(rest.overflow_y_scrollbar());

    // ── Каскад «Open In»: отдельный бокс справа от меню; влево при нехватке
    let mut layer = div().absolute().top_0().left_0().size_full().child(col);
    layer = crate::ui::fmenu::open_in::open_in(
        layer, menu, is_dir, x, y, menu_w, viewport_w, viewport_h, tx, p,
    );
    layer.into_any_element()
}

#[cfg(test)]
mod tests {
    use crate::ui::fmenu::model::{group_key, when_allows};

    #[test]
    fn when_literals() {
        let p = r"C:\dir\file.ts";
        assert!(when_allows("", p, true));
        assert!(when_allows("explorerResourceIsFolder", p, true));
        assert!(!when_allows("explorerResourceIsFolder", p, false));
        assert!(when_allows("!explorerResourceIsFolder", p, false));
        // Полный движок: сравнения по контексту узла работают
        // (небуквенный RHS — в кавычках, как в when-clause.ts)
        assert!(when_allows("resourceExtname == '.ts'", p, false));
        assert!(!when_allows("resourceExtname == '.js'", p, false));
        assert!(when_allows("resourceFilename =~ /file/", p, false));
        // Неизвестный ключ — fail-closed
        assert!(!when_allows("someUnknownKey", p, true));
    }

    #[test]
    fn group_order_navigation_first() {
        let mut v = vec![
            "2_workspace@1",
            "navigation@2",
            "1_modification",
            "navigation@1",
        ];
        v.sort_by_key(|g| group_key(g));
        assert_eq!(
            v,
            vec![
                "navigation@1",
                "navigation@2",
                "1_modification",
                "2_workspace@1"
            ]
        );
    }
}
