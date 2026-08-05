//! Сборка дерева узлов в элементы GPUI.
//!
//! Блочные узлы становятся `div` со своим стилем; подряд идущие инлайн-узлы
//! собираются в один абзац (`inline.rs`). Списки, таблицы и картинки имеют
//! свои правила — они и описаны в доке отдельными разделами.

use crate::apply::{apply, apply_hover};
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

/// Базовый стиль элемента плюс слой наведения, если он есть.
fn styled_div(e: &Element) -> gpui::Div {
    let d = apply(div(), &e.style);
    match &e.hover {
        Some(h) => apply_hover(d, h),
        None => d,
    }
}

/// Отрисовать корневые узлы документа.
///
/// Годится для короткого документа — виджета, ответа модели. Длинный документ
/// рисуйте по блокам (`render_block`): раскладка в GPUI считается заново
/// каждый кадр, поэтому стоимость кадра обязана зависеть от видимой части, а
/// не от размера документа.
pub fn render(nodes: &[Node], opts: &RenderOpts) -> Vec<AnyElement> {
    let root = Computed::default();
    blocks(nodes, &root, opts)
}

/// Один блок верхнего уровня — единица виртуализации.
///
/// Список GPUI спрашивает только видимые блоки, и невидимая часть документа
/// не стоит ничего: ни раскладки, ни отрисовки. Это то же ухищрение, которым
/// держится дерево файлов и чат.
pub fn render_block(nodes: &[Node], index: usize, opts: &RenderOpts) -> Option<AnyElement> {
    let node = nodes.get(index)?;
    let root = Computed::default();
    blocks(std::slice::from_ref(node), &root, opts)
        .into_iter()
        .next()
}

