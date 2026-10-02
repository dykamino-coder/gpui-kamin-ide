//! Прогон reftest-ов WPT: тест и эталон рисуются НАШИМ движком, снимки
//! сравниваются между собой.
//!
//!     cargo run -p kamin-html --example wptrun -- <список.txt> [ширина] [высота]
//!
//! Формат списка: по строке на пару, `тест|эталон`. Эталон в reftest написан
//! примитивной вёрсткой, дающей заведомо тот же результат, поэтому
//! эталонного снимка из браузера не нужно: если наша раскладка верна, оба
//! файла дают одинаковую картинку.
//!
//! Всё в ОДНОМ процессе и одном окне: запуск окна стоит секунды, а пар —
//! тысячи. Документ подменяется в той же сущности, кадр снимается прямо
//! отсюда (`PrintWindow`), и следующая пара идёт без перезапуска.

use gpui::{
    AppContext as _, Application, Bounds, Context, Entity, IntoElement, ParentElement, Render,
    Styled, Timer, TitlebarOptions, Window, WindowBackgroundAppearance, WindowBounds,
    WindowDecorations, WindowOptions, div, point, px, rgb, size,
};
use kamin_html::{BROWSER_CSS, Document, RenderOpts, render};
use std::rc::Rc;
use std::time::Duration;

struct Page {
    doc: Rc<Document>,
}

impl Render for Page {
    fn render(&mut self, window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let mut text = window.text_style();
        text.color = rgb(0x000000).into();
        // Умолчание документа в браузере на Windows — Times New Roman 16px.
        // Тесты, которые шрифт не задают, меряются ИМ: ширина слова решает,
        // куда встанет перенос и до какого минимума сожмётся элемент ряда.
        text.font_family = "Times New Roman".into();
        // Полноширинную подстановку для кириллицы/кана НЕ навязываем: замер
        // показал 1041 → 1006 по css-text. Метрики системной подстановки
        // (кана в 0.82 кегля) совпадают с эталонами чаще, чем `MS Gothic`.
        text.font_size = px(16.).into();
        // Печатная пара: страница WPT по умолчанию 5in x 3in = 480x288
        // точек; `@page` может задать size/margin/фон/рамку. Область
        // просмотра документа = page area (css-page-3 §page-model).
        // Стопка страниц (css-page-3) — ПО ФЛАГУ `WPT_PAGE`: печатные пары
        // сегодня зелены ложно (обе стороны рисуются непагинированным
        // документом), и честная пагинация краснит их, пока не умеет того,
        // чем пользуются эталоны. ★ ЗАМЕРЕНО (06.09), срез 1303 пары
        // (css-page + все `-print` + css-break): шаг 1 без разрезов внутри
        // детей 475 -> 443 (+2/-34); с разрезами класса A и маской по
        // фрагменту 475 -> 435 (+9/-49). Потери — таблицы (разрыв между
        // рядами, `table-fragmentation-*`, `*-page-break-inside-avoid-*`),
        // `monolithic-overflow-*` (contain:size + vh), именованные страницы,
        // `page-margin-006`. Включать по умолчанию после шагов 2-4 плана
        // `target/scout-pagination-2026-09.md`. Прежние 197 HUNG давал не
        // лист, а КРОП снимка: `ink()` при разной длине кадра и разделителя
        // возвращал `usize::MAX`, вердикт умножал его на 8 — переполнение в
        // debug-сборке роняло задачу стенда. Кропа больше нет.
        let page = (kamin_html::css::PRINT_MEDIA.load(std::sync::atomic::Ordering::Relaxed)
            && std::env::var("WPT_PAGE").is_ok())
        .then(|| {
            // Лист — по своему номеру и имени страницы: каскад `@page`
            // (`:first`, `:left`/`:right`, имена; специфичность (f, g, h),
            // css-page-3 §cascading-and-page-context) решает
            // `css::page_decls_in`. `margin: inherit` — от корневого элемента
            // (§page-properties: «The page context inherits from the root
            // element»; `page-margin-006`).
            let rules = kamin_html::css::page_rules_snapshot();
            let root_margin = self
                .doc
                .nodes()
                .iter()
                .find_map(|n| match n {
                    kamin_html::dom::Node::Element(e) if e.tag == "html" => Some(e),
                    _ => None,
                })
                .map(|e| {
                    let px = |l: &Option<kamin_html::value::Len>| match l {
                        Some(kamin_html::value::Len::Px(v)) => *v,
                        _ => 0.0,
                    };
                    let m = &e.style.margin;
                    [px(&m.top), px(&m.right), px(&m.bottom), px(&m.left)]
                })
                .unwrap_or([0.0; 4]);
            // Режим письма листа — от КОРНЯ (css-page-3 §page-properties: «The
            // page context inherits from the root element»; Blink `StyleForPage`
            // наследует от documentElement), `writing-mode` внутри `@page` не
            // действует (`page-box-009`: «should be in horizontal-tb»). Нужен
            // логическим полям и отступам листа (`page-box-008`).
            let root_wm = self
                .doc
                .nodes()
                .iter()
                .find_map(|n| match n {
                    kamin_html::dom::Node::Element(e) if e.tag == "html" => Some(e),
                    _ => None,
                })
                .map(|e| {
                    (
                        e.style.vertical == Some(true),
                        e.style.vertical_rl == Some(true),
                        e.style.rtl == Some(true),
                    )
                })
                .unwrap_or((false, false, false));
            let rtl = root_wm.2;
            let first = kamin_html::render::first_page_name(self.doc.nodes());
            let boxes: std::rc::Rc<dyn Fn(usize, &str) -> PageBox> =
                std::rc::Rc::new(move |i, name: &str| {
                    page_box(
                        kamin_html::css::page_decls_in(&rules, i, name, rtl),
                        root_margin,
                        root_wm,
                    )
                });
            let margins: kamin_html::render::PageMarginDeclsFn = {
                let rules = kamin_html::css::page_rules_snapshot();
                std::rc::Rc::new(move |i, name: &str| {
                    (
                        kamin_html::css::page_decls_in(&rules, i, name, rtl),
                        kamin_html::css::page_margins_in(&rules, i, name, rtl),
                    )
                })
            };
            (boxes(0, &first), boxes, margins)
        });
        let opts = RenderOpts {
            viewport: page.as_ref().map(|p| (p.0.area.0, p.0.area.1)).unwrap_or((
                f32::from(window.viewport_size().width),
                f32::from(window.viewport_size().height),
            )),
            text,
            normal_line_height: 1.31,
            doc_salt: self.doc.salt(),
        };
        // Начальный содержащий блок — ВИДИМАЯ ОБЛАСТЬ, и размер ему нужен
        // точный, в точках. С долей (`size_full`) высота корня остаётся
        // неопределённой, и абсолютный элемент, растянутый краями
        // (`top: 0; bottom: 0`), схлопывается в ноль — целые семейства
        // css-backgrounds и css-position выходили пустыми.
        if std::env::var("HTML_VIEWPORT").is_ok() {
            eprintln!(
                "VIEWPORT {:?} print={} page={:?}",
                opts.viewport,
                kamin_html::css::PRINT_MEDIA.load(std::sync::atomic::Ordering::Relaxed),
                page.as_ref().map(|p| (p.0.size, p.0.area, p.0.margin))
            );
        }
        if let Some((_, boxes, margins)) = page {
            // Печатная пара: стопка страниц (css-page-3) на всё окно —
            // движок сам режет документ по page area, рисует листы и
            // масштабирует их сеткой в окно. Сравнивается весь кадр.
            let geom_for: kamin_html::flow::PageGeomFn = std::rc::Rc::new(move |i, name: &str| {
                let p = boxes(i, name);
                kamin_html::flow::PageGeom {
                    size: p.size,
                    margin: p.margin,
                    border: (p.border.0, p.border.1.to_hsla()),
                    // Page area внутри отступов листа (`PageGeom::area_origin`).
                    padding: p.padding,
                    bg: p.bg.map(|c| c.to_hsla()).unwrap_or(gpui::white()),
                    canvas: None,
                    outline: (p.outline.0, p.outline.1, p.outline.2.to_hsla()),
                    area: p.area,
                }
            });
            let stack =
                kamin_html::render::render_paged(self.doc.nodes(), &opts, geom_for, Some(margins));
            return div()
                .w(px(f32::from(window.viewport_size().width)))
                .h(px(f32::from(window.viewport_size().height)))
                .bg(rgb(0xffffff))
                .text_size(px(16.))
                .child(stack)
                .into_any_element();
        }
        let children = render(self.doc.nodes(), &opts);
        if std::env::var("HTML_VIEWPORT").is_ok() {
            eprintln!("BUILT {} детей", children.len());
        }
        div()
            .w(px(opts.viewport.0))
            .h(px(opts.viewport.1))
            .bg(rgb(0xffffff))
            .text_size(px(16.))
            .children(children)
            .into_any_element()
    }
}

// Кропа по листу больше нет (см. комментарий у `WPT_NO_PAGE`): печатная пара
// сравнивается по всему окну, как остальные, — стопка страниц рисует листы
// сама и масштабирует их в окно.

/// Прочитать подключённую таблицу стилей БАЙТАМИ, как это делает браузер
/// с wpt-сервером.
///
/// `read_to_string` молча съедал любой не-UTF-8 файл (`unwrap_or_default`
/// давал пустую таблицу — семь at-charset-тестов краснели одинаковыми 3.53).
/// Рядом с файлом может лежать `<имя>.headers` с HTTP-заголовками: не
/// `text/css` — таблица не подключается вовсе; `charset=` из заголовка
/// слабее метки порядка байтов, но сильнее `@charset` в самом файле.
fn read_stylesheet(file: &std::path::Path) -> String {
    let Ok(bytes) = std::fs::read(file) else {
        return String::new();
    };
    let headers = std::fs::read_to_string(format!("{}.headers", file.display())).ok();
    let content_type = headers.as_deref().and_then(|h| {
        h.lines()
            .find(|l| l.to_ascii_lowercase().starts_with("content-type:"))
            .map(|l| l[13..].trim().to_string())
    });
    if let Some(ct) = &content_type
        && !ct.to_ascii_lowercase().starts_with("text/css")
    {
        return String::new();
    }
    let at_charset = || {
        let head = bytes.get(..bytes.len().min(64))?;
        let text = head.strip_prefix(b"@charset \x22")?;
        let end = text.iter().position(|b| *b == b'\x22')?;
        encoding_rs::Encoding::for_label(&text[..end])
    };
    let enc = kamin_html::encoding::from_bom(&bytes)
        .or_else(|| {
            content_type
                .as_deref()
                .and_then(kamin_html::encoding::from_content_type)
        })
        .or_else(at_charset)
        .map(kamin_html::encoding::fix_utf16)
        .unwrap_or(encoding_rs::UTF_8);
    enc.decode(&bytes).0.into_owned()
}

