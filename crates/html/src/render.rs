//! Сборка дерева узлов в элементы GPUI.
//!
//! Блочные узлы становятся `div` со своим стилем; подряд идущие инлайн-узлы
//! собираются в один абзац (`inline.rs`). Списки, таблицы и картинки имеют
//! свои правила — они и описаны в доке отдельными разделами.

use crate::apply::apply;
use crate::computed::{Computed, Display};
use crate::dom::{Element, Node};
use crate::inline::{self};
use crate::value::Len;
use gpui::{AnyElement, IntoElement, ParentElement, SharedString, Styled, TextStyle, div, px};

/// Настройки отрисовки: то, что задаёт приложение, а не документ.
#[derive(Clone)]
pub struct RenderOpts {
    /// Базовый стиль текста — от него считаются прогоны и наследование.
    pub text: TextStyle,
    /// Ширина колонок таблицы, если документ её не задал.
    pub table_min_col: f32,
}

impl RenderOpts {
    fn base_size(&self) -> f32 {
        f32::from(self.text.font_size.to_pixels(px(16.)))
    }
}

/// Отрисовать корневые узлы документа.
pub fn render(nodes: &[Node], opts: &RenderOpts) -> Vec<AnyElement> {
    let root = Computed::default();
    blocks(nodes, &root, opts)
}

/// Разбор списка детей на блоки: инлайн-подряд склеивается в абзац.
fn blocks(nodes: &[Node], inherited: &Computed, opts: &RenderOpts) -> Vec<AnyElement> {
    let mut out = vec![];
    let mut pending: Vec<Node> = vec![];
    for n in nodes {
        let is_inline = match n {
            Node::Text(t) => !t.trim().is_empty(),
            Node::Element(e) => e.inline && e.style.display.is_none(),
        };
        if is_inline {
            pending.push(n.clone());
            continue;
        }
        if !pending.is_empty() {
            out.push(paragraph(&std::mem::take(&mut pending), inherited, opts));
        }
        if let Node::Element(e) = n {
            out.push(element(e, inherited, opts));
        }
    }
    if !pending.is_empty() {
        out.push(paragraph(&pending, inherited, opts));
    }
    out
}

/// Абзац: одна строка текста с прогонами либо гибкая строка из кусков.
fn paragraph(nodes: &[Node], inherited: &Computed, opts: &RenderOpts) -> AnyElement {
    let mut atom = |e: &Element| -> Option<AnyElement> { atom_element(e, inherited, opts) };
    let pieces = inline::collect(nodes, inherited, &mut atom);
    if pieces.is_empty() {
        return div().into_any_element();
    }
    if inline::single_block(&pieces, opts.base_size())
        && let Some(text) = inline::as_styled_text(&pieces, &opts.text)
    {
        return text.into_any_element();
    }
    let mut render_text = |t: String, style: &Computed| -> AnyElement {
        apply(div(), style)
            .child(SharedString::from(t))
            .into_any_element()
    };
    inline::as_wrapped_row(pieces, &mut render_text)
}

/// Не-текстовые инлайн-элементы, которые в поток встроить нельзя.
fn atom_element(e: &Element, inherited: &Computed, opts: &RenderOpts) -> Option<AnyElement> {
    match e.tag.as_str() {
        "img" => Some(image(e)),
        // Свой бокс (фон, рамка, отступы) означает, что кусок не может быть
        // прогоном текста: прогон не умеет рисовать вокруг себя рамку.
        _ if has_own_box(&e.style) => {
            let merged = inline::inherit(inherited, &e.style);
            Some(
                apply(div(), &e.style)
                    .children(blocks(&e.children, &merged, opts))
                    .into_any_element(),
            )
        }
        _ => None,
    }
}

fn has_own_box(c: &Computed) -> bool {
    c.background.is_some()
        || c.gradient.is_some()
        || c.border_color.is_some()
        || c.border_width.top.is_some()
        || c.padding.top.is_some()
        || c.padding.left.is_some()
        || c.radius.tl.is_some()
}

/// Блочный элемент.
fn element(e: &Element, inherited: &Computed, opts: &RenderOpts) -> AnyElement {
    let merged = inline::inherit(inherited, &e.style);
    match e.tag.as_str() {
        "img" => image(e),
        "hr" => apply(div(), &e.style).w_full().into_any_element(),
        "table" => table(e, &merged, opts),
        "ul" | "ol" => list(e, &merged, opts),
        "pre" => pre(e, &merged, opts),
        _ => {
            let mut d = apply(div(), &e.style);
            // Блок без явного display — колонка: в HTML блоки идут сверху вниз.
            if e.style.display.is_none() {
                d = d.flex().flex_col();
            }
            if e.style.display == Some(Display::Flex) && e.style.flex_dir.is_none() {
                d = d.flex_row();
            }
            d.children(blocks(&e.children, &merged, opts))
                .into_any_element()
        }
    }
}

