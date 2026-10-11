//! Буквица `initial-letter` как флоат.
// owner: A

use crate::dom::{Element, Node};
use crate::render::RenderOpts;
use crate::style::computed::Computed;
use crate::style::values::value::Len;
use crate::text::text_box::{blank_text, normal_fraction};
mod letter;
use letter::{emit_initial_letter, initial_letter_style};
mod leading;
pub(super) use leading::inline_float_host;
pub(crate) use leading::px_margin_w;
pub(super) use leading::split_leading_float;

/// `initial-letter` (css-inline-3 §initial-letter): буквица — не кусок
/// текста, а коробка В НАЧАЛЕ БЛОКА, которую строки обтекают. Эталоны WPT
/// пишут её плавающим блоком (`initial-letter-drop-initial-ref`: `float:
/// left; width: 80px; height: 80px; margin-top: 2px` при `font: 20px/24px
/// Ahem` и `initial-letter: 3`), и наше обтекание (`wrap_floats` →
/// `kamin-float` → `float_flow`) такой узел уже ведёт — поэтому буквица
/// расшивается в синтетический флоат ДО `wrap_floats`, и обе стороны пары
/// идут одним путём.
///
/// Числа (§sizing-initial-letter; Blink `ComputeInitialLetterFont` и
/// `initial_letter_utils.cc::ComputeInitialLetterBoxBlockOffset`):
/// * прописная буквицы C = (N − 1)·line-height + cap(абзаца);
/// * кегль F = C / доля прописной шрифта буквицы (Ahem: 64 / 0.8 = 80);
/// * коробка высотой ascent(F) + descent(F), строка той же высоты;
/// * верх коробки = N·line-height − ascent(F) − (descent(абзаца) +
///   полулидинг) — у Ahem/20/24/3 ровно 2;
/// * осадка M < N: строки под буквицей уходят на (N − M) строк вниз — как
///   `<br>` перед текстом в эталонах `raise`/`sunk`.
///
/// Собственные `font-size` и `line-height` слоя НЕ действуют
/// (§initial-letter-properties). Шаг 1: горизонтальное письмо, буква —
/// первый текстовый узел блока (перед ним допустимы только флоаты и пустой
/// текст), ширина коробки — продвижение нуля семейства (у Ahem равно
/// кеглю; текстовым шрифтам нужен щуп продвижения знака — шаг 2).
pub(crate) fn initial_letter_float(
    nodes: Vec<Node>,
    inherited: &Computed,
    opts: &RenderOpts,
) -> Vec<Node> {
    let Some(first) = inherited.first_letter.as_deref() else {
        return nodes;
    };
    let Some((size_lines, sink)) = first.initial_letter else {
        return nodes;
    };
    // Вертикальное письмо больше НЕ отсекается. Эталоны семейства пишут в
    // вертикали ту же плавающую коробку 80×80 с тем же `float: left`
    // (`initial-letter-drop-initial-vrl-ref` и ещё пятнадцать), меняются
    // ровно две вещи: поле сдвига стоит на БЛОК-СТАРТЕ (`margin-right` при
    // `*-rl`, `margin-left` при `*-lr` — css-writing-modes-4 §6.3, строка
    // `block-start`), а величина сдвига у `vertical-*` считается
    // центрированием, а не по алфавитной базовой.
    let vert = inherited.vertical == Some(true);
    // `sideways-*` типографски ГОРИЗОНТАЛЕН: у него алфавитная базовая и та
    // же формула, что в горизонтали. Blink `initial_letter_utils.cc:81-83`
    // разводит ветки условием
    // `IsHorizontalTypographicMode() || text-orientation: sideways`, а
    // `sideways-rl`/`sideways-lr` попадают во вторую половину этого «или».
    let sideways = inherited.sideways == Some(true);
    // Блок-старт вертикали: правый край при `vertical-rl`/`sideways-rl`,
    // левый при `vertical-lr`/`sideways-lr`.
    let block_rl = inherited.vertical_rl == Some(true);
    let at = nodes.iter().position(|n| match n {
        Node::Text(t) => !blank_text(t),
        Node::Element(e) => !e.style.float.is_some_and(|f| f != 0),
    });
    let Some(at) = at else {
        return nodes;
    };
    let Node::Text(text) = &nodes[at] else {
        return nodes;
    };
    let Some((pos, ch)) = text.char_indices().find(|(_, c)| !c.is_whitespace()) else {
        return nodes;
    };
    let end = pos + ch.len_utf8();
    let font_px = match inherited.font_size {
        Some(Len::Px(v)) => v,
        _ => opts.base_size(),
    };
    let family = inherited.font_family.clone().unwrap_or_default();
    let (asc, desc, cap) = crate::text::metrics::vmetrics_px(&family, font_px);
    let line = match inherited.line_height {
        Some(Len::Px(v)) => v,
        Some(Len::Pct(k)) | Some(Len::Em(k)) => k * font_px,
        _ => font_px * normal_fraction(inherited, opts),
    };
    // Доля прописной — у шрифта БУКВИЦЫ: слой может сменить семейство.
    let letter_family = first.font_family.clone().unwrap_or_else(|| family.clone());
    let (_, _, cap_frac) = crate::text::metrics::vmetrics_px(&letter_family, 1.0);
    if cap_frac <= 0.0 || line <= 0.0 {
        return nodes;
    }
    let want_cap = (size_lines - 1.0) * line + cap;
    let letter_px = want_cap / cap_frac;
    let (l_asc, l_desc, _) = crate::text::metrics::vmetrics_px(&letter_family, letter_px);
    let box_h = l_asc + l_desc;
    let half_leading = (line - (asc + desc)) / 2.0;
    // Размер меньше осадки (`3 5`) — выравнивание по верху
    // (§initial-letter-block-position): коробка опускается на sink строк.
    let rows = size_lines.ceil() as u32;
    let top = if rows < sink {
        line * sink as f32 - box_h
    } else if vert && !sideways {
        // Вертикальное письмо со СМЕШАННОЙ ориентацией: базовая линия
        // центральная, и коробка выравнивается по центру строки. Blink
        // `initial_letter_utils.cc:99-101` дословно:
        //   // In vertical writing mode, `block_offset` will be physical
        //   // offset x. Align initial letter box in center.
        //   return (line_height * size - block_size) / 2;
        // Ahem 20px/24px и `initial-letter: 3`: (3·24 − 80)/2 = −4 — ровно
        // `margin-right: -4px` эталона `initial-letter-drop-initial-vrl-ref`
        // и `margin-left: -4px` эталона `-vlr-ref`.
        (size_lines * line - box_h) / 2.0
    } else {
        size_lines * line - l_asc - (desc + half_leading)
    };
    let shift = rows.saturating_sub(sink);
    // Слой — копия стиля блока плюс объявления `::first-letter`, поэтому
    // «своё» у слоя — то, что отличается от блока: поля, цвет, фон.
    let own = |layer: Option<Len>, base: Option<Len>| match layer {
        Some(Len::Px(v)) if layer != base => v,
        _ => 0.0,
    };
    let color = match first.color {
        Some(c) if Some(c) != inherited.color => Some(c),
        // `::first-letter` наследует у `::first-line`
        // (`initial-letter-with-first-line`: `color: inherit` → цвет строки).
        _ => inherited
            .first_line
            .as_deref()
            .and_then(|l| l.color)
            .or(inherited.color),
    };
    let rtl = inherited.rtl == Some(true);
    // Отступ первой строки (css-inline-3 §initial-letter-indentation:
    // «'text-indent' … cause a shift in the start of the line's contents
    // including the initial letter itself»). Blink сдвигает КОРОБКУ буквицы
    // на отступ (`inline_layout_algorithm.cc`: `bfc_line_offset +=
    // TextIndent()` до `PostPlaceInitialLetterBox`), а своя строка внутри
    // коробки наследует тот же `text-indent`, и ширина коробки его включает
    // (`CalculateInitialLetterBoxInlineSize`). Итог — эталон
    // `initial-letter-indentation-ref`: при `text-indent: 10px` квадрат стоит
    // с `margin-left: 20px`, строки обтекают от 100. У нас знак уже сдвигался
    // унаследованным отступом, а коробка — нет: знак вылезал на 10 точек за
    // флоат, строки 2-4 обтекали по 80 с нахлёстом. Только горизонталь ltr:
    // `-indentation-rtl` зелёный (0.41) на зеркальной ошибке `float: right`
    // в rtl, и менять его сторону нечем.
    let para_indent = match inherited.text_indent {
        Some(Len::Px(v)) if inherited.text_indent_hanging != Some(true) => v,
        _ => 0.0,
    };
    let indent = if !vert && !rtl && para_indent > 0.0 {
        para_indent
    } else {
        0.0
    };
    // Сохранённые пробелы ПЕРЕД буквой входят в буквицу: Blink
    // `FirstLetterPseudoElement::FirstLetterLength` сперва забирает ведущие
    // пробелы, и при `white-space: pre` табуляция остаётся в коробке
    // буквицы. Эталон `initial-letter-with-tab-ref` пишет перед квадратом
    // 80×80 жёлтый (фон слоя) флоат шириной 160 = шаг табуляции АБЗАЦА
    // (8 × 20px Ahem), и текст идёт с 240; раньше ведущий `\t` просто
    // выбрасывался. Шаг — та же формула, что у `tab_stop` абзаца
    // (`tab-size` × ширина `0`, css-text-3 §tab-size); пробел меряется той же
    // шириной — точной ширины пробела здесь нет, у Ahem они равны.
    let lead = &text[..pos];
    let style = initial_letter_style(
        inherited,
        first,
        vert,
        block_rl,
        font_px,
        family,
        letter_family,
        letter_px,
        box_h,
        top,
        own,
        color,
        rtl,
        para_indent,
        indent,
        lead,
    );
    let synthetic = |tag: &str, style: Computed, children: Vec<Node>, inline: bool| {
        Node::Element(Element {
            list_item: None,
            node_id: 0,
            anim: None,
            tag: tag.into(),
            style,
            hover: None,
            first_letter: None,
            first_line: None,
            children,
            attrs: vec![],
            inline,
        })
    };

    emit_initial_letter(&nodes, at, text, pos, end, shift, style, synthetic)
}