/// Обрезать снимок окна до логического прямоугольника (учёт плотности).
fn crop_shot(shot: (u32, u32, Vec<u8>), win: (f32, f32), area: (f32, f32)) -> (u32, u32, Vec<u8>) {
    let (w, h, bytes) = shot;
    let sx = w as f32 / win.0.max(1.0);
    let sy = h as f32 / win.1.max(1.0);
    let cw = ((area.0 * sx).round() as u32).clamp(1, w);
    let ch = ((area.1 * sy).round() as u32).clamp(1, h);
    let mut out = Vec::with_capacity((cw * ch * 4) as usize);
    for y in 0..ch {
        let start = ((y * w) * 4) as usize;
        out.extend_from_slice(&bytes[start..start + (cw * 4) as usize]);
    }
    (cw, ch, out)
}

/// Вычисленная коробка страницы печатной пары.
struct PageBox {
    /// Полный размер листа.
    size: (f32, f32),
    /// Поля: верх/право/низ/лево.
    margin: [f32; 4],
    /// Отступы листа: верх/право/низ/лево. Page area — КОНТЕНТНАЯ область
    /// коробки страницы, внутри отступов (css-page-3 §page-model; Blink
    /// `ResolvePageBoxGeometry` считает их как у обычного блока).
    padding: [f32; 4],
    /// Область содержимого (page area).
    area: (f32, f32),
    bg: Option<kamin_html::value::Color>,
    border: (f32, kamin_html::value::Color),
    /// Контур листа: толщина, сдвиг, цвет (`outline`/`outline-offset`).
    outline: (f32, f32, kamin_html::value::Color),
}

fn page_box(decls: Vec<(String, String)>, root_margin: [f32; 4], wm: (bool, bool, bool)) -> PageBox {
    use kamin_html::value::{Color, Len};
    let (mut w, mut h) = (480.0f32, 288.0f32);
    // Поля листа по умолчанию — ЗАМЕР отдельным прогоном: WPT их не
    // оговаривает, но `monolithic-overflow-027/-028/-029` сходятся с эталоном
    // только при page area 2in (5in x 3in при полях 0.5in = 48px, wpt#40788;
    // `media-queries-001`: «WPT tests that assume that there's a half-inch
    // margin on each side»). `WPT_PAGE_MARGIN=48` против нуля.
    let default_margin: f32 = std::env::var("WPT_PAGE_MARGIN")
        .ok()
        .and_then(|v| v.parse().ok())
        // ★ Умолчание — 0.5in: так печатает раннер WPT (wpt#40788; Blink
        // `StyleForPage` берёт поля из параметров печати), и эталоны это
        // ЗАШИВАЮТ: `monolithic-overflow-027-ref` 400vh = 8in = четыре листа
        // по 2in, `monolithic-overflow-030-ref` — `height: 1.5in` + квадрат
        // 0.5in на лист. `WPT_PAGE_MARGIN=0` — прежнее поведение.
        .unwrap_or(48.0);
    let mut margin = [default_margin; 4];
    let mut padding = [0.0f32; 4];
    // Стороны с `margin: auto` — их размер решается остатком (ниже).
    let mut auto_m = [false; 4];
    // Логическая сторона листа → физический индекс [верх, право, низ, лево]
    // по письму КОРНЯ (css-writing-modes-4 §6.2): `page-box-008`
    // (`html { writing-mode: vertical-rl }`) — inline-start сверху,
    // block-start справа; `page-box-009` — как у горизонтального корня.
    let (vertical, vertical_rl, rtl) = wm;
    let logical = move |block: bool, start: bool| -> usize {
        match (vertical, block) {
            (false, true) if start => 0,
            (false, true) => 2,
            (false, false) if start != rtl => 3,
            (false, false) => 1,
            (true, true) if start == vertical_rl => 1,
            (true, true) => 3,
            (true, false) if start != rtl => 0,
            (true, false) => 2,
        }
    };
    // `width`/`height` листа — размер PAGE AREA, не листа (css-page-3
    // §page-model); применяются после полей, см. ниже.
    let (mut explicit_w, mut explicit_h): (Option<f32>, Option<f32>) = (None, None);
    let mut bg: Option<Color> = None;
    // Контур по умолчанию — `currentColor`, у листа это чёрный.
    let mut outline = (
        0.0f32,
        0.0f32,
        Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        },
    );
    let mut border = (
        0.0f32,
        Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        },
    );
    // Длина: точки как есть; vw/vh — от ДЕФОЛТНОЙ страницы (по тестам
    // page-size-016/017); проценты полей — от ОБЪЯВЛЕННОГО размера
    // страницы по своей оси (page-margin-005: 10% от 300px = 30px),
    // поэтому размер считается первым проходом.
    // `em` — от кегля контекста страницы (css-page-3 §page-properties: «Values
    // in units of em … relative to the font associated with their context»);
    // кегль листа по умолчанию — 16 (`content-004`: `margin: 4em` = 64).
    let fs = decls
        .iter()
        .rev()
        .find(|(k, _)| k == "font-size")
        .and_then(|(_, v)| match Len::parse(v.trim()) {
            Some(Len::Px(p)) => Some(p),
            Some(Len::Em(k)) => Some(k * 16.0),
            Some(Len::Pct(k)) => Some(k * 16.0),
            _ => None,
        })
        .unwrap_or(16.0);
    let px_abs = |t: &str| -> Option<f32> {
        match Len::parse(t)? {
            Len::Px(v) => Some(v),
            Len::Em(k) => Some(k * fs),
            Len::Vw(k) => Some(k * 480.0),
            Len::Vh(k) => Some(k * 288.0),
            _ => None,
        }
    };
    for (k, v) in &decls {
        match k.as_str() {
            "size" => {
                let toks: Vec<&str> = v.split_whitespace().collect();
                let nums: Vec<f32> = toks.iter().filter_map(|t| px_abs(t)).collect();
                // Нулевой лист — начальное значение (csswg#8335;
                // `printing/zero-size-001-print`: «The used page size is the
                // initial value instead of the authored width and height of zero»).
                let nums: Vec<f32> = if nums.iter().any(|v| *v <= 0.0) { Vec::new() } else { nums };
                match nums.len() {
                    2 => {
                        w = nums[0];
                        h = nums[1];
                    }
                    1 => {
                        w = nums[0];
                        h = nums[0];
                    }
                    _ => {}
                }
                // Имена носителей (css-page-3 §page-size-prop, `<page-size>`:
                // «A5 — 148mm wide and 210 mm high» и т.д.) и ориентация:
                // `landscape` кладёт длинную сторону горизонтально, `portrait`
                // — вертикально (`page-size-011-print`: эталон пишет те же
                // листы в мм). Без имени ориентация поворачивает лист WPT.
                let mm = 96.0 / 25.4;
                for t in &toks {
                    let named = match t.to_ascii_lowercase().as_str() {
                        "a5" => Some((148.0 * mm, 210.0 * mm)),
                        "a4" => Some((210.0 * mm, 297.0 * mm)),
                        "a3" => Some((297.0 * mm, 420.0 * mm)),
                        "b5" => Some((176.0 * mm, 250.0 * mm)),
                        "b4" => Some((250.0 * mm, 353.0 * mm)),
                        "jis-b5" => Some((182.0 * mm, 257.0 * mm)),
                        "jis-b4" => Some((257.0 * mm, 364.0 * mm)),
                        "letter" => Some((8.5 * 96.0, 11.0 * 96.0)),
                        "legal" => Some((8.5 * 96.0, 14.0 * 96.0)),
                        "ledger" => Some((11.0 * 96.0, 17.0 * 96.0)),
                        _ => None,
                    };
                    if let Some((a, b)) = named {
                        w = a;
                        h = b;
                    }
                }
                for t in &toks {
                    match t.to_ascii_lowercase().as_str() {
                        "landscape" if w < h => std::mem::swap(&mut w, &mut h),
                        "portrait" if w > h => std::mem::swap(&mut w, &mut h),
                        _ => {}
                    }
                }
            }
            "width" => explicit_w = px_abs(v).or(explicit_w),
            "height" => explicit_h = px_abs(v).or(explicit_h),
            _ => {}
        }
    }
    let (pw, ph) = (w, h);
    let px_of = move |t: &str, axis_h: bool| -> Option<f32> {
        match Len::parse(t)? {
            Len::Px(v) => Some(v),
            Len::Em(k) => Some(k * fs),
            Len::Vw(k) => Some(k * 480.0),
            Len::Vh(k) => Some(k * 288.0),
            Len::Pct(k) => Some(k * if axis_h { ph } else { pw }),
            _ => None,
        }
    };
    for (k, v) in &decls {
        match k.as_str() {
            "margin" if v.trim() == "inherit" => {
                // `page-margin-006`: `margin: 13px; margin: inherit` → поля
                // корневого элемента (0.5in), не 13px.
                margin = root_margin;
            }
            "margin" => {
                let vals: Vec<&str> = v.split_whitespace().collect();
                let side = |i: usize| vals.get(i).copied().unwrap_or("0");
                let (a, b, c, d) = match vals.len() {
                    1 => (side(0), side(0), side(0), side(0)),
                    2 => (side(0), side(1), side(0), side(1)),
                    3 => (side(0), side(1), side(2), side(1)),
                    _ => (side(0), side(1), side(2), side(3)),
                };
                for (i, (slot, (t, vert))) in margin
                    .iter_mut()
                    .zip([(a, true), (b, false), (c, true), (d, false)])
                    .enumerate()
                {
                    auto_m[i] = t == "auto";
                    if auto_m[i] {
                        *slot = 0.0;
                    } else if let Some(px) = px_of(t, vert) {
                        *slot = px;
                    }
                }
            }
            "margin-top" | "margin-right" | "margin-bottom" | "margin-left" => {
                let i = match k.as_str() {
                    "margin-top" => 0,
                    "margin-right" => 1,
                    "margin-bottom" => 2,
                    _ => 3,
                };
                auto_m[i] = v.trim() == "auto";
                if auto_m[i] {
                    margin[i] = 0.0;
                } else if let Some(px) = px_of(v, i % 2 == 0) {
                    margin[i] = px;
                }
            }
            // Отступы листа. Проценты — по СВОЕЙ физической оси, как у полей
            // (css-page-3 §page-properties: «for right and left values,
            // percentages are relative to the width of the containing block;
            // for top and bottom values, … to the height»): `page-box-007`
            // `5% 20% 15% 40%` от 400x800 = 40/80/120/160.
            "padding" => {
                let vals: Vec<&str> = v.split_whitespace().collect();
                let side = |i: usize| vals.get(i).copied().unwrap_or("0");
                let (a, b, c, d) = match vals.len() {
                    1 => (side(0), side(0), side(0), side(0)),
                    2 => (side(0), side(1), side(0), side(1)),
                    3 => (side(0), side(1), side(2), side(1)),
                    _ => (side(0), side(1), side(2), side(3)),
                };
                for (slot, (t, vert)) in padding
                    .iter_mut()
                    .zip([(a, true), (b, false), (c, true), (d, false)])
                {
                    if let Some(px) = px_of(t, vert) {
                        *slot = px;
                    }
                }
            }
            "padding-top" => padding[0] = px_of(v, true).unwrap_or(padding[0]),
            "padding-right" => padding[1] = px_of(v, false).unwrap_or(padding[1]),
            "padding-bottom" => padding[2] = px_of(v, true).unwrap_or(padding[2]),
            "padding-left" => padding[3] = px_of(v, false).unwrap_or(padding[3]),
            // Логические поля и отступы (`margin-block-start` и родня): сторона
            // — по письму корня, доля — по физической оси этой стороны.
            k if (k.starts_with("margin-") || k.starts_with("padding-"))
                && (k.contains("-block-") || k.contains("-inline-"))
                && (k.ends_with("-start") || k.ends_with("-end")) =>
            {
                let i = logical(k.contains("-block-"), k.ends_with("-start"));
                let slot = if k.starts_with("padding-") {
                    &mut padding[i]
                } else {
                    &mut margin[i]
                };
                if let Some(px) = px_of(v, i % 2 == 0) {
                    *slot = px;
                }
            }
            "background" | "background-color" => {
                let first = v.split_whitespace().next().unwrap_or("");
                if let Some(c) = Color::parse(first) {
                    bg = Some(c);
                }
            }
            "outline" => {
                for t in v.split_whitespace() {
                    if let Some(px) = px_of(t, false) {
                        outline.0 = px;
                    } else if let Some(c) = Color::parse(t) {
                        outline.2 = c;
                    }
                }
            }
            "outline-width" => outline.0 = px_of(v, false).unwrap_or(outline.0),
            "outline-offset" => outline.1 = px_of(v, false).unwrap_or(outline.1),
            "outline-color" => outline.2 = Color::parse(v.trim()).unwrap_or(outline.2),
            "border" => {
                // Сокращение без толщины — `medium`, 3px (css-backgrounds-3
                // §border-width); `none`/`hidden` — без рамки
                // (`margin-boxes/auto-margins-001`: `@page { border: solid }`).
                let mut width: Option<f32> = None;
                let mut styled = false;
                for t in v.split_whitespace() {
                    match t.to_ascii_lowercase().as_str() {
                        "none" | "hidden" => {
                            width = Some(0.0);
                            styled = false;
                        }
                        "solid" | "dashed" | "dotted" | "double" | "groove" | "ridge" | "inset"
                        | "outset" => styled = true,
                        "thin" => width = Some(1.0),
                        "medium" => width = Some(3.0),
                        "thick" => width = Some(5.0),
                        _ => {
                            if let Some(px) = px_of(t, false) {
                                width = Some(px);
                            } else if let Some(c) = Color::parse(t) {
                                border.1 = c;
                            }
                        }
                    }
                }
                border.0 = match width {
                    Some(w) if styled => w,
                    None if styled => 3.0,
                    _ => 0.0,
                };
            }
            _ => {}
        }
    }
    // css-page-3 §page-model: при переопределении «instead of ignoring any
    // margins, the containing block is resized to coincide with the margin
    // edges of the page box» (Blink `ResolvePageBoxGeometry`);
    // `page-size-013`: size 500px, margin 50px, width 200px, height 300px →
    // лист 300x400 (эталон `size: 300px 400px; margin: 50px`).
    // `width`/`height` — контентная коробка листа, то есть page area: лист =
    // она + отступы + рамка + поля.
    // Ось с `auto`-полем: лист остаётся размером `size`, а `auto`-поля делят
    // остаток — он бывает ОТРИЦАТЕЛЬНЫМ, если коробка шире `size` (Blink
    // `ResolvePageBoxGeometry` → `ResolveAutoMargins`, csswg#8508;
    // `page-margin-auto-negative-print.tentative`: size 300, width 340,
    // margin auto → по −20 с каждой стороны). Ось без `auto` — прежнее
    // переопределение: лист подгоняется под поля (`page-size-013`).
    let fit = |size: f32, used: f32, a: bool, b: bool, ma: &mut f32, mb: &mut f32| -> f32 {
        match (a, b) {
            (true, true) => {
                *ma = (size - used) / 2.0;
                *mb = *ma;
                size
            }
            (true, false) => {
                *ma = size - used - *mb;
                size
            }
            (false, true) => {
                *mb = size - used - *ma;
                size
            }
            (false, false) => used + *ma + *mb,
        }
    };
    let [mt, mr, mb, ml] = &mut margin;
    if let Some(x) = explicit_w {
        let used = x + border.0 * 2.0 + padding[1] + padding[3];
        w = fit(w, used, auto_m[3], auto_m[1], ml, mr);
    }
    if let Some(y) = explicit_h {
        let used = y + border.0 * 2.0 + padding[0] + padding[2];
        h = fit(h, used, auto_m[0], auto_m[2], mt, mb);
    }
    let area = (
        (w - margin[1] - margin[3] - border.0 * 2.0 - padding[1] - padding[3]).max(0.0),
        (h - margin[0] - margin[2] - border.0 * 2.0 - padding[0] - padding[2]).max(0.0),
    );
    // `visibility: hidden` у листа прячет его украшения, не геометрию
    // (`page-visibility-hidden-001-print`: красная рамка листа не видна,
    // содержимое на месте).
    if decls
        .iter()
        .rev()
        .find(|(k, _)| k == "visibility")
        .is_some_and(|(_, v)| matches!(v.trim(), "hidden" | "collapse"))
    {
        border.1.a = 0.0;
        bg = None;
        outline.0 = 0.0;
    }
    PageBox {
        size: (w, h),
        margin,
        padding,
        area,
        bg,
        border,
        outline,
    }
}

