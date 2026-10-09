//! Инлайн-поток: текст с вкраплениями `<b>`, `<a>`, `<code>` внутри абзаца.
//!
//! Здесь лежит главное расхождение GPUI с HTML. В браузере абзац — это поток
//! строк, куда встраиваются куски любого размера, и строка растёт под самый
//! высокий из них. В GPUI текстовый блок — лист раскладки: размер шрифта у него
//! ОДИН на весь блок (`shape_text` принимает `font_size` скаляром), и метрики
//! строки тоже одни.
//!
//! Отсюда две стратегии, и выбор между ними делается по факту содержимого:
//!
//! * **Один блок текста.** Если все куски абзаца одного размера и ни один не
//!   несёт собственного бокса (фон, рамка, отступы), они собираются в один
//!   `StyledText` с прогонами. Перенос строк тогда честный — как в браузере,
//!   по словам, сквозь границы `<b>` и `<a>`.
//! * **Строка из элементов.** Иначе куски становятся отдельными элементами в
//!   гибкой строке с переносом. Перенос идёт по кускам, а не по словам внутри
//!   них — это заметно на длинном `<code>`, но зато размеры и боксы честные.
//!
//! Первая ветка покрывает подавляющее большинство: жирный, курсив, ссылка,
//! цвет. Вторая включается там, где без неё пришлось бы врать про размер.

pub(crate) mod emphasis;
pub use emphasis::emphasis_spans;

pub(crate) mod first_letter;
pub(crate) mod first_line_background;
pub(crate) mod empty_inline;
pub use first_letter::split_first_letter;

pub(crate) mod tabs;
pub(crate) mod lang_case;
pub(crate) mod physical_sides;
pub(crate) mod physical_projection;
pub(crate) mod inline_spacing;
pub(crate) mod bidi_controls;
pub use bidi_controls::bidi_marks;
pub use tabs::tab_stops;

pub(crate) mod text_case;

use crate::dom::{Element, Node};
use crate::style::computed::Computed;
use crate::style::values::value::Len;
use gpui::{AnyElement, IntoElement};
pub(crate) mod collect;
pub(crate) use crate::text::inline::collect::*;
pub(crate) mod hyphenate;
pub use crate::text::inline::hyphenate::*;
pub(crate) mod spans;
pub use crate::text::inline::spans::*;
pub(crate) mod spacers;
pub use crate::text::inline::spacers::*;
pub(crate) mod whitespace;
pub use crate::text::inline::whitespace::*;
pub(crate) mod case;
pub use crate::text::inline::case::*;
pub(crate) mod runs;
pub use crate::text::inline::runs::*;

/// Кусок инлайн-содержимого: либо текст со своим стилем, либо готовый элемент
/// (картинка, кнопка — то, что текстом не является).
pub enum Piece {
    Text {
        text: String,
        style: Computed,
    },
    Atom(AnyElement),
    /// Элемент ВНЕ потока строки: места не занимает, но рисуется там, где
    /// стоит в тексте (абсолютный элемент на статической позиции). В отличие
    /// от `Atom` не выводит абзац из текстового пути — иначе строка теряет
    /// пробелы и перенос по словам (`line-breaking-018`).
    ///
    /// Второе поле — БЛОЧНЫЙ уровень гипотетической коробки: её статическая
    /// позиция — начало СЛЕДУЮЩЕЙ строки, а не точка в текущей (CSS 2.1
    /// §10.6.4/§10.3.7 «if position had been static»; Blink
    /// `LogicalStaticPosition` блочного OOF в строчном контексте — блок-
    /// начало после текущей строки, строчное начало — край содержимого).
    Overlay(AnyElement, OverlayAt),
}

