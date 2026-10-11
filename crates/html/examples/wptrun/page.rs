//! The runner's single view: one document rendered as a screen page or a print page stack.

use super::page_box::{PageBoxFn, page_box};
use gpui::{Context, IntoElement, ParentElement, Render, Styled, Window, div, px, rgb};
use kamin_html::{Document, RenderOpts, render};
use std::rc::Rc;

pub(super) struct Page {
    pub(super) doc: Rc<Document>,
    /// Сравниваемые листы печатного теста (с нуля), `None` — все.
    pub(super) select: Option<Vec<usize>>,
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
            let boxes: PageBoxFn = std::rc::Rc::new(move |i, name: &str| {
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
                    turn: p.turn,
                }
            });
            let stack = kamin_html::render::render_paged_select(
                self.doc.nodes(),
                &opts,
                geom_for,
                Some(margins),
                self.select.clone(),
            );
            return div()
                .w(px(f32::from(window.viewport_size().width)))
                .h(px(f32::from(window.viewport_size().height)))
                .bg(rgb(0xffffff))
                .text_size(px(16.))
                .font_family(opts.text.font_family.clone())
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
            // Шрифт документа — и наследуемому стилю текста окна: куски без
            // своего семейства, нарисованные НЕ текстовым путём абзаца (ряд
            // слов при картинке вне потока, подпись `alt`), брали шрифт
            // интерфейса (Segoe UI) вместо Times (`background-bg-pos-204-ref`).
            .font_family(opts.text.font_family.clone())
            .children(children)
            .into_any_element()
    }
}

// Кропа по листу больше нет (см. комментарий у `WPT_NO_PAGE`): печатная пара
// сравнивается по всему окну, как остальные, — стопка страниц рисует листы
// сама и масштабирует их в окно.