/// Снимок клиентской области окна: массив байт BGRA и его размеры.
fn capture(hwnd: isize) -> Option<(u32, u32, Vec<u8>)> {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Gdi::{
        BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleBitmap, CreateCompatibleDC,
        DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDC, GetDIBits, ReleaseDC, SelectObject,
    };
    use windows::Win32::Storage::Xps::{PRINT_WINDOW_FLAGS, PrintWindow};
    use windows::Win32::UI::WindowsAndMessaging::GetClientRect;

    let hwnd = HWND(hwnd as *mut _);
    unsafe {
        let mut rect = Default::default();
        GetClientRect(hwnd, &mut rect).ok()?;
        let (w, h) = (
            (rect.right - rect.left) as u32,
            (rect.bottom - rect.top) as u32,
        );
        if w == 0 || h == 0 {
            return None;
        }
        let screen = GetDC(None);
        let dc = CreateCompatibleDC(Some(screen));
        let bitmap = CreateCompatibleBitmap(screen, w as i32, h as i32);
        let old = SelectObject(dc, bitmap.into());
        // Флаг 3 = PW_RENDERFULLCONTENT: без него аппаратно нарисованное окно
        // снимается пустым.
        let ok = PrintWindow(hwnd, dc, PRINT_WINDOW_FLAGS(3)).as_bool();
        let mut info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w as i32,
                // Отрицательная высота — строки сверху вниз.
                biHeight: -(h as i32),
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut pixels = vec![0u8; (w * h * 4) as usize];
        let lines = GetDIBits(
            dc,
            bitmap,
            0,
            h,
            Some(pixels.as_mut_ptr() as *mut _),
            &mut info,
            DIB_RGB_COLORS,
        );
        SelectObject(dc, old);
        let _ = DeleteObject(bitmap.into());
        let _ = DeleteDC(dc);
        ReleaseDC(None, screen);
        (ok && lines > 0).then_some((w, h, pixels))
    }
}

/// Доля точек, разошедшихся сильнее допуска.
/// Выгрузить снимок пары в PNG рядом с отчётом.
///
/// Число расхождения говорит только «не сошлось». Что именно разъехалось —
/// видно лишь на картинке, и без неё правка идёт вслепую. Пишется по просьбе
/// (`WPT_DUMP=1`) и только для разошедшихся пар: на полном прогоне это тысячи
/// файлов.
fn dump(name: &str, shot: &(u32, u32, Vec<u8>)) {
    let dir = std::path::Path::new("target/wpt-shots");
    if std::fs::create_dir_all(dir).is_err() {
        return;
    }
    let Ok(file) = std::fs::File::create(dir.join(format!("{name}.png"))) else {
        return;
    };
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), shot.0, shot.1);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let Ok(mut writer) = encoder.write_header() else {
        return;
    };
    // Снимок приходит от системы в порядке BGRA, PNG ждёт RGBA. Заодно
    // выставляется полная непрозрачность: у слоя окна альфа нулевая, и без
    // этого картинка открывается пустой.
    let mut rgba = shot.2.clone();
    for px in rgba.chunks_exact_mut(4) {
        px.swap(0, 2);
        px[3] = 255;
    }
    let _ = writer.write_image_data(&rgba);
}

/// Своя рабочая память в мегабайтах — второй след деградации стенда рядом со
/// временем пары. Растёт вместе с ним, если дело в утечке ресурсов окна.
fn rss_mb() -> u64 {
    #[cfg(windows)]
    {
        use windows::Win32::System::ProcessStatus::{
            GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS,
        };
        use windows::Win32::System::Threading::GetCurrentProcess;
        let mut counters = PROCESS_MEMORY_COUNTERS {
            cb: std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
            ..Default::default()
        };
        unsafe {
            if GetProcessMemoryInfo(GetCurrentProcess(), &mut counters, counters.cb).is_ok() {
                return (counters.WorkingSetSize / (1024 * 1024)) as u64;
            }
        }
        0
    }
    #[cfg(not(windows))]
    0
}

