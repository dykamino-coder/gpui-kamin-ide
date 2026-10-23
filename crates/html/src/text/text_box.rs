//! Метрики строки блока: владелец первой/последней строки для `text-box-trim` и срез по
//! `text-box-edge` (css-inline-3 §4.2–4.3), высота строки для статической позиции,
//! доля `line-height: normal` по шрифту и пустота текстового узла по CSS.

use crate::document::state::ValueCx;

use crate::dom::Node;
use crate::layout::box_kind::{
    breaks_inline, contains_block, holds_line_box, out_of_flow, real_inline,
};
use crate::layout::options::RenderOpts;
use crate::style::cascade::inherit::inherit;
use crate::style::computed::{Computed, Display};
use crate::style::values::value::{Len, px_lossy_zero};

/// Стиль блока, которому принадлежит первая (`start`) или последняя
/// отформатированная строка контейнера — для `text-box-trim`
/// (css-inline-3 §4.2, css-pseudo-4 «first formatted line»).
///
/// `None` — такой строки нет или до неё стоит отступ либо рамка: срезать
/// нечего. Контейнер со строчным содержимым отдаёт СВОЙ стиль; с блочным —
/// спускается в первый/последний in-flow блочный ребёнок (плавающие и
/// абсолютные в потоке не участвуют, пробельные узлы пропускаются). Пустой
/// блок, пустая анонимная коробка (пробельный `<span>` среди блоков —
/// `half-leading-block-box-001/003`), гибкий, сеточный и табличный контекст
/// обрывают поиск; отступ и рамка ПОТОМКА на срезаемой стороне — тоже
/// (`-004/-005`). Строчный элемент с блоком внутри (блок-в-строчном)
/// прозрачен: строка — в его блоке (`block-in-inline-*`).
///
/// Второе значение — узел блока, чей абзац держит строку (`owner` —
/// узел самого контейнера): ему `line_trim::request` адресует срез.
pub(crate) fn text_box_line_style(
    nodes: &[Node],
    inherited: &Computed,
    start: bool,
    owner: u64,
    vcx: ValueCx<'_>,
) -> Option<(Computed, u64)> {
    let has_block = nodes.iter().any(breaks_inline);
    let order: Vec<&Node> = if start {
        nodes.iter().collect()
    } else {
        nodes.iter().rev().collect()
    };
    for (i, n) in order.iter().enumerate() {
        let e = match n {
            Node::Text(t) if blank_text(t) => continue,
            // Непробельный текст — строка в этом контейнере.
            Node::Text(_) => return Some((inherited.clone(), owner)),
            Node::Element(e) => e,
        };
        if e.style.display == Some(Display::None) || out_of_flow(&e.style) {
            continue;
        }
        if e.style.display == Some(Display::Contents) {
            return text_box_line_style(
                &e.children,
                &inherit(inherited, &e.style, vcx),
                start,
                owner,
                vcx,
            );
        }
        if breaks_inline(n) {
            // Через гибкий, сеточный и табличный контекст свойство не
            // распространяется (css-inline-3 §4.2, примечание).
            if matches!(
                e.style.display,
                Some(Display::Flex)
                    | Some(Display::Grid)
                    | Some(Display::Table)
                    | Some(Display::GridLanes)
            ) {
                return None;
            }
            let merged = inherit(inherited, &e.style, vcx);
            let px_of = px_lossy_zero;
            let bs = merged.borders();
            let blocked = if start {
                px_of(merged.padding.top) != 0.0 || px_of(bs.top) != 0.0
            } else {
                px_of(merged.padding.bottom) != 0.0 || px_of(bs.bottom) != 0.0
            };
            if blocked {
                return None;
            }
            // Потомок, который САМ срезает эту сторону, срежет ту же строку в
            // своём хвосте `blocks()`: срез «до метрики» идемпотентен, второй
            // раз его не кладут (`ul, li { text-box-trim: trim-both }` —
            // `text-box-trim-list-001`). `merged` несёт собственные флаги
            // ребёнка: `inherit()` клонирует его стиль, а свойство не
            // наследуется.
            let own_trim = if start {
                merged.text_box_trim_start
            } else {
                merged.text_box_trim_end
            };
            if own_trim {
                return None;
            }
            return text_box_line_style(&e.children, &merged, start, e.node_id, vcx);
        }
        // Блок-в-строчном: оболочка прозрачна, строка — внутри блока.
        if real_inline(e) && contains_block(&e.children) {
            return text_box_line_style(&e.children, inherited, start, owner, vcx);
        }
        // Строчное содержимое. Без блочных братьев это весь контекст
        // форматирования; с ними — анонимная коробка из прогона до блока,
        // и пустая (только пробелы и пустые спаны) строки не имеет.
        let has_line = if has_block {
            let run: Vec<Node> = order[i..]
                .iter()
                .take_while(|m| !breaks_inline(m))
                .map(|m| (*m).clone())
                .collect();
            holds_line_box(&run)
        } else {
            holds_line_box(nodes)
        };
        return has_line.then(|| (inherited.clone(), owner));
    }
    None
}

