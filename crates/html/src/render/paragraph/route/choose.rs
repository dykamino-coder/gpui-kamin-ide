//! Выбор пути по готовым кускам: своя раскладка строк (`native`), сплошной
//! текст без выделения или ряд из слов и атомов.

use super::native::native_paragraph;
use super::{LineAtoms, NativeRequest};
use crate::layout::options::RenderOpts;
use crate::layout::options::own_size;
use crate::layout::writing_mode::upright_in_mixed;
use crate::paint::apply::apply;
use crate::render::paragraph::edges::edge_pieces;
use crate::render::paragraph::metrics::line_box_spans;
use crate::style::computed::{BoxStyle, Computed};
use crate::text::inline;
use crate::text::text_box::normal_fraction;
use gpui::{AnyElement, IntoElement, ParentElement, SharedString, Styled, div};

#[allow(clippy::too_many_arguments)]
pub(crate) fn route_pieces(
    pieces: Vec<inline::Piece>,
    inherited: &BoxStyle,
    opts: &RenderOpts,
    native_request: Option<&NativeRequest<'_>>,
    indent: crate::text::paragraph::Indent,
    clamp_budget: Option<usize>,
    clamp_tag: Option<(u64, u32)>,
    line_atoms: LineAtoms,
) -> AnyElement {
    let vcx = opts.cx.vcx();
    let edges = edge_pieces(&pieces, inherited, opts);
    if inline::single_block(&pieces, opts.base_size())
        && let Some((text, runs)) = inline::text_and_runs(&pieces, &opts.text, opts.cx.vcx())
        // A physical atom row has no turned inline frame for a phantom static point.
        && (!text.is_empty() || !opts.cx.inline.pure_atom_paragraph)
    {
        // `word-space-transform` смотрит на СОСЕДЕЙ точки переноса, а они
        // сплошь и рядом лежат в других кусках (`あ<wbr>い` — это три куска:
        // текст, точка, текст). По кускам преобразование их не видит, поэтому
        // идёт по собранному тексту абзаца. Замена знак-в-знак: нулевой
        // пробел и идеографический занимают в UTF-8 одинаково, поэтому
        // прогоны не съезжают.
        // Строка растёт под самый крупный кусок — как коробка строки в CSS.
        // Иначе крупный `<span>` вылезал бы на соседние строки.
        let biggest = inline::max_font_size(
            &pieces,
            own_size(inherited, opts),
            opts.base_size(),
            opts.cx.vcx(),
        );
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
            opts.text.line_height = gpui::px(
                inherited
                    .numeric_line_px(opts.base_size())
                    .unwrap_or_else(|| {
                        crate::text::metrics::normal_px(
                            normal_fraction(inherited, &opts),
                            biggest,
                            vcx,
                        )
                    }),
            )
            .into();
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
    // (§10.8): при `font: 100px/1 Ahem` короб 120, атом на +20.
    let mut render_text = |t: String, style: &Computed| -> AnyElement {
        // Стоячие знаки в вертикальном письме (`text-orientation: mixed`,
        // CJK): набор идёт вертикальными формами шрифта — возможность `vert`
        // подставляет глиф, а продвижение берётся из его вертикальных метрик
        // (css-writing-modes-3 §7.3, реализация в DirectWrite-слое). Пока
        // только для кусков, стоячих ЦЕЛИКОМ: смешанный кусок потребовал бы
        // резки на прогоны по ориентации.
        let vert_style;
        let style = if style.wm().turned()
            && !style.wm().sideways()
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
        let d = apply(div(), &BoxStyle::new(style.text_only()), vcx)
            .max_w_full()
            .child(SharedString::from(t));
        d.into_any_element()
    };
    // Начальное значение `text-align` — `start`, а он при письме справа налево
    // означает ПРАВЫЙ край. Без этого ряд из слов оставался слева, и строка
    // расходилась с абзацем-соседом.
    let align = super::alignment::marker(opts).unwrap_or_else(|| {
        inherited
            .text_align
            .unwrap_or(crate::style::computed::TextAlign::Start)
            .physical(inherited.wm().rtl())
    });
    inline::as_wrapped_row(
        pieces,
        inherited.vertical_align,
        Some(align),
        inherited.wm().rtl(),
        // Ряд из слов долю не применяет (`Pct` идёт нулём); у смеси берутся
        // точки — абсолютная часть отступа, уже сведённая в `collect`.
        indent.px,
        inherited.nowrap == Some(true),
        &mut render_text,
        !inherited.wm().vertical() && !inherited.wm().turned(),
        (inherited.wm().turned()).then_some(inherited.wm().block_rl()),
        &opts.cx,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn physical_rows_keep_overlay_only_content_out_of_the_turned_paragraph() {
        for physical in [false, true] {
            for text in ["", "text"] {
                let mut opts = RenderOpts {
                    text: gpui::TextStyle::default(),
                    viewport: (800.0, 600.0),
                    normal_line_height: 1.2,
                    cx: Default::default(),
                    frag: Default::default(),
                };
                opts.cx.inline.pure_atom_paragraph = physical;
                let inherited = Computed {
                    no_select: Some(true),
                    ..Default::default()
                };
                let pieces = vec![
                    inline::Piece::Text {
                        text: text.into(),
                        style: BoxStyle::from(inherited.clone()),
                    },
                    inline::Piece::Overlay(div().into_any_element(), Default::default()),
                ];
                let mut element = route_pieces(
                    pieces,
                    &BoxStyle::from(inherited.clone()),
                    &opts,
                    None,
                    Default::default(),
                    None,
                    None,
                    vec![],
                );
                assert_eq!(
                    element.downcast_mut::<gpui::StyledText>().is_some(),
                    !physical || !text.is_empty(),
                    "physical={physical}, text={text:?}",
                );
            }
        }
    }
}