/// Сколько точек снимка ЯВНО красные.
///
/// Второй признак провала, не зависящий от эталона. Сравнение снимков слепо
/// там, где движок ломает обе стороны одинаково: пара сходится в ноль, а тест
/// провален. Но добрая половина набора прямо пишет «no red», и рисует красное
/// подложкой под зелёным — если красное видно, тест провален независимо от
/// совпадения. Порог по каналам взят с запасом, чтобы не считать красным
/// сглаживание кромок тёмного текста.
fn red_pixels(shot: &[u8]) -> usize {
    shot.chunks_exact(4)
        .filter(|p| p[2] > 150 && p[1] < 100 && p[0] < 100)
        .count()
}

/// Ниже этого числа нарисованных точек сторона считается ПУСТОЙ.
///
/// ★ Порог намеренно низкий. Прежняя тысяча объявляла пустыми целые семейства,
/// где обе стороны рисовали ОДИНАКОВО и немного (892/892, 675/675) — страница
/// из десятка знаков Ahem это норма, а не пустота. Пустота — это когда рисовать
/// нечего вовсе; несравнимость ловится отдельно, по РАЗНИЦЕ чернил сторон.
const INK_MIN: usize = 40;

/// Разделитель между страницами. Он РОВНЫЙ (все точки одного цвета) — по этому
/// признаку стенд понимает, что прошлая страница уже стёрта. Цвет заметный, а
/// не белый: окно стенда висит на экране десятками минут, и белизна неотличима
/// от зависшего стенда — на неё уже дважды жаловались.
const SEPARATOR: &str = "<body style=\"background:#cfd8e8;margin:0\"></body>";

/// Допуск, объявленный САМИМ тестом: `<meta name="fuzzy"
/// content="maxDifference=0-2;totalPixels=0-1200">`. Это часть протокола
/// reftest WPT, а не поблажка стенда: тест знает, что расходится с эталоном
/// на антиалиасинге, и называет верхнюю границу расхождения. Возвращается
/// наибольшее допустимое ЧИСЛО разошедшихся точек.
fn fuzzy_pixels(source: &str) -> usize {
    let lower = source.to_ascii_lowercase();
    let Some(at) = lower
        .find("name=\"fuzzy\"")
        .or_else(|| lower.find("name=fuzzy"))
    else {
        return 0;
    };
    let tail = &lower[at..];
    let Some(c) = tail.find("totalpixels=") else {
        return 0;
    };
    let value = &tail[c + "totalpixels=".len()..];
    let value = &value[..value
        .find(|ch: char| !ch.is_ascii_digit() && ch != '-')
        .unwrap_or(value.len())];
    // Запись — диапазон `0-1200` или число; допуск — верхняя граница.
    value
        .rsplit('-')
        .next()
        .and_then(|n| n.parse().ok())
        .unwrap_or(0)
}

fn diff(a: &[u8], b: &[u8]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 100.0;
    }
    let mut bad = 0usize;
    for (x, y) in a.chunks_exact(4).zip(b.chunks_exact(4)) {
        // Порог тот же, что у стенда против Chrome: сглаживание кромок — не
        // расхождение раскладки.
        if x[..3].iter().zip(&y[..3]).any(|(p, q)| p.abs_diff(*q) > 40) {
            bad += 1;
        }
    }
    bad as f32 * 400.0 / a.len() as f32
}

/// Разрешить ссылки документа: адреса картинок — в абсолютные пути, внешние
/// таблицы стилей — внутрь разметки.
///
/// Своего разрешения адресов у движка нет и быть не должно: он не знает, откуда
/// взялся документ. Знает это тот, кто документ открыл, — здесь стенд. Без
/// этого шага половина эталонов WPT рисует подпись `alt` вместо картинки, а
/// правила из `<link rel=stylesheet>` пропадают целиком.
/// Сколько точек кадра отличается от разделителя — «чернила» страницы.
///
/// Белый экран — это ОШИБКА, а не совпадение: пара, где обе стороны почти
/// ничего не нарисовали, сходится с нулевым расхождением и уходит в зелёные,
/// хотя сравнивать там нечего.
fn ink(shot: &[u8], blank: Option<&Vec<u8>>) -> usize {
    let Some(blank) = blank else {
        return usize::MAX;
    };
    if blank.len() != shot.len() {
        return usize::MAX;
    }
    shot.chunks_exact(4)
        .zip(blank.chunks_exact(4))
        .filter(|(x, y)| x[..3].iter().zip(&y[..3]).any(|(p, q)| p.abs_diff(*q) > 40))
        .count()
}

/// Ровный ли кадр — все точки одного цвета. Таким выходит пустая страница.
fn flat(buf: &[u8]) -> bool {
    let Some(first) = buf.get(..4) else {
        return true;
    };
    buf.chunks_exact(4).all(|px| px == first)
}

/// Перебазировать `url(...)` содержимого css-файла на его папку: после
/// инлайна в документ относительные адреса считались бы от папки ТЕСТА.
fn rebase_css_urls(css: &str, base: &std::path::Path) -> String {
    let mut out = String::with_capacity(css.len());
    let mut rest = css;
    while let Some((at, head)) = find_url(rest) {
        out.push_str(&rest[..at + head]);
        rest = &rest[at + head..];
        let Some(end) = rest.find(')') else { break };
        let raw = rest[..end].trim();
        let bare = raw.trim_matches(|c| c == '\'' || c == '"');
        if bare.starts_with("data:") || bare.contains("://") || bare.starts_with('/') {
            out.push_str(raw);
        } else {
            let abs = base.join(bare).display().to_string().replace('\\', "/");
            out.push('"');
            out.push_str(&abs);
            out.push('"');
        }
        out.push(')');
        rest = &rest[end + 1..];
    }
    out.push_str(rest);
    out
}

fn resolve_links(html: &str, path: &str) -> String {
    let dir = std::path::Path::new(path)
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_default();
    // Корень набора: от него считаются адреса, начинающиеся со слэша.
    let root = dir
        .ancestors()
        .find(|p| p.join("css").is_dir() && p.join("html").is_dir())
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| dir.clone());
    let resolve = |href: &str| -> Option<std::path::PathBuf> {
        if href.starts_with("data:") || href.contains("://") {
            return None;
        }
        let file = match href.strip_prefix('/') {
            Some(rest) => root.join(rest),
            None => dir.join(href),
        };
        file.exists().then_some(file)
    };

    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(at) = rest.find('<') {
        out.push_str(&rest[..at]);
        let tail = &rest[at..];
        let Some(end) = tail.find('>') else {
            out.push_str(tail);
            return out;
        };
        let tag = &tail[..=end];
        let lower = tag.to_ascii_lowercase();
        if lower.starts_with("<img")
            || lower.starts_with("<link")
            || lower.starts_with("<iframe")
            || lower.starts_with("<embed")
            || lower.starts_with("<object")
            || lower.starts_with("<video")
        {
            let attr = if lower.starts_with("<link") {
                "href"
            } else if lower.starts_with("<object") {
                "data"
            } else if lower.starts_with("<video") {
                "poster"
            } else {
                "src"
            };
            match attr_value(tag, attr).and_then(|v| resolve(&v).map(|p| (v, p))) {
                Some((href, file))
                    if lower.starts_with("<img")
                        || lower.starts_with("<iframe")
                        || lower.starts_with("<embed")
                        || lower.starts_with("<object")
                        || lower.starts_with("<video") =>
                {
                    // Разделитель пути в адресе — прямая косая даже на
                    // Windows: с обратной загрузчик картинок молча ничего не
                    // показывал, и эталоны из одних картинок выходили пустой
                    // страницей.
                    let uri = format!("file:///{}", file.display()).replace('\\', "/");
                    out.push_str(&tag.replace(&href, &uri));
                }
                Some((_, file)) if lower.contains("stylesheet") => {
                    let css = read_stylesheet(&file);
                    // Адреса внутри ПОДКЛЮЧЁННОГО файла считаются от ЕГО
                    // папки: после вставки в документ база сместилась бы на
                    // папку теста, и `url(WidthTest-Regular.otf)` из
                    // support/width-test.css терял шрифт (compression-004).
                    let css = match file.parent() {
                        Some(base) => rebase_css_urls(&css, base),
                        None => css,
                    };
                    out.push_str("<style>");
                    out.push_str(&css);
                    out.push_str("</style>");
                }
                _ => out.push_str(tag),
            }
        } else {
            out.push_str(tag);
        }
        rest = &tail[end + 1..];
    }
    out.push_str(rest);
    // ПРОБОВАЛИ И ОТКАТИЛИ ДВАЖДЫ: подставлять `@import "…";` в `<style>`
    // содержимым файла. Первый заход искал по всему документу и рвал
    // разметку (css-text 992 → 930, шестьдесят две пустые страницы).
    // Второй разбирал только содержимое `<style>` — работает, но счёт
    // 992 → 991: `letter-spacing-206` зелёным не стал, а `-201`
    // покраснел. Причина: без подстановки ОБЕ стороны пары набирались
    // подменой шрифта и сходились; с ней Ahem получает только та
    // сторона, где `@import` есть. Возвращать вместе с разбором пар.
    // Адреса внутри стилей — `url(...)` у `@font-face` и фонов: их разрешаем
    // отдельным проходом, потому что в тегах они не лежат.
    let mut with_urls = String::with_capacity(out.len());
    let mut tail = out.as_str();
    // Имя записи регистронезависимо (§3.3): `URL(` и `Url(` — та же запись.
    while let Some((at, head)) = find_url(tail) {
        with_urls.push_str(&tail[..at + head]);
        let rest = &tail[at + head..];
        // Незакрытая запись живёт до конца СВОЕГО стиля — его закрывает
        // EOF таблицы (§4.2), а не конец документа (`uri-017`). Скобка,
        // найденная уже за `</style`, — из чужой разметки, не наша.
        let (close, after_len, closed) = match rest.find(')') {
            Some(close) if !rest[..close].to_ascii_lowercase().contains("</style") => {
                (close, close + 1, true)
            }
            _ => {
                let end = rest
                    .to_ascii_lowercase()
                    .find("</style")
                    .unwrap_or(rest.len());
                (end, end, false)
            }
        };
        let raw = rest[..close].trim();
        let bare = raw.trim_matches(|c| c == '\'' || c == '"');
        // Проценты раскодируются: в адресе они стоят вместо знаков, которые в
        // имени файла записаны как есть (`%27green%20block.png` — это файл
        // `'green block.png`). Браузер раскодирует их при обращении к файлу,
        // и без этого путь просто не находится (`uri-004`).
        // Экранирование снимается ДО поиска файла: в адресе оно записывает
        // знаки, которые в имени файла стоят как есть (`support/\\'green\\ block.png`
        // — это файл `'green block.png`). Стенд ищет файл так же, как его
        // нашёл бы браузер (`uri-005`).
        let bare = kamin_html::css::unescape(bare);
        let decoded = percent_decode(&bare);
        // Адрес с фрагментом (`file.svg#mask`): файл существует без хвоста —
        // резолвим базу, хвост приклеиваем обратно
        // (mask-image-url-remote-mask).
        let frag_split = |s: &str| -> (String, Option<String>) {
            match s.split_once('#') {
                Some((b, f)) if !b.is_empty() => (b.to_string(), Some(f.to_string())),
                _ => (s.to_string(), None),
            }
        };
        let (dec_base, dec_frag) = frag_split(&decoded);
        let (bare_base, bare_frag) = frag_split(&bare);
        let resolved = resolve(&decoded)
            .map(|f| (f, None))
            .or_else(|| resolve(&bare).map(|f| (f, None)))
            .or_else(|| {
                dec_frag
                    .as_ref()
                    .and_then(|fr| resolve(&dec_base).map(|f| (f, Some(fr.clone()))))
            })
            .or_else(|| {
                bare_frag
                    .as_ref()
                    .and_then(|fr| resolve(&bare_base).map(|f| (f, Some(fr.clone()))))
            });
        match resolved {
            // Кавычки ставятся ТОЛЬКО когда без них нельзя: адрес попадает и
            // в атрибут `style="…"`, а двойная кавычка внутри него обрывает
            // сам атрибут — правило теряется целиком вместе с картинкой
            // (`background-size-near-zero-png` и вся родня с оформлением по
            // месту). В имени файла из набора встречается апостроф
            // (`'green block.png` из `uri-004`), поэтому запасные кавычки —
            // двойные: в пути Windows их не бывает.
            Some((file, frag)) => {
                // Разделитель — ПРЯМАЯ косая: обратная в записи адреса
                // означает экранирование, и путь Windows терял её вместе со
                // следующим знаком (`C:\Users` превращалось в `C:Users`).
                let mut path = file.display().to_string().replace('\\', "/");
                if let Some(fr) = frag {
                    path.push('#');
                    path.push_str(&fr);
                }
                let plain = !path.contains([' ', '\'', '"', '(', ')', ',', '\t']);
                if plain {
                    with_urls.push_str(&path);
                } else {
                    with_urls.push_str(&format!("\"{path}\""));
                }
            }
            None => with_urls.push_str(raw),
        }
        if closed {
            with_urls.push(')');
        }
        tail = &rest[after_len..];
    }
    with_urls.push_str(tail);
    // Подключения разворачиваются ПОСЛЕ разбора адресов: к этому мигу
    // `@import url(...)` уже несёт разрешённый путь к файлу.
    let with_urls = expand_style_imports(&with_urls, &dir);
    if std::env::var("WPT_HTML_DUMP").is_ok() {
        use std::io::Write as _;
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open("target/dbg-page.html")
        {
            let _ = writeln!(f, "<!-- ==== page ==== -->");
            let _ = f.write_all(with_urls.as_bytes());
        }
    }
    with_urls
}

