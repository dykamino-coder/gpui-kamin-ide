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
        e.marks.initial_letter = true;
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
    fonts: &crate::text::text_fonts::TextFonts,
    vcx: crate::values::ValueCx<'_>,
) -> Computed {
    let lead_w = if !vert
        && inherited.inh.text_flow.keep_spaces == Some(true)
        && !lead.contains(['\n', '\r'])
    {
        let space = crate::text::metrics::ch_ex_px(&family, font_px, fonts, vcx).0;
        let stop = match inherited.inh.text_flow.tab_size_len {
            Some(Len::Px(v)) if v > 0.0 => v,
            _ => inherited.inh.text_flow.tab_size.unwrap_or(8.0).max(0.0) * space,
        };
        lead.chars().fold(0.0f32, |x, c| match c {
            '\t' if stop > 0.0 => ((x / stop).floor() + 1.0) * stop,
            ' ' => x + space,
            _ => x,
        })
    } else {
        0.0
    };
    let mut style = {
        let mut with_inherited = Computed {
            reset_box_layout: crate::style::computed::StyleGroup::new(
                crate::style::computed::ResetBoxLayout {
                    float: Some(if rtl { 1 } else { -1 }),
                    width: Some(Len::Px(
                        crate::text::metrics::ch_ex_px(&letter_family, letter_px, fonts, vcx).0
                            + indent
                            + lead_w,
                    )),
                    height: Some(Len::Px(box_h)),
                    ..Default::default()
                },
            ),
            reset_background: crate::style::computed::StyleGroup::new(
                crate::style::computed::ResetBackground {
                    background: (first.reset_background.background
                        != inherited.reset_background.background)
                        .then_some(first.reset_background.background)
                        .flatten(),
                    ..Default::default()
                },
            ),
            ..Computed::default()
        };
        with_inherited.inh.font.font_size = Some(Len::Px(letter_px));
        with_inherited.inh.font.line_height = Some(Len::Px(box_h));
        with_inherited.inh.font.font_family = Some(letter_family);
        with_inherited.inh.font.font_weight = first.inh.font.font_weight;
        with_inherited.inh.font.italic = first.inh.font.italic;
        with_inherited.inh.ui.color = color;
        with_inherited
    };
    // Картинки фона слоя первой буквы (css-pseudo-4 §3.6: к `::first-letter`
    // применимы все свойства фона) — со своими размером, положением,
    // повтором и списками слоёв. Прежде переносился только цвет, и вместо
    // зелёных картинок проступал красный цвет фона (`background-image-007`).
    if first.reset_background.bg_image.is_some()
        || first.reset_background.gradient.is_some()
        || !first.reset_background.bg_lists.is_empty()
    {
        style.reset_background.bg_image = first.reset_background.bg_image.clone();
        style.reset_background.gradient = first.reset_background.gradient.clone();
        style.reset_background.gradient_raw = first.reset_background.gradient_raw.clone();
        style.reset_background.bg_size = first.reset_background.bg_size;
        style.reset_background.bg_pos = first.reset_background.bg_pos;
        style.reset_background.bg_repeat = first.reset_background.bg_repeat;
        style.reset_background.bg_origin = first.reset_background.bg_origin;
        style.reset_background.bg_clip = first.reset_background.bg_clip;
        style.reset_background.bg_lists = first.reset_background.bg_lists.clone();
    }
    // Сдвиг ложится на поле БЛОК-СТАРТА и СКЛАДЫВАЕТСЯ с полем слоя — ровно
    // так же, как в эталонах: `block-position-margins-vrl` задаёт слою
    // `margin-right: 45px`, а его эталон пишет `margin-right: 41px`
    // = 45 + (−4); у `-vlr` то же на левом краю — `margin-left: 11px`
    // = 15 + (−4).
    let (mt, ml, mr) = (
        own(
            first.reset_box_layout.margin.top,
            inherited.reset_box_layout.margin.top,
        ),
        own(
            first.reset_box_layout.margin.left,
            inherited.reset_box_layout.margin.left,
        ),
        own(
            first.reset_box_layout.margin.right,
            inherited.reset_box_layout.margin.right,
        ),
    );
    style.reset_box_layout.margin.top = Some(Len::Px(if vert { mt } else { mt + top }));
    style.reset_box_layout.margin.bottom = Some(Len::Px(own(
        first.reset_box_layout.margin.bottom,
        inherited.reset_box_layout.margin.bottom,
    )));
    style.reset_box_layout.margin.left = Some(Len::Px(if vert && !block_rl {
        ml + top
    } else {
        ml + indent
    }));
    style.reset_box_layout.margin.right =
        Some(Len::Px(if vert && block_rl { mr + top } else { mr }));
    // Знак встаёт ЗА ведущими пробелами: внутренний отступ строки флоата =
    // отступ абзаца + их ширина (в rtl отступ идёт от правого края коробки,
    // и пробелы остаются у начала строки). Без ведущих пробелов отступ
    // наследуется, как раньше.
    if lead_w > 0.0 {
        style.inh.text_flow.text_indent = Some(Len::Px(para_indent + lead_w));
    }
    style
}