/// Как кусок вне потока садится на своё место в тексте (`lines.rs`).
#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub struct OverlayAt {
    /// Гипотетическая коробка БЛОЧНАЯ: место — начало следующей строки.
    pub next_line: bool,
    /// Относительный сдвиг строчных предков по ДО-ПОВОРОТНОЙ оси y в
    /// повёрнутом абзаце. Прикладывается в `lines.rs` вместе с округлением
    /// до целой физической точки ВНИЗ — так же, как глиф соседнего текста
    /// (`window.rs: paint_glyph` берёт `floor`, раскладка — `round`).
    pub rot_dy: f32,
    /// То же по до-поворотной x (строчная ось). Отбивкой его не задать:
    /// при `direction: rtl` `inset-inline-start` даёт ОТРИЦАТЕЛЬНЫЙ сдвиг, а
    /// отрицательная отбивка обнуляется — коробка оставалась на месте, текст
    /// уезжал на 2px, и полоса красного проступала (`static-position/
    /// v{lr,rl}-rtl-*`, `cb`-случаи).
    pub rot_dx: f32,
    /// Строчный абсолют на статической точке повёрнутого rtl-абзаца: сторону,
    /// которой коробка висит на точке, решает УРОВЕНЬ bidi в точке
    /// (`lines.rs: rtl_level_at`), а не направление блока. Гипотетическая
    /// коробка стоит В ПРОГОНЕ: в ltr-прогоне (латиница, Ahem) она уходит от
    /// точки вправо, и строчное начало rtl-блока — её правый край — лежит на
    /// ширину правее точки (CSS 2.1 §10.3.7 «set 'right' to the static
    /// position»: статическая позиция — край гипотетической коробки).
    pub bidi_hang: bool,
    /// Абсолют с краями по ОБЕИМ осям, чей содержащий блок — позиционированный
    /// строчный предок в этом же абзаце (`render.rs: atom_element_raw`).
    pub edges: bool,
    /// Содержащий блок такого абсолюта — строчная коробка (`mark_inline_cb`).
    pub cb: Option<InlineCb>,
    /// Пустой кусок-метка края содержимого такой коробки `(id, начало?)`:
    /// байтовые края считаются по ГОТОВЫМ кускам (`overlays`) — схлопывание
    /// пробелов на границах кусков идёт уже после сбора.
    pub cb_marker: Option<(u32, bool)>,
}

/// Содержащий блок из фрагментов строчной коробки (CSS 2.1 §10.1 п.4.1;
/// Blink `out_of_flow_layout_part.cc` `ComputeInlineContainingBlocks`:
/// начало — верхний строчно-начальный угол первого фрагмента, конец —
/// нижний строчно-конечный угол последнего, отрицательный размер — ноль).
/// Байтовые края СОДЕРЖИМОГО коробки — относительно места самого куска.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InlineCb {
    /// Метка коробки (`OverlayAt::cb_marker`).
    pub id: u32,
    pub start: isize,
    pub end: isize,
    /// Отбивка коробки `[top, right, bottom, left]`: содержащий блок — край
    /// отбивки (§10.1 п.4: «padding edges»).
    pub pad: [f32; 4],
    /// Относительный сдвиг коробки и её строчных предков (§9.4.3).
    pub shift: (f32, f32),
}

