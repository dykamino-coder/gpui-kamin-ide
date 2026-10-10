//! Выбор текстовой раскладки или ряда слов и атомов.

use super::native_paragraph;
use crate::layout::writing_mode::upright_in_mixed;
use crate::render::*;
use crate::style::apply::apply;
use crate::style::computed::Computed;
use crate::style::values::value::Len;
use crate::text::inline;
use crate::text::text_box::normal_fraction;
use gpui::{AnyElement, IntoElement, ParentElement, SharedString, Styled, div};

#[allow(clippy::too_many_arguments)]
pub(crate) fn route_pieces(
    pieces: Vec<inline::Piece>,
    inherited: &Computed,
    opts: &RenderOpts,
    native_request: Option<&native_paragraph_route::Request<'_>>,
    indent: crate::text::paragraph::Indent,
    clamp_budget: Option<usize>,
    clamp_tag: Option<(u64, u32)>,
    line_atoms: Vec<(
        usize,
        AnyElement,
        crate::text::paragraph::AtomAlign,
        crate::text::paragraph::RubyExtents,
    )>,
) -> AnyElement {
    let edges = edge_pieces(&pieces, inherited, opts);
    if inline::single_block(&pieces, opts.base_size())
        && let Some((text, runs)) = inline::text_and_runs(&pieces, &opts.text)
    {
        // `word-space-transform` смотрит на СОСЕДЕЙ точки переноса, а они
        // сплошь и рядом лежат в других кусках (`あ<wbr>い` — это три куска:
        // текст, точка, текст). По кускам преобразование их не видит, поэтому
        // идёт по собранному тексту абзаца. Замена знак-в-знак: нулевой
        // пробел и идеографический занимают в UTF-8 одинаково, поэтому
        // прогоны не съезжают.
        // Строка растёт под самый крупный кусок — как коробка строки в CSS.
        // Иначе крупный `<span>` вылезал бы на соседние строки.
        let biggest = inline::max_font_size(&pieces, own_size(inherited, opts), opts.base_size());
        // Прогон без своего кегля набирается кеглем АБЗАЦА, а им здесь стоит
        // самый крупный кусок: текст блока без объявленного `font-size` рядом
        // с крупным `<span>` вырастал до его кегля (`c43-rpl-ibx-000`: вся
        // строка в 3.75em). Свой кегль такого куска — кегль блока.
        let boxes = line_box_spans(&pieces, &edges, inherited, opts);
        let mut runs = runs;
        if (!line_atoms.is_empty() || !edges.is_empty() || boxes.is_some())
            && crate::render::paragraph::paragraph_style(inherited)
                .text_fit
                .is_none()
        {
            let own = own_size(inherited, opts);
            if biggest != own {
                for run in runs.iter_mut().filter(|r| r.font_size.is_none()) {
                    run.font_size = Some(gpui::px(own));
                }
            }
        }
        let mut opts = opts.clone();
        if biggest != opts.base_size() {
            opts.text.line_height = gpui::px(biggest * normal_fraction(inherited, &opts)).into();
        }
        let opts = &opts;
        // Selection controls handlers, while native text retains its layout and paint.
        let wrap_rules = crate::text::paragraph::rules(inherited);
        let native = wrap_rules.is_some()
            && native_request
                .is_some_and(|request| request.accepts(&pieces, !line_atoms.is_empty()));
        let selectable =
            inherited.no_select != Some(true) && inherited.pointer_events_none != Some(true);
        if !native && !selectable {
            return gpui::StyledText::new(SharedString::from(text))
                .with_ligature_breaking(false)
                .with_runs(runs)
                .into_any_element();
        }
        // Native construction remains available without installing selection handlers.
        if let Some(wrap) = wrap_rules {
            return native_paragraph(
                inherited,
                opts,
                native_request,
                native,
                selectable,
                text,
                runs,
                pieces,
                edges,
                boxes,
                biggest,
                wrap,
                indent,
                clamp_budget,
                clamp_tag,
                line_atoms,
            );
        }
    }
    // Ряд из слов и атом сходятся по базовой линии по РАЗНЫМ правилам: GPUI
    // базовой линии текста в taffy не отдаёт вовсе (`vendor/gpui`), и для
    // куска текста taffy берёт НИЖНИЙ край коробки
    // (`vendor/taffy/src/compute/flexbox.rs:1704`, `height + margin.bottom`),
    // а вложенный атом свою первую базовую линию пропагирует
    // (`flexbox.rs:405`). Кусок заявляет 1.0em, атом — 0.8em: атом тонет на
    // спуск шрифта, а короб строки растёт до 1.2em вместо `line-height`
    // (§10.8). Замерено зондом `target/probe/atom-probe2.html`: при
    // `font: 100px/1 Ahem` короб 120, атом на +20.
    let mut render_text = |t: String, style: &Computed| -> AnyElement {
        // Стоячие знаки в вертикальном письме (`text-orientation: mixed`,
        // CJK): набор идёт вертикальными формами шрифта — возможность `vert`
        // подставляет глиф, а продвижение берётся из его вертикальных метрик
        // (css-writing-modes-3 §7.3, реализация в DirectWrite-слое). Пока
        // только для кусков, стоячих ЦЕЛИКОМ: смешанный кусок потребовал бы
        // резки на прогоны по ориентации.
        let vert_style;
        let style = if style.rotated_line == Some(true)
            && style.sideways != Some(true)
            && t.chars().any(upright_in_mixed)
            && t.chars().all(|c| c.is_whitespace() || upright_in_mixed(c))
        {
            let mut s = style.clone();
            s.font_features.push(("vert".into(), 1));
            vert_style = s;
            &vert_style
        } else {
            style
        };
        // На кусок текста идут ТОЛЬКО текстовые свойства: фон, отступы и
        // рамка принадлежат абзацу целиком, а не каждому его слову.
        let d = apply(div(), &style.text_only())
            .max_w_full()
            .child(SharedString::from(t.clone()));
        d.into_any_element()
    };
    // Начальное значение `text-align` — `start`, а он при письме справа налево
    // означает ПРАВЫЙ край. Без этого ряд из слов оставался слева, и строка
    // расходилась с абзацем-соседом.
    let align = inherited
        .text_align
        .unwrap_or(crate::style::computed::TextAlign::Start)
        .physical(inherited.rtl == Some(true));
    inline::as_wrapped_row(
        pieces,
        inherited.vertical_align,
        Some(align),
        inherited.rtl == Some(true),
        match inherited.text_indent {
            Some(Len::Px(v)) => v,
            // Ряд из слов долю и прежде не применял (`Pct` идёт нулём); у
            // смеси берутся хотя бы точки — как у чистых точек.
            Some(Len::Calc(i)) => crate::style::values::value::calc_get(i)
                .pct_px()
                .map_or(0.0, |(_, px)| px),
            _ => 0.0,
        },
        inherited.nowrap == Some(true),
        &mut render_text,
        inherited.vertical != Some(true) && inherited.rotated_line != Some(true),
    )
}