/// Развернуть `@import url(...)` ВНУТРИ `<style>` содержимым файла.
///
/// Страницу до движка доводит стенд, и подключений он не делает сам. Набор
/// WOFF2 подключает свои опорные шрифты ТОЛЬКО через `@import`, и без
/// разворачивания обе стороны каждой пары рисуются системной подменой.
///
/// Разворачивается лишь то, что лежит между `<style>` и `</style>`: первый
/// заход правил весь документ и рвал разметку (css-text 992 → 930).
/// Глубина ограничена: кольцо подключений иначе вешает стенд насмерть.
fn expand_style_imports(html: &str, dir: &std::path::Path) -> String {
    // Пролог таблицы (css-cascade-5 §6.3): `@import` действует, только пока
    // перед ним нет ничего, кроме `@charset`, `@layer a, b;` и других
    // `@import`. Подключение после правила игнорируется (`cascade-import-001`,
    // `at-rule-013`).
    fn prologue_only(head: &str) -> bool {
        let mut text = String::new();
        let mut rest = head;
        while let Some(a) = rest.find("/*") {
            text.push_str(&rest[..a]);
            rest = rest[a + 2..].find("*/").map_or("", |b| &rest[a + 2 + b + 2..]);
        }
        text.push_str(rest);
        let text = text
            .replace("<!--", "")
            .replace("-->", "")
            .replace("<![CDATA[", "")
            .replace("]]>", "");
        !text.contains('{')
            && text.split(';').all(|part| {
                let t = part.trim().to_ascii_lowercase();
                t.is_empty() || t.starts_with("@charset") || t.starts_with("@layer")
            })
    }
    fn expand(css: &str, base: &std::path::Path, depth: usize) -> String {
        if depth == 0 {
            return css.to_string();
        }
        let mut out = String::with_capacity(css.len());
        let mut rest = css;
        let mut prologue = true;
        while let Some(at) = rest.to_ascii_lowercase().find("@import") {
            let (head, tail) = rest.split_at(at);
            out.push_str(head);
            prologue = prologue && prologue_only(head);
            let Some(end) = tail.find(';') else {
                out.push_str(tail);
                return out;
            };
            let rule = tail["@import".len()..end].trim_start();
            rest = &tail[end + 1..];
            if !prologue {
                continue;
            }
            // Адрес — `url(…)` или строка (§6.3); всё после него — условия
            // подключения: `layer(…)`, `supports(…)` и список медиазапросов.
            let low = rule.to_ascii_lowercase();
            let (name, cond) = if low.starts_with("url(") {
                let Some(b) = rule.find(')') else { continue };
                (rule[4..b].trim().trim_matches(|c| c == '\'' || c == '"'), &rule[b + 1..])
            } else if let Some(q) = rule.chars().next().filter(|c| *c == '"' || *c == '\'') {
                let Some(b) = rule[1..].find(q) else { continue };
                (&rule[1..1 + b], &rule[b + 2..])
            } else {
                continue;
            };
            let name = name.trim_start_matches("file:///");
            let file = if std::path::Path::new(name).is_absolute() {
                std::path::PathBuf::from(name)
            } else {
                base.join(kamin_html::css::unescape(name))
            };
            if !file.exists() {
                continue;
            }
            let css = read_stylesheet(&file);
            // Адреса подключённого файла считаются от ЕГО папки — тем же
            // приёмом, что и у `<link rel=stylesheet>`.
            let inner_base = file.parent().unwrap_or(base).to_path_buf();
            let css = rebase_css_urls(&css, &inner_base);
            let mut body = expand(&css, &inner_base, depth - 1);
            // Условия разворачиваются обёртками изнутри наружу: медиазапрос,
            // затем `supports()`, затем `layer()` — той же семантикой, что у
            // одноимённых групп (`import-conditional-001/002`).
            let mut cond = cond.trim().to_string();
            let mut layer: Option<String> = None;
            let mut supports: Option<String> = None;
            loop {
                let lc = cond.to_ascii_lowercase();
                if lc.starts_with("layer(") {
                    let Some(b) = cond.find(')') else { break };
                    layer = Some(cond[6..b].trim().to_string());
                    cond = cond[b + 1..].trim().to_string();
                } else if lc == "layer" || lc.starts_with("layer ") {
                    layer = Some(String::new());
                    cond = cond[5..].trim().to_string();
                } else if lc.starts_with("supports(") {
                    let mut depth = 0i32;
                    let mut close = None;
                    for (i, ch) in cond.char_indices() {
                        match ch {
                            '(' => depth += 1,
                            ')' => {
                                depth -= 1;
                                if depth == 0 {
                                    close = Some(i);
                                    break;
                                }
                            }
                            _ => {}
                        }
                    }
                    let Some(b) = close else { break };
                    let arg = cond[9..b].trim();
                    supports = Some(if arg.starts_with('(') {
                        arg.to_string()
                    } else {
                        format!("({arg})")
                    });
                    cond = cond[b + 1..].trim().to_string();
                } else {
                    break;
                }
            }
            if !cond.is_empty() {
                body = format!("@media {cond} {{\n{body}\n}}");
            }
            if let Some(sup) = supports {
                body = format!("@supports {sup} {{\n{body}\n}}");
            }
            if let Some(name) = layer {
                body = format!("@layer {name} {{\n{body}\n}}");
            }
            out.push_str(&body);
        }
        out.push_str(rest);
        out
    }
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(at) = rest.to_ascii_lowercase().find("<style") {
        let Some(open) = rest[at..].find('>') else { break };
        let body = at + open + 1;
        out.push_str(&rest[..body]);
        let Some(close) = rest[body..].to_ascii_lowercase().find("</style") else {
            break;
        };
        out.push_str(&expand(&rest[body..body + close], dir, 4));
        rest = &rest[body + close..];
    }
    out.push_str(rest);
    out
}

/// Ближайшая запись `url(` без учёта регистра, включая экранированные формы
/// имени (`U\\r\\4c (` — то же `url(`, §4.1.3): даёт начало и длину головы
/// вместе с открывающей скобкой.
fn find_url(text: &str) -> Option<(usize, usize)> {
    let bytes = text.as_bytes();
    (0..bytes.len()).find_map(|i| {
        if !(bytes[i] == b'\\' || bytes[i].eq_ignore_ascii_case(&b'u')) {
            return None;
        }
        url_head(&text[i..]).map(|len| (i, len))
    })
}