thread_local! {
    /// Глубина позиционированных строчных предков текущего сбора кусков.
    pub(crate) static INLINE_CB_DEPTH: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    /// Атом, который строится прямо сейчас, лежит внутри такого предка.
    pub(crate) static ATOM_CB: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// Атом ушёл абсолютом с краями от строчного содержащего блока.
    pub(crate) static ABS_CB_TAKEN: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Строящийся атом — внутри позиционированного строчного (одноразово).
pub(crate) fn take_atom_cb() -> bool {
    ATOM_CB.with(|c| c.replace(false))
}

/// Вернуть признак `take_atom_cb` перед постройкой атома.
pub(crate) fn set_atom_cb(v: bool) {
    ATOM_CB.with(|c| c.set(v));
}

/// Отметить, что атом построен для строчного содержащего блока.
pub(crate) fn note_abs_cb() {
    ABS_CB_TAKEN.with(|c| c.set(true));
}

/// Забрать отметку `note_abs_cb`.
pub(crate) fn take_abs_cb() -> bool {
    ABS_CB_TAKEN.with(|c| c.replace(false))
}

/// Отметить куски-абсолюты с краями (`OverlayAt::edges`) содержимого
/// строчной коробки `e`, у которых содержащего блока ещё нет: ближайший
/// позиционированный предок — она.
pub(crate) fn mark_inline_cb(pieces: Vec<Piece>, e: &Element) -> Vec<Piece> {
    if !pieces
        .iter()
        .any(|p| matches!(p, Piece::Overlay(_, how) if how.edges && how.cb.is_none()))
    {
        return pieces;
    }
    static CB_ID: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(1);
    let id = CB_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let px_of = |l: Option<Len>| match l {
        Some(Len::Px(v)) => v,
        _ => 0.0,
    };
    let pad = [
        px_of(e.style.padding.top),
        px_of(e.style.padding.right),
        px_of(e.style.padding.bottom),
        px_of(e.style.padding.left),
    ];
    let marker = |start: bool| {
        Piece::Overlay(
            gpui::Empty.into_any_element(),
            OverlayAt {
                cb_marker: Some((id, start)),
                ..Default::default()
            },
        )
    };
    let mut out = Vec::with_capacity(pieces.len() + 2);
    out.push(marker(true));
    out.extend(pieces.into_iter().map(|p| match p {
        Piece::Overlay(el, how) if how.edges && how.cb.is_none() => Piece::Overlay(
            el,
            OverlayAt {
                cb: Some(InlineCb {
                    id,
                    start: 0,
                    end: 0,
                    pad,
                    shift: (0.0, 0.0),
                }),
                ..how
            },
        ),
        other => other,
    }));
    out.push(marker(false));
    out
}

/// Схлопывание пробелов ЧЕРЕЗ границу кусков (CSS 2.1 §16.6.1,
/// css-text-3 §4.1.1).
///
/// Схлопывание у нас поузловое, а ряд пробелов сплошь и рядом лежит в РАЗНЫХ
/// кусках: спека прямо оговаривает пробел «even one outside the boundary of
/// the inline containing that space». `<span>Row 1, </span>` плюс перевод
/// строки перед следующим тегом набирались как два пробела — на знак шире
/// эталона, и так на каждой границе.
///
/// Кусок из ОДНИХ пробелов ряд не разрывает: он схлопывается в предыдущий
/// пробел целиком и остаётся пустым.
///
/// ПРОБОВАЛИ И ОТКАТИЛИ: пропускать здесь и СЛУЖЕБНЫЕ куски — распорку полей
/// строчной коробки (U+FEFF) и метку границы атома (U+200B), — не сбрасывая
/// признак пробела. Замерено по семьям text/*, linebox/*, bidi-*: 0 и 0.
/// Семью `white-space-normal-*` держит не это.
pub fn collapse_across_pieces(pieces: &mut [Piece]) {
    let mut prev_space = false;
    for piece in pieces.iter_mut() {
        match piece {
            // Кусок вне потока места не занимает и ряд пробелов не рвёт.
            Piece::Overlay(..) => {}
            // Замещаемая коробка — не пробел: ряд на ней кончается.
            Piece::Atom(_) => prev_space = false,
            Piece::Text { text, style } => {
                // A zero-advance empty-box metric marker is not document
                // content and must not interrupt adjoining collapsible spaces.
                if text == SPACER && style.letter_spacing == Some(Len::Px(0.0)) {
                    continue;
                }
                // `white-space: pre*` пробелы бережёт — там схлопывать нечего.
                // `pre-line` переводы строк бережёт, а ПРОБЕЛЫ схлопывает
                // (§16.6): освобождать его от схлопки нельзя.
                if style.keep_spaces == Some(true) {
                    prev_space = false;
                    continue;
                }
                // Кусок-метка направления (`bidi_marks`: RLE/LRE/PDF вокруг
                // `<span dir>`) ряд пробелов НЕ рвёт и сам его не начинает —
                // css-text-3 §4.1 велит обрабатывать пробелы, не видя этих
                // знаков. Иначе `x <span dir=rtl> x </span> x` набирался
                // семью знаками вместо пяти (`white-space-collapsing-bidi-002`).
                if !text.is_empty() && text.chars().all(bidi_format) {
                    continue;
                }
                if prev_space {
                    let rest = text.trim_start_matches(' ');
                    if rest.len() != text.len() {
                        *text = rest.to_string();
                    }
                }
                match text.chars().next_back() {
                    Some(' ') => prev_space = true,
                    // Пустой кусок ряда не меняет: он и есть схлопнутый пробел.
                    None => {}
                    Some(_) => prev_space = false,
                }
            }
        }
    }
}

/// Собрать инлайн-куски из детей узла.
pub fn collect(
    children: &[Node],
    inherited: &Computed,
    atom: &mut dyn FnMut(&Element) -> Option<Piece>,
) -> Vec<Piece> {
    collect_with_empty_metrics(
        children,
        inherited,
        atom,
        empty_inline::has_text(children),
        &mut text_case::Context::default(),
    )
}

/// Одеть первые `at` байт абзаца в стиль первой строки (`::first-line`).
///
/// Длину первой строки считает замер: до переноса она неизвестна.
pub fn style_first_line(pieces: Vec<Piece>, at: usize, style: &Computed) -> Vec<Piece> {
    let mut out: Vec<Piece> = Vec::with_capacity(pieces.len() + 1);
    let mut seen = 0usize;
    for p in pieces {
        match p {
            Piece::Text { text, style: own } if seen < at => {
                let dress = |base: &Computed| {
                    let mut c = base.clone();
                    c.font_size = style.font_size.or(base.font_size);
                    c.color = style.color.or(base.color);
                    c.font_weight = style.font_weight.or(base.font_weight);
                    c.italic = style.italic.or(base.italic);
                    // Возможности шрифта первой строки, в том числе запрет
                    // подмены начертания (`nsyw`/`nsys`, css-fonts-4 §6.5):
                    // без них полужирный и курсив `::first-line` синтезировались
                    // вопреки `font-synthesis-*: none`. Дописываются ПОСЛЕ
                    // своих: при повторе тега побеждает первая строка.
                    c.font_features.extend(style.font_features.iter().cloned());
                    c.font_kerning = style.font_kerning.or(base.font_kerning);
                    c.font_alternates = style
                        .font_alternates
                        .clone()
                        .or_else(|| base.font_alternates.clone());
                    crate::style::computed::font_family::inherit(&mut c, style, base);
                    c.font_settings = style
                        .font_settings
                        .clone()
                        .or_else(|| base.font_settings.clone());
                    // Коробочная часть первой строки: интерлиньяж и подложка
                    // (css-pseudo-4 §4.1; first-line-line-height-001/002).
                    c.background = style.background.or(base.background);
                    // Красит подложку прогон текста, и берёт он её из
                    // `inline_bg`: слой первой строки накладывается уже ПОСЛЕ
                    // сборки кусков, когда `inline_bg` посчитан по своему
                    // стилю (`c25-pseudo-elmnt-000`: зелёной полосы не было).
                    c.inline_bg =
                        first_line_background::paint_color(base.inline_bg, style.background);
                    c.line_height = style.line_height.or(base.line_height);
                    // ПРОБОВАЛИ И ОТКАТИЛИ: переносить сюда и сдвиг по
                    // вертикали (§5.12.1 относит `vertical-align` к свойствам
                    // `::first-line`). Проба по 372 парам семей `first-line-*`
                    // и `first-letter-*`: `first-line-pseudo-012` 4.66 -> 4.94,
                    // флипов ноль. Краска поднимается, а коробка нет: высоту
                    // абзаца заявляет `float::FirstLine` (`measure_first_line`),
                    // и подъёма она не знает. Возвращаться вместе с ней.
                    c
                };
                let len = text.len();
                if seen + len <= at {
                    let dressed = dress(&own);
                    out.push(Piece::Text {
                        text,
                        style: dressed,
                    });
                } else {
                    let mut cut = at - seen;
                    while cut < text.len() && !text.is_char_boundary(cut) {
                        cut += 1;
                    }
                    out.push(Piece::Text {
                        text: text[..cut].to_string(),
                        style: dress(&own),
                    });
                    if cut < text.len() {
                        out.push(Piece::Text {
                            text: text[cut..].to_string(),
                            style: own,
                        });
                    }
                }
                seen += len;
            }
            other => {
                if let Piece::Text { text, .. } = &other {
                    seen += text.len();
                }
                out.push(other);
            }
        }
    }
    out
}

/// Наследование: в CSS вниз идут только текстовые свойства. Бокс-свойства
/// (отступы, фон) принадлежат самому элементу и вниз не передаются.
/// Устанавливает ли коробка содержащий блок для внепоточных потомков
/// (§10.1 п.4 плюс барьеры css-transforms и css-contain).
///
/// Список ШИРЕ, чем `position` не `static`: лишний барьер значит невынесенный
/// элемент, то есть прежнее поведение, а пропущенный — вынос из коробки,
/// которая обязана была его удержать.
pub(crate) fn establishes_cb(c: &Computed) -> bool {
    // Растворённый элемент коробки не даёт и содержащим блоком быть не может
    // (css-display-3 §3.2).
    if matches!(
        c.display,
        Some(crate::style::computed::Display::Contents) | Some(crate::style::computed::Display::None)
    ) {
        return false;
    }
    matches!(
        c.position,
        Some(crate::style::computed::Position::Relative)
            | Some(crate::style::computed::Position::Absolute)
            | Some(crate::style::computed::Position::Fixed)
            | Some(crate::style::computed::Position::Sticky)
    ) || c.transform.is_some()
        // css-transforms-2: `preserve-3d` is a containing block for all descendants.
        || c.preserve_3d == Some(true)
        || c.filter.is_some()
        || c.contain_paint == Some(true)
        || c.contain_layout == Some(true)
        // css-will-change-1 §2.1: обещание свойства, которое дало бы блок,
        // даёт его уже сейчас (`will-change-abspos-cb-002/003`).
        || c.will_change & (crate::style::computed::wc::CB_ABS | crate::style::computed::wc::CB_FIXED) != 0
}

/// Корень подложки (filter-effects-2 §BackdropRoot) — без корня документа:
/// его «Backdrop Root Image» и есть весь кадр. Фильтр у потомков
/// наследуется (`inherit`), но они и так под корнем.
pub(crate) fn backdrop_root(c: &Computed) -> bool {
    c.opacity.is_some_and(|o| o < 1.0)
        || c.filter.is_some_and(|f| f != crate::style::computed::Filter::neutral())
        || c.filter_ref.is_some()
        || c.mask_image.is_some()
        || c.clip_ref.is_some()
        || c.clip_polygon.is_some()
        || c.clip_shape.is_some()
        || c.blend.is_some_and(|b| b != 0)
        || c.backdrop_blur.is_some()
        || c.backdrop_color.is_some()
        // Любой `backdrop-filter`, кроме `none` (Overview.bs:119; Blink
        // paint_property_tree_builder.cc:1846), и `view-transition-name`
        // (css-view-transitions-1 Overview.bs:582).
        || c.backdrop_filter_set
        || c.vt_name
        // `clip-path` ЛЮБОЙ формой (Overview.bs:118; Blink
        // paint_property_tree_builder.cc:1887 `ClipPathClip()`): `inset()`,
        // `rect()`, `xywh()` и голая коробка разбором лежат не в
        // `clip_shape`/`clip_polygon` (`backdrop-filter-backdrop-root-clip-path-2`).
        || c.clip_inset.is_some()
        || c.clip_edges.is_some()
        || c.clip_xywh.is_some()
        || c.clip_bare_box
        || c.will_change_root
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::cascade::inherit::inherit;
    use crate::style::computed::TextAlign;
    use crate::style::css::parse_decls;

    fn styled(css: &str) -> Computed {
        let mut c = Computed::default();
        c.apply_decls(&parse_decls(css));
        c
    }

    #[test]
    fn spaces_collapse_like_html() {
        assert_eq!(normalize_spaces("  два\n\tслова  "), " два слова ");
        assert_eq!(normalize_spaces("a\n\nb"), "a b");
        // Идеографический и неразрывный пробелы — знаки, а не пробелы CSS:
        // они остаются как есть и держат свою ширину.
        assert_eq!(normalize_spaces("あ\u{3000}あ"), "あ\u{3000}あ");
        assert_eq!(normalize_spaces("a\u{00a0}b"), "a\u{00a0}b");
    }

    #[test]
    fn logical_alignment_follows_the_writing_direction() {
        // Наследование хранит ЛОГИЧЕСКОЕ значение, разворот — на отрисовке.
        // Иначе левый край, посчитанный для тела страницы, доставался бы по
        // наследству вложенному блоку справа налево.
        let parent = Computed::default();
        let ltr = inherit(&parent, &styled("text-align: start"));
        assert_eq!(ltr.text_align, Some(TextAlign::Start));
        assert_eq!(crate::text::paragraph::align_for(&ltr), crate::text::paragraph::Align::Left);
        let rtl = inherit(&parent, &styled("text-align: start; direction: rtl"));
        assert_eq!(crate::text::paragraph::align_for(&rtl), crate::text::paragraph::Align::Right);
        let rtl_end = inherit(&parent, &styled("text-align: end; direction: rtl"));
        assert_eq!(crate::text::paragraph::align_for(&rtl_end), crate::text::paragraph::Align::Left);
        // Умолчание CSS — `start`: без выключки текст справа налево прижат
        // вправо, а не влево.
        let bare = inherit(&parent, &styled("direction: rtl"));
        assert_eq!(crate::text::paragraph::align_for(&bare), crate::text::paragraph::Align::Right);
        // Наследник блока справа налево берёт сторону письма у него.
        let child = inherit(&bare, &Computed::default());
        assert_eq!(crate::text::paragraph::align_for(&child), crate::text::paragraph::Align::Right);
    }

    #[test]
    fn breaking_rules_leave_the_text_alone_for_the_own_layout() {
        // Подсказки переносчику GPUI ставятся только там, где абзац рисует
        // сам движок. Своей раскладке они бы мешали: невидимый знак стал бы
        // лишней точкой разрыва (правила считает `lines::Paragraph`).
        for css in [
            "line-break: anywhere",
            "word-break: break-all",
            "word-break: keep-all",
            "white-space: break-spaces",
        ] {
            let style = styled(css);
            assert!(crate::text::paragraph::rules(&style).is_some(), "{css}");
            assert_eq!(breakable("a b", &style), "a b", "{css}");
        }
        // Своей раскладке мягкий перенос доезжает КАК ЕСТЬ: она знает его
        // точкой разрыва и рисует на его месте знак переноса.
        assert_eq!(breakable("a\u{ad}b", &Computed::default()), "a\u{ad}b");
        assert_eq!(breakable("a\u{ad}b", &styled("hyphens: none")), "ab");
    }

    #[test]
    fn plain_word_break_is_left_alone() {
        // Умолчание не трогается: там перенос между знаками разрешён самим
        // переносчиком, вставлять нечего.
        assert_eq!(breakable("中文", &styled("word-break: normal")), "中文");
    }

    #[test]
    fn same_size_pieces_go_into_one_block() {
        let pieces = vec![
            Piece::Text {
                text: "обычный ".into(),
                style: styled(""),
            },
            Piece::Text {
                text: "жирный".into(),
                style: styled("font-weight: 700"),
            },
        ];
        assert!(single_block(&pieces, 13.0), "вес не мешает единому блоку");
    }

    #[test]
    fn different_size_stays_in_one_block() {
        let pieces = vec![
            Piece::Text {
                text: "обычный ".into(),
                style: styled(""),
            },
            Piece::Text {
                text: "крупный".into(),
                style: styled("font-size: 24px"),
            },
        ];
        // Кегль едет в прогон (патч GPUI), поэтому разный размер больше не
        // выгоняет абзац в запасную ветку из отдельных слов: раньше там
        // строки наезжали друг на друга.
        assert!(
            single_block(&pieces, 13.0),
            "иной размер собирается единым блоком"
        );
        assert_eq!(
            max_font_size(&pieces, 13.0, 13.0),
            24.0,
            "строка растёт под кусок"
        );
    }

    #[test]
    fn inheritance_carries_text_not_box() {
        let parent = styled("color: #ff0000; padding: 10px; font-size: 20px");
        let child = styled("font-weight: 700");
        let merged = inherit(&parent, &child);
        assert_eq!(merged.color.map(|c| c.r), Some(1.0), "цвет наследуется");
        assert_eq!(merged.font_size, Some(Len::Px(20.0)), "размер наследуется");
        assert_eq!(merged.font_weight, Some(700), "свой вес сохранён");
        assert_eq!(merged.padding.top, None, "отступ родителя вниз не идёт");
    }

    #[test]
    fn own_style_wins_over_inherited() {
        let parent = styled("color: #ff0000");
        let child = styled("color: #0000ff");
        assert_eq!(inherit(&parent, &child).color.map(|c| c.b), Some(1.0));
    }
}

#[cfg(test)]
mod em_tests {
    use super::*;
    use crate::style::cascade::inherit::inherit;
    use crate::style::css::parse_decls;

    fn styled(css: &str) -> Computed {
        let mut c = Computed::default();
        c.apply_decls(&parse_decls(css));
        c
    }

    #[test]
    fn em_resolves_against_the_font_size() {
        // Родитель 12px, свой размер 2em = 24px, высота 1em = 24px.
        let parent = styled("font-size: 12px");
        let child = styled("font-size: 2em; height: 1em");
        let merged = inherit(&parent, &child);
        assert_eq!(
            merged.font_size,
            Some(Len::Px(24.0)),
            "свой кегль от родителя"
        );
        assert_eq!(merged.height, Some(Len::Px(24.0)), "высота от своего кегля");

        // Внук: 2em от 24 = 48, высота 1em = 48.
        let grand = styled("font-size: 2em; height: 1em");
        let merged2 = inherit(&merged, &grand);
        assert_eq!(merged2.font_size, Some(Len::Px(48.0)));
        assert_eq!(merged2.height, Some(Len::Px(48.0)));
    }

    #[test]
    fn em_without_own_font_size_uses_the_inherited_one() {
        let parent = styled("font-size: 20px");
        let child = styled("width: 10em");
        let merged = inherit(&parent, &child);
        assert_eq!(merged.width, Some(Len::Px(200.0)));
    }
}