/// Картинка: `src` с `data:`-URI или путь. Внешние URL не грузим — документ
/// рисуется в чате, где сеть запрещена по тем же причинам, что и в вебвью.
fn image(e: &Element) -> AnyElement {
    let src = e.attr("src").unwrap_or_default();
    let mut d = apply(div(), &e.style);
    if let Some(Len::Px(w)) = e.style.width {
        d = d.w(px(w));
    }
    if src.starts_with("data:") || src.starts_with("file:") || src.starts_with('/') {
        return d
            .child(gpui::img(SharedString::from(src.to_string())))
            .into_any_element();
    }
    // Пустая рамка вместо чужой картинки: молча ничего не показать хуже —
    // в разметке останется дыра без объяснения.
    d.child(SharedString::from(
        e.attr("alt")
            .map(str::to_string)
            .unwrap_or_else(|| "[изображение]".into()),
    ))
    .into_any_element()
}

/// Список: маркер рисуем сами — `list-style` в GPUI нет.
fn list(e: &Element, inherited: &Computed, opts: &RenderOpts) -> AnyElement {
    let ordered = e.tag == "ol";
    let mut rows = vec![];
    let mut idx = 1usize;
    for child in &e.children {
        let Node::Element(li) = child else { continue };
        if li.tag != "li" {
            continue;
        }
        let marker = if ordered {
            format!("{idx}.")
        } else {
            "•".to_string()
        };
        idx += 1;
        let merged = inline::inherit(inherited, &li.style);
        rows.push(
            div()
                .flex()
                .flex_row()
                .gap_x(px(6.))
                .items_start()
                .child(
                    div()
                        .flex_shrink_0()
                        .min_w(px(14.))
                        .child(SharedString::from(marker)),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .children(blocks(&li.children, &merged, opts)),
                )
                .into_any_element(),
        );
    }
    apply(div(), &e.style)
        .flex()
        .flex_col()
        .children(rows)
        .into_any_element()
}

/// Блок кода: переносы значимы, поэтому строки рисуются отдельно, а не
/// склеиваются схлопыванием пробелов.
fn pre(e: &Element, inherited: &Computed, _opts: &RenderOpts) -> AnyElement {
    let mut text = String::new();
    gather_text(&e.children, &mut text);
    let merged = inline::inherit(inherited, &e.style);
    let lines: Vec<AnyElement> = text
        .lines()
        .map(|l| {
            apply(div(), &merged)
                .child(SharedString::from(l.to_string()))
                .into_any_element()
        })
        .collect();
    apply(div(), &e.style)
        .flex()
        .flex_col()
        .children(lines)
        .into_any_element()
}

fn gather_text(nodes: &[Node], out: &mut String) {
    for n in nodes {
        match n {
            Node::Text(t) => out.push_str(t),
            Node::Element(e) => gather_text(&e.children, out),
        }
    }
}

/// Таблица.
///
/// Колонки — по содержимому: каждая дорожка это `minmax(min-content, auto)`,
/// последняя забирает остаток (`1fr`). Так ведёт себя и настоящая табличная
/// раскладка: узкие колонки сжимаются до содержимого, широкая тянется.
///
/// Это стало возможно только вместе с патчем произвольных дорожек в GPUI —
/// короткая форма умела ровно «N равных колонок», и таблица из даты и длинного
/// текста разъезжалась пополам.
fn table(e: &Element, inherited: &Computed, opts: &RenderOpts) -> AnyElement {
    let mut rows: Vec<&Element> = vec![];
    collect_rows(&e.children, &mut rows);
    let cols = rows
        .iter()
        .map(|r| {
            r.children
                .iter()
                .filter(|c| matches!(c, Node::Element(e) if e.tag == "td" || e.tag == "th"))
                .count()
        })
        .max()
        .unwrap_or(1)
        .max(1) as u16;

    let mut out = vec![];
    for row in rows {
        let merged = inline::inherit(inherited, &row.style);
        let cells: Vec<AnyElement> = row
            .children
            .iter()
            .filter_map(|c| match c {
                Node::Element(cell) if cell.tag == "td" || cell.tag == "th" => {
                    let cm = inline::inherit(&merged, &cell.style);
                    Some(
                        apply(div(), &cell.style)
                            .flex()
                            .flex_col()
                            .children(blocks(&cell.children, &cm, opts))
                            .into_any_element(),
                    )
                }
                _ => None,
            })
            .collect();
        out.push(
            apply(div(), &row.style)
                .grid()
                .grid_template_cols(track_list(cols))
                .children(cells)
                .into_any_element(),
        );
    }
    apply(div(), &e.style)
        .flex()
        .flex_col()
        .children(out)
        .into_any_element()
}

/// Дорожки таблицы: все по содержимому, последняя забирает остаток строки.
/// `min-content` снизу не даёт колонке сжаться в ноль на узкой панели.
fn track_list(cols: u16) -> Vec<gpui::GridTrack> {
    (0..cols)
        .map(|i| {
            let last = i + 1 == cols;
            gpui::GridTrack::MinMax(Box::new((
                gpui::GridTrack::MinContent,
                if last {
                    gpui::GridTrack::Fraction(1.0)
                } else {
                    gpui::GridTrack::Auto
                },
            )))
        })
        .collect()
}

fn collect_rows<'a>(nodes: &'a [Node], out: &mut Vec<&'a Element>) {
    for n in nodes {
        if let Node::Element(e) = n {
            if e.tag == "tr" {
                out.push(e);
            } else if e.tag == "thead" || e.tag == "tbody" || e.tag == "tfoot" {
                collect_rows(&e.children, out);
            }
        }
    }
}