/// Длина головы `url(` (до скобки включительно), если запись начинается здесь.
///
/// Имя записи может нести экранирование: hex-код с необязательным
/// пробелом-терминатором либо один буквальный знак. Пробел-терминатор — часть
/// эскейпа, отдельных пробелов между именем и скобкой не бывает.
fn url_head(text: &str) -> Option<usize> {
    let mut at = 0usize;
    let mut name = String::new();
    while at < text.len() && name.len() < 3 {
        let ch = text[at..].chars().next()?;
        match ch {
            '\\' => {
                let tail = &text[at + 1..];
                let mut digits = 0usize;
                let mut end = 0usize;
                for (i, c) in tail.char_indices() {
                    if digits < 6 && c.is_ascii_hexdigit() {
                        digits += 1;
                        end = i + c.len_utf8();
                        continue;
                    }
                    if digits > 0 && c.is_whitespace() {
                        end = i + c.len_utf8();
                    }
                    break;
                }
                if digits > 0 {
                    let code = u32::from_str_radix(tail[..end].trim(), 16).ok()?;
                    name.push(char::from_u32(code)?);
                    at += 1 + end;
                } else {
                    let c = tail.chars().next()?;
                    name.push(c);
                    at += 1 + c.len_utf8();
                }
            }
            c if c.is_ascii_alphabetic() => {
                name.push(c);
                at += c.len_utf8();
            }
            _ => return None,
        }
    }
    if !name.eq_ignore_ascii_case("url") {
        return None;
    }
    (text.as_bytes().get(at) == Some(&b'(')).then_some(at + 1)
}

/// Адрес с раскодированными процентами.
fn percent_decode(raw: &str) -> String {
    if !raw.contains('%') {
        return raw.to_string();
    }
    let bytes = raw.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut at = 0usize;
    while at < bytes.len() {
        if bytes[at] == b'%' && at + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[at + 1..at + 3]).unwrap_or("");
            if let Ok(byte) = u8::from_str_radix(hex, 16) {
                out.push(byte);
                at += 3;
                continue;
            }
        }
        out.push(bytes[at]);
        at += 1;
    }
    String::from_utf8(out).unwrap_or_else(|_| raw.to_string())
}

/// Значение атрибута в теге — в кавычках или без.
///
/// Разбор идёт с учётом кавычек: имя атрибута ищется только ВНЕ значений,
/// иначе `alt="см. src=1"` подсовывает чужое значение.
fn attr_value(tag: &str, name: &str) -> Option<String> {
    let bytes = tag.as_bytes();
    let mut i = 0usize;
    let mut quote: Option<u8> = None;
    let mut word_start: Option<usize> = None;
    while i < bytes.len() {
        let c = bytes[i];
        if let Some(q) = quote {
            if c == q {
                quote = None;
            }
            i += 1;
            continue;
        }
        match c {
            b'"' | b'\'' => {
                quote = Some(c);
                word_start = None;
            }
            b'=' => {
                if let Some(start) = word_start.take()
                    && tag[start..i].trim().eq_ignore_ascii_case(name)
                {
                    let value = tag[i + 1..].trim_start();
                    let quoted = |q: char| value.strip_prefix(q).and_then(|v| v.split(q).next());
                    return quoted('"')
                        .or_else(|| quoted('\''))
                        .or_else(|| value.split([' ', '>', '/']).next())
                        .map(str::to_string);
                }
            }
            c if c.is_ascii_whitespace() => word_start = None,
            _ => {
                if word_start.is_none() {
                    word_start = Some(i);
                }
            }
        }
        i += 1;
    }
    None
}

