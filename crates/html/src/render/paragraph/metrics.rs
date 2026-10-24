//! Метрики абзаца и границы line box для атомарных элементов.

use crate::layout::options::RenderOpts;
use crate::layout::options::own_size;
use crate::layout::writing_mode::rotated_atom;
use crate::render::paragraph::edges::in_edge;
use crate::style::computed::Computed;
use crate::style::values::value::Len;
use crate::text::inline;
use crate::text::text_box::normal_fraction;
use crate::tolerance::COORD_EPS;

/// Самый крупный кегль и наибольшая `line-height` кусков ВНЕ краевых: струт
/// строки и её базовая линия от прижатых к краю не зависят (§10.8.1).
pub(crate) fn flow_metrics(
    pieces: &[inline::Piece],
    edges: &[(std::ops::Range<usize>, bool, f32)],
    inherited: &Computed,
    opts: &RenderOpts,
) -> (f32, f32) {
    let strut = own_size(inherited, opts);
    let fraction = normal_fraction(inherited, opts);
    let em_base = opts.base_size();
    let (mut size_max, mut lh_max) = (
        strut,
        crate::text::metrics::normal_px(fraction, strut, opts.cx.vcx()),
    );
    let mut at = 0usize;
    for p in pieces {
        let inline::Piece::Text { text, style } = p else {
            continue;
        };
        let r = at..at + text.len();
        at = r.end;
        if in_edge(edges, &r) {
            continue;
        }
        let size = match style.font_size {
            Some(Len::Px(v)) => v,
            Some(Len::Em(k)) => k * em_base,
            _ => strut,
        };
        let own = match style.line_height {
            Some(Len::Px(v)) => v,
            Some(Len::Pct(k)) | Some(Len::Em(k)) => k * size,
            _ => crate::text::metrics::normal_px(fraction, size, opts.cx.vcx()),
        };
        size_max = size_max.max(size);
        lh_max = lh_max.max(own);
    }
    (size_max, lh_max)
}

/// Строчные коробки кусков для построчной высоты (`Paragraph::line_boxes`,
/// CSS 2.1 §10.8.1): отрезок байт → `line-height` куска в точках, плюс
/// `line-height` струта блока. `None` — все куски одного кегля, гарнитуры и
/// высоты строки: строка тогда и так равна струту, абзац идёт прежним путём.
///
/// Прежде высота строки на ВЕСЬ абзац бралась по самому крупному куску
/// (`max_line_height`, `k × biggest`): одна крупная буква растила все строки,
/// а базовая линия мелкого текста в строке с крупным стояла посередине.
// Each span carries a text range, a baseline offset and a two-axis displacement.
#[allow(clippy::type_complexity)]
pub(crate) fn line_box_spans(
    pieces: &[inline::Piece],
    edges: &[(std::ops::Range<usize>, bool, f32)],
    inherited: &Computed,
    opts: &RenderOpts,
) -> Option<(Vec<(std::ops::Range<usize>, f32)>, f32)> {
    let vertical = inherited.wm().vertical() || inherited.wm().turned();
    // Sideways lines use the same alphabetic metrics and per-run half-leading
    // as horizontal lines (Blink inline_box_state.cc:110-150). Only the physical
    // projection differs. Central baseline runs need their own metric table.
    let alphabetic = inherited.wm().alphabetic_baseline();
    if (vertical && !alphabetic)
        || crate::render::paragraph::paragraph_style(inherited)
            .text_fit
            .is_some()
    {
        return None;
    }
    let own = own_size(inherited, opts);
    let strut = match inherited.line_height {
        Some(Len::Px(v)) => v,
        Some(Len::Pct(k)) | Some(Len::Em(k)) => k * own,
        None | Some(Len::Auto) => {
            crate::text::metrics::normal_px(normal_fraction(inherited, opts), own, opts.cx.vcx())
        }
        _ => return None,
    };
    let mut out: Vec<(std::ops::Range<usize>, f32)> = Vec::new();
    let mut mixed = inherited.first_line.is_some();
    let mut at = 0usize;
    for p in pieces {
        let inline::Piece::Text { text, style } = p else {
            continue;
        };
        let r = at..at + text.len();
        at = r.end;
        if text.is_empty() || in_edge(edges, &r) {
            continue;
        }
        let size = match style.font_size {
            Some(Len::Px(v)) => v,
            Some(Len::Em(k)) => k * opts.base_size(),
            None => own,
            _ => return None,
        };
        let lh = match style.line_height {
            Some(Len::Px(v)) => v,
            Some(Len::Pct(k)) | Some(Len::Em(k)) => k * size,
            None | Some(Len::Auto) => {
                crate::text::metrics::normal_px(normal_fraction(style, opts), size, opts.cx.vcx())
            }
            _ => return None,
        };
        if (size - own).abs() > COORD_EPS
            || (lh - strut).abs() > COORD_EPS
            || style.font_family != inherited.font_family
        {
            mixed = true;
        }
        out.push((r, lh));
    }
    mixed.then_some((out, strut))
}

/// Можно ли абзацу ставить атомы в свою строку: горизонтальное письмо слева
/// направо, без раздачи по ширине (места атомов считаются от продвижения
/// распорки, а растяжку пробелов `Paragraph` раздаёт уже при отрисовке) и с
/// выделяемым текстом — путь `StyledText` атомов не несёт.
///
/// Абзац с руби идёт в строку и при `rtl`: иначе он остаётся в
/// ряду, где строка под аннотацию не растёт, а такой же абзац слева направо
/// растёт (`ruby-bidi-002`: эталон из ltr-абзаца с `text-align: right`).
pub(crate) fn atoms_fit_line(
    inherited: &Computed,
    ruby: bool,
    cx: &crate::layout::cx::LayoutCx,
) -> bool {
    !inherited.wm().vertical()
        // A rotated paragraph of atoms only is a row whose end edge sits on
        // the end of the paragraph box (`paragraph_routed`, pure-atom
        // branch); a line box would add the strut's descent below the atoms
        // (`wm-propagation-body-035`: the caption image rose by the descent).
        && (!inherited.wm().turned() || rotated_atom::text_turn(cx))
        && (ruby || !inherited.wm().rtl())
        && inherited.no_select != Some(true)
        && inherited.pointer_events_none != Some(true)
        && crate::text::paragraph::align_for(inherited) != crate::text::paragraph::Align::Justify
        // Обрыв строки многоточием (`text-overflow: ellipsis`) режет текст по
        // знакам, а распорку атома знаком не считает: атом обрывался не там
        // (`text-overflow-016`, `text-overflow-ruby`) — такой абзац в ряду.
        && inherited.ellipsis != Some(true)
}
