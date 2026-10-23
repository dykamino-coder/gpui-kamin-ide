//! Выравнивание атомарных inline-коробок относительно строки.

use crate::dom::{Element, Node};
use crate::layout::options::RenderOpts;
use crate::layout::options::own_size;
use crate::layout::positioned::static_position::at_static_position;
use crate::style::computed::{Computed, Display};
use crate::style::values::value::Len;
use crate::text::ruby::ruby_role;
use crate::text::text_box::normal_fraction;

/// `vertical-align` атома для строки абзаца, если атом туда годится.
///
/// Атом раскладывается ДО замера абзаца по своему содержимому
/// (`Paragraph::lay_atoms`), поэтому в строку идут только атомы, чей размер от
/// ширины строки не зависит: без долей в размерах, полях и отступах. Абсолюты
/// (их место — щуп статической позиции), поля форм и руби остаются в ряду.
pub(crate) fn atom_line_align(
    e: &Element,
    inherited: &Computed,
    opts: &RenderOpts,
) -> Option<crate::text::paragraph::AtomAlign> {
    let vcx = opts.cx.vcx();
    use crate::text::paragraph::AtomAlign;
    let st = &*e.style.computed();
    let atomic = matches!(
        st.display,
        Some(Display::InlineBlock)
            | Some(Display::InlineTable)
            | Some(Display::InlineFlex)
            | Some(Display::InlineGrid)
    ) && st.inline_display != Some(true);
    let replaced = matches!(
        e.tag.as_str(),
        "img" | "svg" | "canvas" | "video" | "embed" | "object" | "iframe"
    );
    // Контейнер руби — тоже атом строки: колонки баз с аннотациями
    // монолитны (§3.5), а строка обязана вырасти под аннотацию (§3.4), чего
    // гибкий ряд слов не умеет. Прочие роли (база, аннотация вне контейнера)
    // остаются в ряду.
    let ruby = ruby_role(e) == Some(crate::style::computed::RubyRole::Container);
    if !(atomic || replaced || ruby) || (st.ruby_role.is_some() && !ruby) {
        return None;
    }
    // Внутри `line-clamp` руби остаётся в ряду: вычислитель среза
    // (`text::clamp::geometry::ClampGeometry`) делит высоту абзаца на РАВНЫЕ строки, а строка с
    // аннотацией выше прочих — в строке срез встаёт строкой выше
    // (`line-clamp-auto-with-ruby-002`, руби за срезом). Вернуть в строку
    // можно только вместе с настоящими низами строк в `ClampEntry`.
    // Ортогональный поток внутри атома меряется от ДОСТУПНОГО места (§7.3
    // css-writing-modes-3), а замер «по содержимому» его не даёт: коробка с
    // `writing-mode: vertical-*` и строчной стороной `auto` внутри атома
    // выходила другой высоты (`inline-box-orthogonal-child-with-margins`).
    // Элемент сетки и гибкого ряда размер берёт от дорожки/ряда, и от
    // доступного места не зависит — такой атом остаётся в строке: иначе абзац
    // теста с `vertical-rl`-элементами сетки шёл прежним рядом, а эталон из
    // простых `inline-block` — строкой (`grid-container-baseline-
    // synthesized-001..004`).
    fn has_vertical(nodes: &[Node], in_box_layout: bool) -> bool {
        nodes.iter().any(|n| match n {
            Node::Element(k) => {
                let sized_by_parent = in_box_layout || k.style.computed().height.is_some();
                // Ломает замер только ортогональный ФЛОАТ (его ширина «по
                // содержимому» берётся от доступного места); ортогональный
                // блок в потоке раскладывается одинаково, и исключать атом
                // ради него значило вести тест рядом, а эталон строкой
                // (`baseline-with-orthogonal-flow-001`).
                let floated = k.style.computed().float.is_some_and(|f| f != 0);
                (k.style.cascaded().writing_mode.is_some() && !sized_by_parent && floated)
                    || has_vertical(&k.children, box_layout(&k.style.computed()))
            }
            _ => false,
        })
    }
    fn box_layout(c: &Computed) -> bool {
        matches!(
            c.display,
            Some(Display::Grid)
                | Some(Display::InlineGrid)
                | Some(Display::Flex)
                | Some(Display::InlineFlex)
        )
    }
    // Вертикальный inline-block в горизонтальной строке сохраняет физические
    // размеры (Writing Modes 4 §4.3); own_box подавляет его содержательную
    // baseline. Обратная ортогональность требует адаптера повёрнутой строки:
    // обычный атом и text-combine там пока идут общим прежним путём.
    let orthogonal_block = st.display == Some(Display::InlineBlock)
        && st.wm().vertical()
        && !inherited.wm().vertical()
        && !inherited.wm().turned();
    // Своё письмо атома — объявленное (вычисленное наследуется).
    if (e.style.cascaded().writing_mode.is_some() && !box_layout(st) && !orthogonal_block)
        || has_vertical(&e.children, box_layout(st))
    {
        return None;
    }
    // Замещаемый с `aspect-ratio`: соотношение разрешается от ДОСТУПНОГО
    // места, а замер по содержимому его не даёт (`zero-or-infinity-006`:
    // `aspect-ratio: 0/1` давал другую высоту).
    if replaced && (st.aspect_ratio.is_some() || st.aspect_ratio_auto.is_some()) {
        return None;
    }
    // Абсолютная замещаемая коробка с заданными краями места в строке не
    // занимает (`atom_element` отдаёт пустышку нулевого размера): атомом
    // строки она абзац с текстового пути не уводит. Прежде ряд слов набирал
    // соседний текст шире и ниже, чем эталон с тем же текстом без картинки
    // (`background-bg-pos-204-ref`).
    if matches!(
        st.position,
        Some(crate::style::computed::Position::Absolute)
            | Some(crate::style::computed::Position::Fixed)
    ) {
        if replaced && !at_static_position(st) {
            return Some(AtomAlign::Shift(0.0));
        }
        return None;
    }
    let fixed = |l: Option<Len>| !matches!(l, Some(Len::Pct(_)) | Some(Len::Calc(_)));
    let sides = |s: &crate::style::computed::Sides| {
        fixed(s.top) && fixed(s.right) && fixed(s.bottom) && fixed(s.left)
    };
    if ![
        st.width,
        st.height,
        st.min_width,
        st.min_height,
        st.max_width,
        st.max_height,
    ]
    .into_iter()
    .all(fixed)
        || !sides(&st.margin)
        || !sides(&st.padding)
    {
        return None;
    }
    // Сдвиги — как у текстового куска (`inline::shift_spans`), но от кегля
    // САМОГО атома; ось подъёма смотрит вверх.
    let merged = crate::style::cascade::materialize::box_style(inherited, &e.style, vcx);
    let size = match merged.font_size {
        Some(Len::Px(v)) => v,
        _ => own_size(inherited, opts),
    };
    Some(match st.vertical_align {
        Some(crate::style::computed::Align::Start) => AtomAlign::Top,
        Some(crate::style::computed::Align::End) => AtomAlign::Bottom,
        Some(crate::style::computed::Align::Center) => AtomAlign::Middle,
        _ => match st.vertical_align_text {
            Some(true) => AtomAlign::TextTop,
            Some(false) => AtomAlign::TextBottom,
            None => {
                if let Some(v) = st.vertical_shift_px {
                    AtomAlign::Shift(-v)
                } else if let Some(l) = st.vertical_shift_len {
                    let family = merged.font_family.unwrap_or_default();
                    AtomAlign::Shift(crate::text::metrics::spacing_px(
                        Some(l),
                        &family,
                        size,
                        vcx,
                    ))
                } else if let Some(k) = st.vertical_shift {
                    AtomAlign::Shift(-k * size)
                } else if let Some(k) = st.vertical_shift_pct {
                    // Процент — от `line-height` самого атома (§10.8.1).
                    let own = match merged.line_height {
                        Some(Len::Px(v)) => v,
                        Some(Len::Pct(f)) | Some(Len::Em(f)) => f * size,
                        _ => crate::text::metrics::normal_px(
                            normal_fraction(&merged, opts),
                            size,
                            vcx,
                        ),
                    };
                    AtomAlign::Shift(-k * own)
                } else {
                    AtomAlign::Shift(0.0)
                }
            }
        },
    })
}