/// Разбор списка детей на блоки: инлайн-подряд склеивается в абзац.
fn blocks(nodes: &[Node], inherited: &Computed, opts: &RenderOpts) -> Vec<AnyElement> {
    let collapsed = collapse_margins(nodes);
    let mut out = vec![];
    let mut pending: Vec<Node> = vec![];
    let nodes = collapsed.as_slice();
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

/// Схлопывание вертикальных отступов соседних блоков.
///
/// В CSS нижний отступ одного блока и верхний отступ следующего не
/// складываются, а сливаются в больший из двух. Движок раскладки под нами
/// складывает их, и документ становится длиннее браузерного — расхождение
/// накапливается сверху вниз и было поймано сравнением с Chrome.
fn collapse_margins(nodes: &[Node]) -> Vec<Node> {
    let mut out: Vec<Node> = nodes.to_vec();
    // Отступ первого ребёнка «протекает» наружу, если родителя от него не
    // отделяют ни рамка, ни внутренний отступ: в CSS это один и тот же отступ,
    // а не два. Без этого блок уезжает вниз на величину детского отступа.
    for node in out.iter_mut() {
        let Node::Element(e) = node else { continue };
        if e.inline {
            continue;
        }
        let separated = e.style.padding.top.is_some() || e.style.border_width.top.is_some();
        if separated {
            continue;
        }
        // Именно ПЕРВЫЙ блок в потоке: отступ второго и последующих
        // схлопывается с соседом, а не выносится наружу.
        let child_top = e
            .children
            .iter()
            .find(|c| match c {
                Node::Element(ch) => !ch.inline,
                Node::Text(t) => !t.trim().is_empty(),
            })
            .and_then(|c| match c {
                Node::Element(ch) => match ch.style.margin.top {
                    Some(Len::Px(v)) if v > 0.0 => Some(v),
                    _ => None,
                },
                _ => None,
            });
        if let Some(v) = child_top {
            let own = match e.style.margin.top {
                Some(Len::Px(t)) => t,
                _ => 0.0,
            };
            e.style.margin.top = Some(Len::Px(own.max(v)));
            for c in e.children.iter_mut() {
                if let Node::Element(ch) = c
                    && !ch.inline
                    && matches!(ch.style.margin.top, Some(Len::Px(t)) if t > 0.0)
                {
                    ch.style.margin.top = Some(Len::Px(0.0));
                    break;
                }
            }
        }
    }
    let mut prev_bottom: Option<f32> = None;
    for node in out.iter_mut() {
        let Node::Element(e) = node else {
            // Переводы строк между блоками разрывом потока не считаются: в
            // форматированной разметке они стоят везде, и из-за них
            // схлопывание не срабатывало ни разу (поймано сравнением с
            // Chrome — документ уезжал вниз).
            if matches!(node, Node::Text(t) if t.trim().is_empty()) {
                continue;
            }
            prev_bottom = None;
            continue;
        };
        // Отступы схлопываются только у блоков в обычном потоке.
        if e.inline || e.style.position == Some(crate::computed::Position::Absolute) {
            prev_bottom = None;
            continue;
        }
        let top = match e.style.margin.top {
            Some(Len::Px(v)) => v,
            _ => 0.0,
        };
        if let Some(bottom) = prev_bottom
            && top > 0.0
        {
            // Верхний отступ уменьшается на уже отданное нижним отступом
            // предыдущего блока: в сумме получается больший из двух.
            e.style.margin.top = Some(Len::Px((top - bottom).max(0.0)));
        }
        prev_bottom = match e.style.margin.bottom {
            Some(Len::Px(v)) => Some(v),
            _ => Some(0.0),
        };
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
    if let Some(el) = crate::forms::element(e, &e.style) {
        return Some(el);
    }
    match e.tag.as_str() {
        "img" => Some(image(e)),
        "svg" => crate::svg::element(e).or_else(|| Some(image(e))),
        // Свой бокс (фон, рамка, отступы) означает, что кусок не может быть
        // прогоном текста: прогон не умеет рисовать вокруг себя рамку.
        _ if has_own_box(&e.style) => {
            let merged = inline::inherit(inherited, &e.style);
            Some(
                styled_div(e)
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
    // Элементы форм рисуются своим набором: без него поле ввода — пустой
    // прямоугольник, что выглядит поломкой разметки.
    if let Some(el) = crate::forms::element(e, &e.style) {
        return el;
    }
    match e.tag.as_str() {
        "img" => image(e),
        // Рисунок не разобрался — показываем запасной текст, а не пустоту.
        "svg" => crate::svg::element(e).unwrap_or_else(|| {
            styled_div(e)
                .child(SharedString::from("[рисунок]"))
                .into_any_element()
        }),
        "hr" => styled_div(e).w_full().into_any_element(),
        "table" => table(e, &merged, opts),
        "ul" | "ol" => list(e, &merged, opts),
        "pre" => pre(e, &merged, opts),
        _ => {
            let mut d = styled_div(e);
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
    let mut d = styled_div(e);
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
    styled_div(e)
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
    styled_div(e)
        .flex()
        .flex_col()
        .children(lines)
        .into_any_element()
}

/// Текст поддерева — нужен формам (`<textarea>`, `<option>`).
pub fn gather_text_public(nodes: &[Node], out: &mut String) {
    gather_text(nodes, out)
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

    // ОДНА сетка на всю таблицу, а не по сетке на строку. Со строками-сетками
    // ширина колонки считалась внутри строки, и соседние строки расходились —
    // заголовок стоял над одним столбцом, значения под другим. Колонки общие
    // только если ячейки живут в общей сетке.
    let mut cells: Vec<AnyElement> = vec![];
    for row in rows {
        let row_style = inline::inherit(inherited, &row.style);
        for child in &row.children {
            let Node::Element(cell) = child else { continue };
            if cell.tag != "td" && cell.tag != "th" {
                continue;
            }
            let cm = inline::inherit(&row_style, &cell.style);
            // Фон и рамка строки переносятся на её ячейки: своей строки как
            // элемента больше нет, а зебра и разделители нужны.
            let mut d = styled_div(cell);
            if let Some(bg) = row.style.background {
                d = d.bg(bg.to_hsla());
            }
            cells.push(
                d.flex()
                    .flex_col()
                    .children(blocks(&cell.children, &cm, opts))
                    .into_any_element(),
            );
        }
    }

    // Заголовок таблицы: браузер рисует его над сеткой, а не ячейкой.
    let caption = e.children.iter().find_map(|c| match c {
        Node::Element(cap) if cap.tag == "caption" => {
            let cm = inline::inherit(inherited, &cap.style);
            Some(
                styled_div(cap)
                    .flex()
                    .flex_col()
                    .children(blocks(&cap.children, &cm, opts))
                    .into_any_element(),
            )
        }
        _ => None,
    });

    styled_div(e)
        .flex()
        .flex_col()
        .children(caption)
        .child(
            div()
                .grid()
                .grid_template_cols(track_list(cols))
                .children(cells),
        )
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dom::parse;

    fn find_class<'a>(nodes: &'a [Node], class: &str) -> Option<&'a Element> {
        for n in nodes {
            if let Node::Element(e) = n {
                if e.attr("class")
                    .is_some_and(|c| c.split_whitespace().any(|x| x == class))
                {
                    return Some(e);
                }
                if let Some(found) = find_class(&e.children, class) {
                    return Some(found);
                }
            }
        }
        None
    }

    /// Разворачивает обёртки документа до содержимого страницы.
    fn page_children(html: &str) -> Vec<Node> {
        fn dive(n: &[Node]) -> Vec<Node> {
            match n.first() {
                Some(Node::Element(e)) if e.tag == "html" || e.tag == "body" => dive(&e.children),
                _ => n.to_vec(),
            }
        }
        dive(&parse(html, ""))
    }

    #[test]
    fn margin_collapse_matches_the_browser_on_the_fixture_case() {
        // Ровно тот случай, на котором сравнение с Chrome показало сдвиг на
        // 10 точек: блок-обёртка без своего отступа сверху и ребёнок с ним.
        let page = page_children(
            "<div class=\"page\">\
               <div class=\"wrap\" style=\"margin: 0 0 10px\">w</div>\
               <div class=\"stack\" style=\"margin: 0 0 10px\">\
                 <div class=\"mt\" style=\"margin-top: 24px\">m</div>\
               </div>\
             </div>",
        );
        let children = match &page[0] {
            Node::Element(e) => collapse_margins(&e.children),
            _ => panic!("нет страницы"),
        };
        let stack = children
            .iter()
            .find_map(|n| match n {
                Node::Element(e) if e.attr("class") == Some("stack") => Some(e),
                _ => None,
            })
            .expect("нет обёртки");
        // Отступ ребёнка вынесен наружу (24) и уменьшен на уже отданные
        // предыдущим блоком 10 — суммарный зазор остаётся 24, как в браузере.
        assert_eq!(stack.style.margin.top, Some(Len::Px(14.0)), "у обёртки");
        let child_top = stack.children.iter().find_map(|n| match n {
            Node::Element(e) => Some(e.style.margin.top),
            _ => None,
        });
        assert_eq!(child_top, Some(Some(Len::Px(0.0))), "у ребёнка снят");
    }

    #[test]
    fn adjacent_margins_collapse_into_the_larger() {
        // В CSS нижний отступ одного блока и верхний отступ следующего не
        // складываются: остаётся больший. Иначе документ растёт сверху вниз.
        let nodes = parse(
            "<div style=\"margin-bottom: 10px\">a</div><div style=\"margin-top: 24px\">b</div>",
            "",
        );
        let inner = match &nodes[0] {
            Node::Element(html) => collapse_margins(&html.children),
            _ => panic!("нет корня"),
        };
        let body = match &inner[0] {
            Node::Element(b) => collapse_margins(&b.children),
            _ => panic!("нет body"),
        };
        let second = match &body[1] {
            Node::Element(e) => e.style.margin.top,
            _ => panic!("нет второго блока"),
        };
        // 24 всего, из них 10 уже дал нижний отступ предыдущего блока.
        assert_eq!(second, Some(Len::Px(14.0)), "получено {second:?}");
    }

    #[test]
    fn first_child_margin_leaks_through_a_borderless_parent() {
        // Отступ первого ребёнка в CSS — тот же отступ, что у родителя, если
        // между ними нет ни рамки, ни внутреннего отступа.
        let nodes = parse(
            "<div class=\"wrap\"><div class=\"in\" style=\"margin-top: 24px\">x</div></div>",
            "",
        );
        let inner = match &nodes[0] {
            Node::Element(html) => collapse_margins(&html.children),
            _ => panic!("нет корня"),
        };
        let body = match &inner[0] {
            Node::Element(b) => collapse_margins(&b.children),
            _ => panic!("нет body"),
        };
        let wrap = match &body[0] {
            Node::Element(e) => e,
            _ => panic!("нет обёртки"),
        };
        assert_eq!(
            wrap.style.margin.top,
            Some(Len::Px(24.0)),
            "отступ вынесен наружу"
        );
        let child_top =
            find_class(std::slice::from_ref(&body[0]), "in").and_then(|e| e.style.margin.top);
        assert_eq!(child_top, Some(Len::Px(0.0)), "у ребёнка отступ снят");
    }
}