// ★ ЗАМЕРЕНО И ОТКАЧЕНО (05.09): подставлять `@import` содержимым файла
// внутри `<style>`, чтобы набор WOFF2 подключал свои шрифты. Срез 2012 пар
// (WOFF2 + шрифты + текст): 1480 -> 1446, +0/-34. Все 34 потери — семьи
// `*-invalid-*`, `*-bad-*`, `blocks-*`, `tabledata-*`: они проверяют, что
// НЕГОДНЫЙ шрифт НЕ применяется, и с подстановкой мы начали его принимать
// (0.41 -> 4.29). Правильный порядок обратный: сперва научить `fonts.rs`
// отвергать негодный woff2, и только потом подставлять `@import`.
fn main() {
    // Паника В КАДРЕ не роняет процесс: оконный вызов Windows её глотает, и
    // на экране молча остаётся предыдущая страница — стенд считает её пустой.
    // След в файле — единственный способ увидеть такую панику.
    // Стенд запускают пачками и без человека у экрана: любое модальное окно
    // gpui («Failed to launch») повесило бы процесс намертво.
    unsafe { std::env::set_var("GPUI_NO_ERROR_DIALOG", "1") };
    std::panic::set_hook(Box::new(|info| {
        let _ = std::fs::write("target/wpt-panic.txt", format!("{info}"));
        eprintln!("{info}");
    }));
    let args: Vec<String> = std::env::args().collect();
    let list = args.get(1).cloned().unwrap_or_default();
    let w: f32 = args.get(2).and_then(|v| v.parse().ok()).unwrap_or(800.);
    let h: f32 = args.get(3).and_then(|v| v.parse().ok()).unwrap_or(600.);
    // `WPT_DUMP=1` — снимки разошедшихся пар, `WPT_DUMP=all` — всех подряд.
    // Второе нужно ручному разбору: там проверяется и то, что стенд счёл
    // сошедшимся (см. `scripts/wpt_review.py`).
    let dump_mode = std::env::var("WPT_DUMP").unwrap_or_default();
    let dumping = !dump_mode.is_empty();
    let dump_all = dump_mode == "all";
    let pairs: Vec<(String, String)> = std::fs::read_to_string(&list)
        .unwrap_or_default()
        .lines()
        .filter_map(|line| line.split_once('|'))
        // Хвост после ВТОРОЙ черты отрезается: списки часто нарезаются из
        // ОТЧЁТА, где третьим полем стоит вердикт, и он молча приклеивался к
        // пути эталона. Путь становился битым, страница показывалась пустой,
        // и пара выглядела сломанной по несуществующей причине.
        .map(|(a, b)| {
            let b = b.split('|').next().unwrap_or(b).trim();
            (a.trim().to_string(), b.to_string())
        })
        .collect();
    if pairs.is_empty() {
        eprintln!("пустой список пар: {list}");
        std::process::exit(1);
    }

    Application::new().run(move |cx| {
        // Ahem — служебный шрифт набора: все его буквы одинаковые чёрные
        // квадраты в кегль. На нём построены сотни тестов: фигуры сходятся
        // ровно потому, что метрики предсказуемы. Без него текст набирается
        // системным шрифтом, и «квадраты» разъезжаются.
        for name in ["Ahem.ttf", "AHEM____.TTF"] {
            let path = std::path::Path::new("vendor/wpt-parsing/fonts").join(name);
            if let Ok(bytes) = std::fs::read(&path) {
                let _ = cx.text_system().add_fonts(vec![bytes.into()]);
                break;
            }
        }
        // Щуп метрик ставится ПОСЛЕ регистрации: до неё `Ahem` ещё не найден,
        // и `ch` мерился бы по шрифту-подмене.
        kamin_html::metrics::use_text_system(cx.text_system().clone());
        // Свои шрифты страницы (`@font-face`): файл отдаётся системе, а её имя
        // семейства запоминается под тем, которым шрифт зовут в разметке.
        let fonts = cx.text_system().clone();
        kamin_html::fonts::install_loader(move |bytes| {
            // Имя — из name-таблицы файла: разность общего списка имён
            // пуста, когда система уже знает такое семейство, и алиас
            // терялся (FontWithFancyFeatures — весь кластер OpenType-фич).
            let named = kamin_html::fonts::sfnt_family(&bytes);
            let before: std::collections::HashSet<String> =
                fonts.all_font_names().into_iter().collect();
            fonts.add_fonts(vec![bytes.into()]).ok()?;
            named.or_else(|| {
                fonts
                    .all_font_names()
                    .into_iter()
                    .find(|name| !before.contains(name))
            })
        });
        let empty = Rc::new(Document::new("", BROWSER_CSS));
        let window = cx
            .open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds {
                        origin: point(px(40.), px(40.)),
                        size: size(px(w), px(h)),
                    })),
                    titlebar: Some(TitlebarOptions {
                        appears_transparent: true,
                        ..Default::default()
                    }),
                    window_decorations: Some(WindowDecorations::Client),
                    window_background: WindowBackgroundAppearance::Opaque,
                    ..Default::default()
                },
                |_, cx| -> Entity<Page> { cx.new(|_| Page { doc: empty }) },
            )
            .unwrap();
        // Фокус НЕ забираем: стенд идёт десятками минут, и всё это время его
        // окно висело поверх чужой работы — в паузах между страницами белое.
        // Снимок делает `PrintWindow` с флагом 3, ему передний план не нужен:
        // достаточно, чтобы окно не было СВЁРНУТО.

        cx.spawn(async move |cx| {
            // Первый кадр окна: до него снимок пустой.
            Timer::after(Duration::from_millis(900)).await;
            // Указатель окна нужен снимку: рисует его система, а не мы.
            let hwnd = cx
                .update_window(window.into(), |_, window, _| {
                    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
                    match window.window_handle().map(|h| h.as_raw()) {
                        Ok(RawWindowHandle::Win32(handle)) => handle.hwnd.get() as isize,
                        _ => 0,
                    }
                })
                .unwrap_or(0);
            // Свой файл отчёта на шард: параллельные прогоны не бьются за один путь.
            let report_path =
                std::env::var("WPT_REPORT").unwrap_or_else(|_| "target/wpt-report.txt".into());
            let mut report = String::new();
            // Показать страницу и дождаться ЕЁ кадра.
            //
            // Снимок делает система, а рисует окно по своему расписанию:
            // сна на глазок не хватает, и тяжёлая страница отдаёт кадр
            // ПРЕДЫДУЩЕЙ. По числам это выглядит как совпадение (ноль
            // расхождения) или как случайные скачки между прогонами.
            // Поэтому кадр опрашивается, пока не сменится и не устоится.
            // Чем кончился опрос кадра у каждого показа пары: `s` — кадр
            // сменился и устоялся, `b` — страница осталась равной разделителю,
            // `o` — запас опроса вышел, кадр есть, `n` — окно за весь запас не
            // отдало НИ ОДНОГО кадра. Без этого «пара идёт 27 секунд» и «пара
            // пустая» — два разных наблюдения без связи между ними.
            let trace = Rc::new(std::cell::RefCell::new(String::new()));
            let note = trace.clone();
            let mut show = async |html: String, prev: Option<Vec<u8>>, want_flat: bool| {
                let _ = cx.update_window(window.into(), |view, window, cx| {
                    if let Ok(page) = view.downcast::<Page>() {
                        page.update(cx, |page, cx| {
                            page.doc = Rc::new(Document::new(&html, BROWSER_CSS));
                            cx.notify();
                        });
                    }
                    window.refresh();
                });
                let lower = html.to_ascii_lowercase();
                let has_image = lower.contains("<img");
                // Страница с ДВИЖЕНИЕМ устаивается ложно: медленная анимация
                // даёт два одинаковых кадра подряд просто потому, что за 16 мс
                // картинка не изменилась на целую точку. Такой странице нужен
                // куда более длинный признак покоя — иначе одна и та же пара
                // скачет между прогонами (`css-flexbox-height-animation-stretch`:
                // то 1.13 %, то 0.03 %).
                let has_anim = lower.contains("animation") || lower.contains("transition");
                let mut last: Option<(u32, u32, Vec<u8>)> = None;
                let mut same = 0u32;
                let mut steady = 0u32;
                // Запас опроса: тяжёлой странице нужно время устояться, а лишнего
                // ожидания на лёгких не будет — цикл выходит по первому же
                // устоявшемуся кадру. Прежние 120 шагов (около двух секунд)
                // такие страницы обрезали, и одна и та же пара скакала между
                // прогонами на 15% расхождения.
                for step in 0..400u32 {
                    Timer::after(Duration::from_millis(16)).await;
                    // Окно ИНОГДА не перерисовывается после подмены документа:
                    // экран продолжает показывать разделитель, и страница
                    // числится пустой (`column-auto-repeat-auto-001`: эталон
                    // одиночно рисуется, в паре — нет; след `sssb`).
                    // Напоминание раз в полсекунды выводит его из этого
                    // состояния; на здоровых показах до него не доходит —
                    // кадр устаивается раньше.
                    if step % 30 == 29 {
                        let _ = cx.update_window(window.into(), |view, window, cx| {
                            if let Ok(page) = view.downcast::<Page>() {
                                page.update(cx, |_, cx| cx.notify());
                            }
                            window.refresh();
                        });
                    }
                    let shot = capture(hwnd);
                    let Some(now) = shot else { continue };
                    let changed = prev.as_ref().is_none_or(|before| *before != now.2);
                    let settled = last.as_ref().is_some_and(|before| before.2 == now.2);
                    // ПРОБОВАЛИ И ОТКАТИЛИ: требовать ТРИ одинаковых кадра
                    // подряд вместо двух — ради устойчивости чисел. Вышло
                    // наоборот: тяжёлая страница не успевает набрать три за
                    // отведённый опрос, снимок берётся последним и уходит
                    // недорисованным (css-grid 386 → 363, и разброс между
                    // двумя одинаковыми прогонами вырос до 18 пар). Лечится
                    // не строгостью признака, а ЗАПАСОМ ВРЕМЕНИ.
                    // Картинка приходит ПОЗЖЕ первого устоявшегося кадра:
                    // файл читается и раскодируется вне кадра, и страница
                    // успевает «устояться» без неё. На странице с `<img>`
                    // ждём подряд ЧЕТЫРЕ одинаковых кадра вместо двух —
                    // иначе одна и та же пара скачет между прогонами
                    // (`wm-propagation-body-032`: то 1.01 %, то 2.90 %, и
                    // картинки то есть, то нет).
                    let need = if has_anim {
                        30
                    } else if has_image {
                        16
                    } else {
                        4
                    };
                    steady = if settled { steady + 1 } else { 0 };
                    // Разделителю мало «устоялся»: он обязан быть РОВНЫМ.
                    // Иначе на экране остаётся кадр прошлой страницы, он и
                    // уходит в `blank`, а дальше от него считаются и «сменился»,
                    // и чернила — то есть следующая страница выглядит пустой
                    // (`css-position/multicol/static-position/*`).
                    if changed && steady + 1 >= need && (!want_flat || flat(&now.2)) {
                        note.borrow_mut().push('s');
                        return Some(now);
                    }
                    // Страница, которая НЕ нарисовалась, равна разделителю, и
                    // «сменился» на ней не наступает никогда — стенд вырабатывал
                    // весь запас опроса. На экране это долгая белизна, в
                    // числах — по 12 секунд на пару. Устоявшийся неизменный
                    // кадр принимается как ответ: он и есть итог, просто пустой.
                    // Ранний выход — ТОЛЬКО для кадра, равного разделителю:
                    // это и есть «страница не нарисовалась», ждать больше
                    // нечего. Принимать по устойчивости любой кадр нельзя —
                    // тяжёлая страница успевает застыть на полпути, и в
                    // сравнение уходит недорисованное (замерено: css-position
                    // просел 49 → 22 на одних таблицах).
                    let blank_now = prev.as_ref().is_some_and(|before| *before == now.2);
                    same = if settled && blank_now { same + 1 } else { 0 };
                    // Порог намеренно высокий (около двух секунд): тяжёлая
                    // страница даёт первый кадр далеко не сразу, и прежние
                    // восемь шагов (130 мс) объявляли пустой любую такую
                    // страницу. Ценой полутора секунд на действительно пустой
                    // странице покупается верный вердикт на тяжёлой
                    // (`css-position/multicol/static-position/*`: все 16 пар
                    // выходили пустыми, а по одной рисуются).
                    if same >= 120 {
                        note.borrow_mut().push('b');
                        return Some(now);
                    }
                    last = Some(now);
                }
                note.borrow_mut()
                    .push(if last.is_some() { 'o' } else { 'n' });
                last
            };
            // Время на пару: страница, у которой один кадр считается
            // секундами, выглядит на экране долгой пустотой и неотличима от
            // зависшего стенда. В отчёт оно не идёт (там сравниваются числа
            // расхождения), зато сразу видно, что тормозит.
            let mut slow: Vec<(u128, String)> = vec![];
            let mut timing = String::new();
            let mut timing_lines = 0usize;
            for (test, reference) in &pairs {
                let started = std::time::Instant::now();
                // Тест, которому нужен JavaScript, стенд исполнить не может.
                // `<meta name="variant">` применяется скриптом
                // (`support/variant-class.js` читает `location.search` и вешает
                // класс на `<html>`); без класса тест и эталон рисуют дефолт, и
                // пара сходится, не проверив ничего (`dominant-baseline-auto`:
                // снимки побайтово равны). Хуже: `text-box-trim-start-001` был
                // 0.00, пока трима не было, и стал 25.62, когда трим появился —
                // эталон без класса остаётся нетримленным. Такая пара — вне
                // цели, а не зелёная и не красная; рисовать её незачем.
                let head = std::fs::read_to_string(test).unwrap_or_default();
                let head_lower = head.to_ascii_lowercase();
                let out_of_scope = if head_lower.contains("name=\"variant\"")
                    || head_lower.contains("name='variant'")
                {
                    Some("вне цели: вариант")
                } else if head_lower.contains("<script") {
                    Some("вне цели: скрипт")
                } else {
                    None
                };
                if let Some(why) = out_of_scope {
                    report.push_str(&format!("{test}|{reference}|{why}\n"));
                    let _ = std::fs::write(report_path.as_str(), &report);
                    continue;
                }
                let mut shots = vec![];
                // Разделитель между страницами: без него «тест совпал с
                // эталоном» неотличимо от «ни один не перерисовался».
                // Разделитель обязан быть РОВНЫМ: если он сам не нарисовался,
                // «сменился» дальше считается от чужого кадра, и страница
                // принимается по устаревшему снимку. В числах это скачок вида
                // 17.45 % → 0.00 % между двумя одинаковыми прогонами.
                let mut blank = None;
                for path in [test, reference] {
                    // Разделитель показывается перед КАЖДОЙ стороной, а не один
                    // раз на пару. Иначе эталон снимается, пока на экране ещё
                    // устоявшийся ТЕСТ: его кадр отличается от разделителя, а
                    // значит принимается за кадр эталона — и пара получает
                    // ложный ноль расхождения.
                    // Разделитель — не печатная страница: флаг печати снимается, иначе
                    // он рисуется стопкой листов, `flat()` его не узнаёт и `show`
                    // выжидает все 400 шагов (по 6 с на каждый показ разделителя).
                    kamin_html::css::PRINT_MEDIA.store(false, std::sync::atomic::Ordering::Relaxed);
                    blank = show(SEPARATOR.into(), None, true).await.map(|s| s.2);
                    // Печатные пары смотрят печатным носителем: `@media print`
                    // истинен, `screen` — ложен (background-image-only-for-print).
                    // Печатность — свойство ПАРЫ (WPT: `-print` в имени ТЕСТА,
                    // эталон печатается тем же носителем): `page-name-001-print`
                    // против `page-name-001-ref.html` иначе сравнивал стопку
                    // листов с экранным документом.
                    let print = |p: &str| p.contains("-print.") || p.contains("-print-ref");
                    kamin_html::css::PRINT_MEDIA.store(
                        print(test) || print(path),
                        std::sync::atomic::Ordering::Relaxed,
                    );
                    let html =
                        resolve_links(&std::fs::read_to_string(path).unwrap_or_default(), path);
                    let mut shot = show(html.clone(), blank.clone(), false).await;
                    // Страница, не отличившаяся от разделителя, не нарисовалась:
                    // повторяем показ, а не записываем пустоту в отчёт.
                    for _ in 0..2 {
                        // Мерой служат ЧЕРНИЛА, а не побайтовое отличие: кадр с
                        // одной кромкой уже «не равен» разделителю, но рисовать
                        // на нём нечего. Страница, которая рисоваться умеет,
                        // получает ещё две попытки — пустой вердикт должен
                        // означать «действительно пусто», а не «не успела».
                        let drew = shot
                            .as_ref()
                            .is_some_and(|s| ink(&s.2, blank.as_ref()) >= INK_MIN);
                        // Повтор нужен странице, которая НЕ УСПЕЛА, а не той,
                        // что устоялась пустой: `b` в следе означает 120 равных
                        // разделителю кадров подряд — рисовать там нечего, и
                        // ещё два показа только жгут по две секунды каждый
                        // (css-backgrounds: 23 с на пару вместо 0.6 с).
                        let settled_blank = trace.borrow().ends_with('b');
                        if drew || settled_blank {
                            break;
                        }
                        shot = show(html.clone(), blank.clone(), false).await;
                    }
                    shots.push(shot);
                }
                // Тест сам сказал, чего быть не должно: «no red». Проверяем
                // это ДО сравнения с эталоном — оно слепо к случаю, когда обе
                // стороны сломаны одинаково.
                let source = std::fs::read_to_string(test).unwrap_or_default();
                let forbids_red = source.to_ascii_lowercase().contains("no red");
                let red_seen = forbids_red
                    && shots[0]
                        .as_ref()
                        .is_some_and(|s| red_pixels(&s.2) > s.2.len() / 4 / 2000);
                // WPT разрешает тесту НЕСКОЛЬКО эталонов (`rel=match`):
                // совпадение с любым — зачёт. Стенд сравнивает с первым, а
                // остальные проверяет, только если первый не сошёлся
                // (`hyphens-manual-011`: два эталона — с дефисом-минусом и с
                // настоящим знаком переноса).
                // ГОЧА: `rel="mismatch"` СОДЕРЖИТ подстроку `match`, а значит
                // при поиске по подстроке анти-эталон шёл запасным эталоном —
                // и тест, совпавший с ним (то есть по-настоящему провалившийся),
                // получал маленькое число и красился зелёным. Разбираем `rel`
                // списком слов и держим анти-эталоны отдельным оракулом.
                let mut alternates: Vec<String> = vec![];
                let mut mismatches: Vec<String> = vec![];
                let source_for_refs = std::fs::read_to_string(test).unwrap_or_default();
                for tag in source_for_refs.to_ascii_lowercase().split("<link").skip(1) {
                    let head = &tag[..tag.find('>').unwrap_or(tag.len())];
                    let Some(rel_at) = head.find("rel") else {
                        continue;
                    };
                    let rel = head[rel_at + 3..]
                        .trim_start()
                        .strip_prefix('=')
                        .map(|r| r.trim_start())
                        .unwrap_or("");
                    let rel = match rel.chars().next() {
                        Some(q @ ('"' | '\'')) => {
                            let rest = &rel[1..];
                            &rest[..rest.find(q).unwrap_or(rest.len())]
                        }
                        _ => &rel[..rel.find(char::is_whitespace).unwrap_or(rel.len())],
                    };
                    let anti = rel.split_whitespace().any(|t| t == "mismatch");
                    if !anti && !rel.split_whitespace().any(|t| t == "match") {
                        continue;
                    }
                    let Some(at) = head.find("href") else {
                        continue;
                    };
                    let rest = &head[at + 4..];
                    let Some(open) = rest.find(['"', '\'']) else {
                        continue;
                    };
                    let quote = rest.as_bytes()[open] as char;
                    let Some(close) = rest[open + 1..].find(quote) else {
                        continue;
                    };
                    let href = &rest[open + 1..open + 1 + close];
                    let dir = std::path::Path::new(test)
                        .parent()
                        .unwrap_or_else(|| std::path::Path::new("."));
                    let full = dir.join(href);
                    let full = full.to_string_lossy().replace('/', "\\");
                    if anti {
                        if !mismatches.contains(&full) {
                            mismatches.push(full);
                        }
                    } else if !full.eq_ignore_ascii_case(reference) && !alternates.contains(&full) {
                        alternates.push(full);
                    }
                }
                let verdict = match (&shots[0], &shots[1]) {
                    _ if red_seen => "красное видно".into(),
                    // Пустая страница совпадает с разделителем, и такая пара
                    // дала бы ложный ноль. Это не «сошлось», это «нечего
                    // сравнивать»: страница не нарисовалась вовсе.
                    // Белый экран у ЛЮБОЙ из сторон — ошибка. Побайтового
                    // равенства разделителю мало: страница с единственной
                    // кромкой сглаживания уже «не равна», а рисовать на ней
                    // нечего, и пара уходит в зелёные с нулём расхождения.
                    // Пусто у любой из сторон — либо одна нарисовала на порядок
                    // меньше другой: значит она не отрисовалась, а сравнение
                    // ничего не проверяет.
                    (Some((_, _, a)), Some((_, _, b)))
                        if {
                            let (x, y) = (ink(a, blank.as_ref()), ink(b, blank.as_ref()));
                            // `ink` отдаёт `usize::MAX`, когда кадр и разделитель
                            // разной длины: голое `* 8` переполнялось и роняло
                            // задачу стенда (197 HUNG прошлого захода с листом).
                            x.min(y) < INK_MIN || x.min(y).saturating_mul(8) < x.max(y)
                        } =>
                    {
                        format!(
                            "пустая страница {}/{}",
                            ink(a, blank.as_ref()),
                            ink(b, blank.as_ref())
                        )
                    }
                    (Some((_, _, a)), Some((_, _, b))) => {
                        let d = diff(a, b);
                        // Тест сам объявил допуск (`meta name=fuzzy`) — часть
                        // протокола reftest: расхождение в пределах названного
                        // ЧИСЛА точек не провал. Процент переводится в точки
                        // по размеру снимка.
                        let allowed = fuzzy_pixels(&source);
                        let points = (d as f64 / 400.0 * a.len() as f64) as usize;
                        if allowed > 0 && points <= allowed {
                            "0.00".to_string()
                        } else {
                            format!("{d:.2}")
                        }
                    }
                    _ => "снимок не получен".into(),
                };
                // Не сошлось с первым эталоном — пробуем остальные.
                let mut verdict = verdict;
                if verdict.parse::<f32>().is_ok_and(|d| d > 0.5) {
                    for other in &alternates {
                        if !std::path::Path::new(other).is_file() {
                            continue;
                        }
                        let html = resolve_links(
                            &std::fs::read_to_string(other).unwrap_or_default(),
                            other,
                        );
                        // Разделитель — не печатная страница: флаг печати снимается, иначе
                        // он рисуется стопкой листов, `flat()` его не узнаёт и `show`
                        // выжидает все 400 шагов (по 6 с на каждый показ разделителя).
                        kamin_html::css::PRINT_MEDIA.store(false, std::sync::atomic::Ordering::Relaxed);
                        blank = show(SEPARATOR.into(), None, true).await.map(|s| s.2);
                        let Some(shot) = show(html, blank.clone(), false).await else {
                            continue;
                        };
                        let Some((_, _, a)) = &shots[0] else { continue };
                        let d = diff(a, &shot.2);
                        if d < verdict.parse::<f32>().unwrap_or(f32::MAX) {
                            verdict = format!("{d:.2}");
                        }
                        if d <= 0.5 {
                            break;
                        }
                    }
                }
                // ПРОБОВАЛИ И ОТКАТИЛИ: применять вердикт только когда
                // анти-эталон отличается от ЭТАЛОНА больше порога. Замерено:
                // CSS2 +5, CSS3 +4. Но выборочная проверка показала, что
                // правка ПРЯЧЕТ настоящие провалы: у `text-wrap-nowrap-001`
                // анти-эталон отличается настоящим `width: 20ch`, и совпадение
                // эталона с ним значит, что ширину не держим МЫ. Отличить это
                // от честно неразличимого анти-эталона (`selectors/grouping-002`
                // — только цвет двух строк текста) стенд не может, а ослаблять
                // мерило ради счёта нельзя.
                //
                // Анти-эталон (`rel="mismatch"`): совпадение с ним — ПРОВАЛ.
                // Проверяется всегда, даже когда пара уже зелёная: именно
                // зелёная пара и подозрительна — обе стороны могли сломаться
                // одинаково.
                if verdict.parse::<f32>().is_ok() {
                    for other in &mismatches {
                        if !std::path::Path::new(other).is_file() {
                            continue;
                        }
                        // Разделитель — не печатная страница: флаг печати снимается, иначе
                        // он рисуется стопкой листов, `flat()` его не узнаёт и `show`
                        // выжидает все 400 шагов (по 6 с на каждый показ разделителя).
                        kamin_html::css::PRINT_MEDIA.store(false, std::sync::atomic::Ordering::Relaxed);
                        blank = show(SEPARATOR.into(), None, true).await.map(|s| s.2);
                        let html = resolve_links(
                            &std::fs::read_to_string(other).unwrap_or_default(),
                            other,
                        );
                        let Some(shot) = show(html, blank.clone(), false).await else {
                            continue;
                        };
                        let Some((_, _, a)) = &shots[0] else { continue };
                        if diff(a, &shot.2) <= 0.5 {
                            verdict = "совпал с анти-эталоном".into();
                            break;
                        }
                    }
                }
                if dumping && (dump_all || verdict.parse::<f32>().map_or(true, |d| d > 0.5)) {
                    let stem = std::path::Path::new(test)
                        .file_stem()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_default();
                    if let Some(shot) = &shots[0] {
                        dump(&stem, shot);
                    }
                    if let Some(shot) = &shots[1] {
                        dump(&format!("{stem}--ref"), shot);
                    }
                }
                report.push_str(&format!("{test}|{reference}|{verdict}\n"));
                // Отчёт пишется после каждой пары: падение на одном файле не
                // должно стирать результат всего прогона.
                let _ = std::fs::write(report_path.as_str(), &report);
                // Порог — вдвое больше, чем нужно на три страницы с полным
                // опросом кадра. Всё, что дольше, тормозит не из-за ожидания.
                let spent = started.elapsed().as_millis();
                if spent > 8000 {
                    slow.push((spent, test.clone()));
                }
                // Замер по КАЖДОЙ паре, не только по медленным. Стенд
                // деградирует на длинном прогоне — страницы перестают отдавать
                // кадр, показ выбирает весь запас опроса, и числа раздела
                // становятся недостоверными. Порог в 8 секунд ловит только
                // хвост; кривая времени и памяти показывает, КОГДА поворот.
                timing.push_str(&format!(
                    "{}|{spent}|{}|{}\n",
                    timing_lines,
                    rss_mb(),
                    trace.replace(String::new()),
                ));
                timing_lines += 1;
                let _ = std::fs::write("target/wpt-timing.txt", &timing);
            }
            let _ = std::fs::write(report_path.as_str(), report);
            slow.sort_by(|a, b| b.0.cmp(&a.0));
            let mut lines = String::new();
            for (ms, test) in &slow {
                lines.push_str(&format!("{ms}|{test}\n"));
            }
            let _ = std::fs::write("target/wpt-slow.txt", lines);
            eprintln!("медленных пар: {}", slow.len());
            cx.update(|cx| cx.quit()).ok();
        })
        .detach();
    });
}
