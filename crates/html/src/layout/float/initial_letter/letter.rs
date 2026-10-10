//! Буквица (initial-letter): стиль буквы и выпуск синтетических узлов.

use crate::dom::Node;
use crate::style::computed::Computed;
use crate::style::values::value::Len;

#[allow(clippy::too_many_arguments)]
pub(super) fn emit_initial_letter(
    nodes: &[Node],
    at: usize,
    text: &str,
    pos: usize,
    end: usize,
    shift: u32,
    style: Computed,
    synthetic: impl Fn(&str, Computed, Vec<Node>, bool) -> Node,
) -> Vec<Node> {
    let mut out: Vec<Node> = Vec::with_capacity(nodes.len() + 2 + shift as usize);
    out.extend(nodes[..at].iter().cloned());
    let mut letter = synthetic(
        "div",
        style,
        vec![Node::Text(text[pos..end].to_string())],
        false,
    );
    // Метка буквицы: её место — исключение строки (css-inline-3
    // §initial-letter, Blink `initial_letter_utils.cc`), а не флоат полос:
    // измеряемый хост ставит её `FloatBands::add_initial_letter` (шаг F11);
    // прогон с руби хост по-прежнему не берёт (`initial-letter-*-ruby`).
    if let Node::Element(e) = &mut letter {
        e.attrs.push(("initial-letter".into(), "1".into()));
    }
    out.push(letter);
    for _ in 0..shift {
        out.push(synthetic("br", Computed::default(), vec![], true));
    }
    if end < text.len() {
        out.push(Node::Text(text[end..].to_string()));
    }
    out.extend(nodes[at + 1..].iter().cloned());
    out
}

#[allow(clippy::too_many_arguments)]
pub(super) fn initial_letter_style(
    inherited: &Computed,
    first: &Computed,
    vert: bool,
    block_rl: bool,
    font_px: f32,
    family: String,
    letter_family: String,
    letter_px: f32,
    box_h: f32,
    top: f32,
    own: impl Fn(Option<Len>, Option<Len>) -> f32,
    color: Option<crate::style::values::value::Color>,
    rtl: bool,
    para_indent: f32,
    indent: f32,
    lead: &str,
) -> Computed {
    let lead_w = if !vert && inherited.keep_spaces == Some(true) && !lead.contains(['\n', '\r']) {
        let space = crate::text::metrics::ch_ex_px(&family, font_px).0;
        let stop = match inherited.tab_size_len {
            Some(Len::Px(v)) if v > 0.0 => v,
            _ => inherited.tab_size.unwrap_or(8.0).max(0.0) * space,
        };
        lead.chars().fold(0.0f32, |x, c| match c {
            '\t' if stop > 0.0 => ((x / stop).floor() + 1.0) * stop,
            ' ' => x + space,
            _ => x,
        })
    } else {
        0.0
    };
    let mut style = Computed {
        // Сторона — начало строки: rtl отправляет буквицу вправо.
        float: Some(if rtl { 1 } else { -1 }),
        // Оба размера в точках: без них `float_flow` откатывается на плоский
        // ряд, и строки под буквицей не возвращаются к левому краю. Ширина
        // несёт и внутренний отступ строки буквицы, и ведущие пробелы:
        // `float_flow` меряет обтекание по ней (`float.rs` `narrow`).
        width: Some(Len::Px(
            crate::text::metrics::ch_ex_px(&letter_family, letter_px).0 + indent + lead_w,
        )),
        height: Some(Len::Px(box_h)),
        font_size: Some(Len::Px(letter_px)),
        line_height: Some(Len::Px(box_h)),
        font_family: Some(letter_family),
        font_weight: first.font_weight,
        italic: first.italic,
        color,
        background: (first.background != inherited.background)
            .then_some(first.background)
            .flatten(),
        ..Computed::default()
    };
    // Картинки фона слоя первой буквы (css-pseudo-4 §3.6: к `::first-letter`
    // применимы все свойства фона) — со своими размером, положением,
    // повтором и списками слоёв. Прежде переносился только цвет, и вместо
    // зелёных картинок проступал красный цвет фона (`background-image-007`).
    if first.bg_image.is_some() || first.gradient.is_some() || !first.bg_lists.is_empty() {
        style.bg_image = first.bg_image.clone();
        style.gradient = first.gradient.clone();
        style.gradient_raw = first.gradient_raw.clone();
        style.bg_size = first.bg_size;
        style.bg_pos = first.bg_pos;
        style.bg_repeat = first.bg_repeat;
        style.bg_origin = first.bg_origin;
        style.bg_clip = first.bg_clip;
        style.bg_lists = first.bg_lists.clone();
    }
    // Сдвиг ложится на поле БЛОК-СТАРТА и СКЛАДЫВАЕТСЯ с полем слоя — ровно
    // так же, как в эталонах: `block-position-margins-vrl` задаёт слою
    // `margin-right: 45px`, а его эталон пишет `margin-right: 41px`
    // = 45 + (−4); у `-vlr` то же на левом краю — `margin-left: 11px`
    // = 15 + (−4).
    let (mt, ml, mr) = (
        own(first.margin.top, inherited.margin.top),
        own(first.margin.left, inherited.margin.left),
        own(first.margin.right, inherited.margin.right),
    );
    style.margin.top = Some(Len::Px(if vert { mt } else { mt + top }));
    style.margin.bottom = Some(Len::Px(own(first.margin.bottom, inherited.margin.bottom)));
    style.margin.left = Some(Len::Px(if vert && !block_rl {
        ml + top
    } else {
        ml + indent
    }));
    style.margin.right = Some(Len::Px(if vert && block_rl { mr + top } else { mr }));
    // Знак встаёт ЗА ведущими пробелами: внутренний отступ строки флоата =
    // отступ абзаца + их ширина (в rtl отступ идёт от правого края коробки,
    // и пробелы остаются у начала строки). Без ведущих пробелов отступ
    // наследуется, как раньше.
    if lead_w > 0.0 {
        style.text_indent = Some(Len::Px(para_indent + lead_w));
    }
    style
}