/// Срез `text-box-trim` одной стороны строки в точках (css-inline-3 §4.2,
/// §4.3): полулидинг корневой строчной коробки плюс расстояние от подъёма
/// (спуска) до метрики края. Та же формула, что `trim_for` в хвосте
/// `blocks()`; нужна и вне его — точке обрыва `line-clamp`.
pub(crate) fn text_box_trim_px(line_style: &Computed, start: bool, opts: &RenderOpts) -> f32 {
    let size = match line_style.font_size {
        Some(Len::Px(v)) => v,
        _ => opts.base_size(),
    };
    let vcx = opts.cx.vcx();
    let family = line_style.font_family.clone().unwrap_or_default();
    let extents = crate::text::metrics::vmetrics_px(&family, size, vcx);
    let (ascent, descent, _) = extents;
    let line = match line_style.line_height {
        Some(Len::Px(v)) => v,
        Some(Len::Pct(k)) | Some(Len::Em(k)) => k * size,
        _ => ascent + descent,
    };
    let half = (line - (ascent + descent)) / 2.0;
    half + edge_inset(line_style, &family, size, extents, start, vcx)
}

/// Расстояние от подъёма (`start`) или спуска до метрики края
/// `text-box-edge` (css-inline-3 §text-box-edge): `text` — ноль, `cap`/`ex`
/// — остаток над прописной/строчной, `alphabetic` — весь спуск (алфавитная
/// линия может стоять НАД нулём глифа: BASE `romn`, `text-box-trim-end-002`),
/// `ideographic`/`-ink` — край em-коробки, синтезированной из подъёма и
/// спуска (§synthesize-baselines: подъём и спуск в той же пропорции, сумма
/// 1em). Blink `InlineBoxState::AdjustEdges` поддерживает только первые
/// четыре; идеографический край по спеке.
pub(crate) fn edge_inset(
    s: &Computed,
    family: &str,
    size: f32,
    (ascent, descent, cap): (f32, f32, f32),
    start: bool,
    vcx: crate::document::state::ValueCx<'_>,
) -> f32 {
    use crate::style::computed::TextEdge;
    let em_box = |part: f32| {
        let sum = ascent + descent;
        if sum > 0.0 {
            part - size * part / sum
        } else {
            0.0
        }
    };
    if start {
        match s.text_box_over {
            TextEdge::Cap => ascent - cap,
            TextEdge::Ex => ascent - crate::text::metrics::ch_ex_px(family, size, vcx).1,
            TextEdge::Ideographic | TextEdge::IdeographicInk => em_box(ascent),
            TextEdge::Text | TextEdge::Alphabetic => 0.0,
        }
    } else {
        match s.text_box_under {
            TextEdge::Alphabetic => descent + crate::text::fonts::alphabetic_em(family, vcx) * size,
            TextEdge::Ideographic | TextEdge::IdeographicInk => em_box(descent),
            TextEdge::Text | TextEdge::Cap | TextEdge::Ex => 0.0,
        }
    }
}

/// Высота строки в точках — для статической позиции блочного элемента.
pub(crate) fn line_height_px(style: &Computed, opts: &RenderOpts) -> f32 {
    let size = match style.font_size {
        Some(Len::Px(v)) => v,
        Some(Len::Em(k)) => k * opts.base_size(),
        _ => opts.base_size(),
    };
    match style.line_height {
        Some(Len::Px(v)) => v,
        // Голое число хранится долей: это множитель к кеглю.
        Some(Len::Pct(k)) | Some(Len::Em(k)) => k * size,
        _ => size * crate::defaults::FALLBACK_NORMAL_LINE_HEIGHT,
    }
}

/// Есть ли в поддереве непустой текст — по нему считается высота строки.
pub(crate) fn has_text(nodes: &[Node]) -> bool {
    nodes.iter().any(|n| match n {
        Node::Text(t) => !blank_text(t),
        Node::Element(e) => has_text(&e.children),
    })
}

/// Доля кегля для `line-height: normal` — по метрикам шрифта элемента.
///
/// Постоянная доля неверна: у Ahem `normal` ровно кегль, у текстовых шрифтов
/// около 1.15–1.3. Из-за постоянной 1.31 коробка с `line-height: 1em` и
/// соседняя без него расходились по высоте строк (`pre-wrap-008`).
pub(crate) fn normal_fraction(style: &Computed, opts: &RenderOpts) -> f32 {
    // Без своего семейства текст набирается шрифтом ДОКУМЕНТА
    // (`opts.text.font_family`, у стенда — Times New Roman), а щуп метрик
    // пустое имя меряет как `GENERIC_SANS` (Segoe UI, `metrics.rs`
    // `use_text_system`): `normal` выходил 1.33 вместо 1.15 у того шрифта,
    // которым строка нарисована, и струт строки из одних атомов был выше
    // атома (`inlines-017-ref`: ячейка 21.3px при картинке 20px).
    let family = style
        .font_family
        .clone()
        .filter(|f| !f.is_empty())
        .unwrap_or_else(|| {
            if style.monospace == Some(true) {
                super::metrics::mono_family_for(style.lang.as_deref(), opts.cx.vcx()).to_string()
            } else {
                opts.text.font_family.to_string()
            }
        });
    let measured = crate::text::metrics::normal_line(&family, opts.cx.vcx());
    if measured > 0.0 {
        measured
    } else {
        opts.normal_line_height
    }
}

/// Пустой ли текстовый узел ПО CSS.
///
/// Схлопывается только `space`, `tab`, `CR`, `LF`. `str::trim` снимает весь
/// юникодный пробел, и узел из идеографических U+3000 (или неразрывных
/// U+00A0) считался пустым: строка из них пропадала целиком, а абзац рвался
/// там, где рваться не должен (`trailing-ideographic-space-017`).
pub(crate) fn blank_text(t: &str) -> bool {
    t.chars().all(|c| matches!(c, ' ' | '\t' | '\r' | '\n'))
}
