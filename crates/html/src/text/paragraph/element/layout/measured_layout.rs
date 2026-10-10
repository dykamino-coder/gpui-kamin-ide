//! Measured layout for layout; split out to keep the owning module within 250 lines.

use crate::text::paragraph::*;
use gpui::{App, Pixels, Window, px, size};

#[allow(clippy::too_many_arguments)]
pub(crate) fn measured_layout(
    mut probe: Paragraph,
    atom_fit: std::rc::Rc<std::cell::RefCell<crate::text::paragraph::atom_fit::AtomFit>>,
    known: gpui::Size<Option<Pixels>>,
    available: gpui::Size<gpui::AvailableSpace>,
    vertical: bool,
    vertical_inline: bool,
    ortho_limit: Option<Pixels>,
    line_height: Pixels,
    ruby_base_sink: &Option<std::rc::Rc<std::cell::Cell<Option<f32>>>>,
    window: &mut Window,
    _cx: &mut App,
) -> gpui::MeasuredContent {
    // Ширина атома «по содержимому» — от содержащего блока, то
    // есть от ширины самого абзаца (CSS 2.1 §10.3.9), см. `atom_fit`.
    if !vertical && !atom_fit.borrow().is_empty() {
        let avail = match (known.width, available.width) {
            (Some(w), _) | (None, gpui::AvailableSpace::Definite(w)) => f32::from(w),
            (None, gpui::AvailableSpace::MinContent) => 0.0,
            (None, gpui::AvailableSpace::MaxContent) => f32::INFINITY,
        };
        let fitted = atom_fit.borrow_mut().fit(avail, window, _cx);
        probe.atom_fit = atom_fit.clone();
        probe.apply_atom_fit(&fitted);
    }
    // Предел переноса берётся ПО ОСИ СТРОКИ: по горизонтали это
    // ширина коробки, по вертикали — её высота. Уже решённая
    // родителем сторона сильнее доступной.
    let (known_along, space_along) = if vertical {
        (known.height, available.height)
    } else {
        (known.width, available.width)
    };
    let limit = probe.measured_inline_limit(known_along, space_along, ortho_limit, window);
    // Кегль подбирается ДО замера: коробка считается уже по
    // подобранному, иначе её высота не сойдётся с отрисовкой.
    // Подбирать есть смысл только под ЗАДАННЫЙ размер строки:
    // когда его нет, коробка растёт под текст, и заполнять нечего
    // (иначе кегль улетал в размер окна — `text-fit/writing-mode`).
    if let Some(w) = known_along {
        probe.apply_fit(w, window);
    }
    if vertical {
        probe.run_metrics = probe.measure_runs(window);
    }
    let lines = probe.split(limit, window);
    // Ширина — по самой длинной строке. Вся отведённая ширина
    // берётся только под выключку по ширине: там остаток строки
    // раздаётся пробелам, и без полной колонки раздавать нечего.
    // В остальных случаях абзац обтягивает текст, иначе ломается
    // размер по содержимому у родителя.
    // Отступ строки входит в её место в колонке: коробка по
    // содержимому обязана вместить и его. Отрицательный уходит в
    // поле и ширины не требует, поэтому в ноль он и упирается.
    // У абзаца с атомами отрицательный отступ ширину по содержимому
    // УМЕНЬШАЕТ (css-text-3 §7.1: отступ входит в строку; доля при
    // замере — ноль): `text-indent: calc(50% - 3px)` у флоата с
    // атомом 10px даёт 7px (`calc-text-indent-intrinsic-1`). Так
    // мерил и прежний ряд слов; у текстового абзаца — как было.
    let atoms_in = !probe.atom_boxes.is_empty();
    // Under a min-content constraint a NEGATIVE indent of a
    // paragraph with atoms narrows only the first piece; the lines
    // split at the min-content width packed more onto the
    // indented first line (`text-indent-intrinsic-003/004`).
    let neg_indent = atoms_in
        && !vertical
        && known_along.is_none()
        && probe.indent.px < 0.0
        && !probe.indent.hanging;
    let min_indented = neg_indent
        .then(|| match space_along {
            gpui::AvailableSpace::MaxContent => None,
            _ => Some(probe.min_content_indented(window)),
        })
        .flatten();
    let content = lines
        .iter()
        .map(|l| {
            if atoms_in {
                (l.width + l.indent).max(px(0.))
            } else {
                l.width + l.indent.max(px(0.))
            }
        })
        .fold(px(0.), |a: Pixels, b| if b > a { b } else { a });
    // A ruby base unit (nowrap) reports its content width for the
    // annotation overhang (`lay_atoms`).
    if let Some(sink) = &ruby_base_sink {
        sink.set(Some(f32::from(content)));
    }
    // Fit-content is max(min-content, min(max-content, available))
    // (css-sizing-3 §5.1): with a negative indent the max-content
    // line can be NARROWER than the widest piece of a later line,
    // and the min-content then wins (`text-indent-intrinsic-004`).
    let content = match (min_indented, space_along) {
        (Some(min), gpui::AvailableSpace::MinContent) => min,
        (Some(min), _) if min > content => min,
        _ => content,
    };
    // Native vertical lines report their inline extent to the
    // band host's intrinsic probe, like `VerticalText` does: the
    // box itself stretches to the window along the line axis.
    if vertical {
        crate::text::vertical::VT_INLINE_MAX.with(|c| {
            if let Some(v) = c.get() {
                c.set(Some(v.max(f32::from(content))));
            }
        });
    }
    let width = known_along.unwrap_or(content);
    // Шире отведённого коробка не бывает: у абзаца блочного уровня
    // ширина ограничена содержащим блоком, и без этого предела
    // длинная сохранённая строка растягивала коробку и вылезала
    // за неё вместо переноса.
    let width = match space_along {
        gpui::AvailableSpace::Definite(w) if width > w => w,
        _ => width,
    };
    // Абзац с атомами, перенесённый МЯГКО, занимает всё отведённое
    // место: ширина «по содержимому» — это min(max-content,
    // max(min-content, доступное)) (CSS 2.1 §10.3.5, css-sizing-3
    // §5.1 fit-content), а не самая длинная строка после переноса.
    // Так мерил и прежний гибкий ряд с переносом, и коробка
    // `width: fit-content(100px)` из двух `inline-block` по 60px
    // выходила 60 вместо 100 (`fit-content-length-percentage-*`).
    let width = match space_along {
        gpui::AvailableSpace::Definite(w)
            if known_along.is_none() && !probe.atom_boxes.is_empty() && width < w =>
        {
            let full = probe
                .split(None, window)
                .iter()
                .map(|l| l.width + l.indent.max(px(0.)))
                .fold(px(0.), |a: Pixels, b| if b > a { b } else { a });
            if full > w { w } else { width }
        }
        _ => width,
    };
    // Высота абзаца — сумма ШАГОВ строк: обычно это ровно
    // `line_height`, но строка со сдвинутым по вертикали куском
    // выше на его вылет (CSS 2.1 §10.8).
    // Сдвиг ПОСЛЕДНЕЙ строки от первой: шаги всех строк перед ней
    // плюс разница их надбавок сверху (у первой базовой надбавка
    // своей строки не учтена — последняя считается тем же отсчётом,
    // и у однострочного абзаца обе совпадают).
    let mut last_shift = px(0.);
    // Верхняя надбавка ПЕРВОЙ строки опускает её базовую линию:
    // атом выше струта сдвигает текст строки вниз, и базовая
    // абзаца (для `inline-block` — его собственная, §10.8.1)
    // обязана уехать вместе с ним.
    let mut first_above = px(0.);
    let across = {
        probe.lines = lines.clone();
        let pads = probe.line_padding();
        if !probe.atom_boxes.is_empty() || !probe.box_spans.is_empty() {
            first_above = px(pads.first().map_or(0.0, |p| p.0));
        }
        if pads.len() > 1 {
            let before: f32 = pads[..pads.len() - 1].iter().map(|(a, b)| a + b).sum();
            let own = pads[pads.len() - 1].0 - pads[0].0;
            last_shift = line_height * (pads.len() - 1) as f32 + px(before + own);
        }
        let extra: f32 = pads.iter().map(|(a, b)| a + b).sum();
        (if vertical {
            probe.line_height
        } else {
            line_height
        }) * lines.len() as f32
            + px(extra)
    };
    if vertical {
        let width = if vertical_inline {
            limit.unwrap_or(width)
        } else {
            width
        };
        return probe.vertical_content_baselines(size(
            known.width.unwrap_or(across),
            known.height.unwrap_or(width),
        ));
    }
    let baseline = probe.measured_first_baseline(line_height, first_above, window);
    let last_baseline = baseline.map(|b| b + last_shift);
    gpui::MeasuredContent {
        first_y: baseline,
        last_y: last_baseline,
        lines_y: Some(probe.content_line_baselines(baseline)),
        ..gpui::MeasuredContent::new(size(
            known.width.unwrap_or(width),
            known.height.unwrap_or(across),
        ))
    }
}
