//! Абзац: строчные куски узла в элемент Paragraph, выравнивание атомов по строке.

use crate::dom::{Element, Node};
use crate::layout::positioned::static_position::at_static_position;
use crate::layout::writing_mode::{native_vertical, rotated_atom};
use crate::paint::effects::paint_scope::DepthScope;
use crate::paint::effects::paint_scope::snapshot as defer_depth;
use crate::render::*;
use crate::style::cascade::inherit::inherit;
use crate::style::computed::{Computed, Display};
use crate::style::values::value::Len;
use crate::text::inline;
use crate::text::ruby::ruby_role;
use crate::text::text_box::normal_fraction;
use gpui::{AnyElement, IntoElement, ParentElement, SharedString, Styled, div, px};

pub(crate) mod atom_piece;
pub(super) mod pieces;

/// Абзац: одна строка текста с прогонами либо гибкая строка из кусков.
/// Абзац для тех, кто собирает текст сам — содержимое поля ввода.
pub fn paragraph_public(nodes: &[Node], inherited: &Computed, opts: &RenderOpts) -> AnyElement {
    paragraph(nodes, inherited, opts)
}

pub(crate) fn paragraph(nodes: &[Node], inherited: &Computed, opts: &RenderOpts) -> AnyElement {
    paragraph_routed(nodes, inherited, opts, None)
}

fn paragraph_routed(
    nodes: &[Node],
    inherited: &Computed,
    opts: &RenderOpts,
    native_request: Option<&native_paragraph_route::Request<'_>>,
) -> AnyElement {
    // Vertical paragraphs choose the physical text route after inline collection.
    if inherited.vertical == Some(true) {
        // `text-orientation: upright`: глифы СТОЯТ и идут сверху вниз —
        // никакого поворота. Это обычный горизонтальный абзац шириной в один
        // кегль (продвижение стоячего глифа = кегль, §7.4) с резкой по
        // знакам: каждый знак — своя строка, стопка растёт вниз.
        // У `sideways-*` ориентация текста ИГНОРИРУЕТСЯ (css-writing-modes-4
        // §text-orientation): глифы всегда лежат, стопка не строится.
        // `text-orientation` наследуется и действует на ТЕКСТ (§4.1): когда
        // все куски абзаца несут `upright` сами (`html::after { upright }`),
        // стопка обязана строиться так же, как при флаге на контейнере.
        let kids_upright = !nodes.is_empty()
            && nodes.iter().all(|n| match n {
                Node::Element(e) => e.style.upright == Some(true),
                Node::Text(t) => t.trim().is_empty(),
            });
        let upright = inherited.upright == Some(true) || kids_upright;
        if upright && inherited.sideways != Some(true) {
            let mut stack = inherited.clone();
            stack.vertical = None;
            stack.upright_stack = true;
            stack.break_word = Some(true);
            let em = match stack.font_size {
                Some(Len::Px(v)) => v,
                _ => opts.base_size(),
            };
            // Каждый стоячий глиф продвигает строку РОВНО на кегль (§7.4):
            // шаг стопки — кегль, а не своя высота строки; полоса переноса
            // уже одного глифа — в строку ложится ровно один знак (два узких
            // нуля вставали рядом, и стопка выходила короче).
            // Толщина вертикальной строки — LINE-HEIGHT, как у горизонтальной
            // (стопка глифов стоит в полосе высоты строки, повернутой набок):
            // читается ДО подмены шага стопки кеглем, иначе полоса всегда
            // равнялась кеглю (`vertical-alignment-vrl-022`).
            let lane = match stack.line_height {
                Some(Len::Px(v)) => v,
                Some(Len::Em(k)) | Some(Len::Pct(k)) => k * em,
                // До этой точки `ch` мог не разрешиться: стоячий ноль
                // продвигается на кегль (§7.4) — считаем сами.
                Some(Len::Ch(k)) => k * em,
                _ => em,
            };
            stack.line_height = Some(Len::Px(em));
            let inner = paragraph(nodes, &stack, opts);
            return div()
                .w(px(lane.max(em)))
                .flex_shrink_0()
                .flex()
                .justify_center()
                .child(div().w(px(em * 0.9)).flex_shrink_0().child(inner))
                .into_any_element();
        }
        let mut horizontal = inherited.clone();
        horizontal.vertical = None;
        // Пометка для `text-combine-upright`: кускам внутри повёрнутого
        // абзаца нужен контр-поворот (см. atom-ветку ниже).
        horizontal.rotated_line = Some(true);
        // `vertical-lr`: колонки идут слева направо — строки подаются снизу
        // вверх, чтобы после поворота ПО ЧАСОВОЙ первая оказалась левой (у
        // vertical-rl порядок родной: первая строка правой колонкой).
        //
        // `sideways-lr` вертится ПРОТИВ часовой (css-writing-modes-4,
        // Abstract-Physical Mapping: line-left = низ, over = лево), и после
        // такого поворота первая горизонтальная строка САМА оказывается левой
        // колонкой. Подавать строки снизу вверх тут — второй разворот,
        // ровно он и давал 180° (`block-flow-direction-slr-043` 32.20).
        let ccw_line = inherited.sideways == Some(true) && inherited.vertical_rl != Some(true);
        if inherited.vertical_rl != Some(true) && !ccw_line {
            horizontal.lines_reversed = Some(true);
        }
        // Поворот — приём отрисовки ТЕКСТА. Замещаемое содержимое (картинка,
        // элемент формы) вертикальное письмо не поворачивает никогда: абзац
        // из одной картинки обязан выглядеть так же, как в горизонтальном
        // письме (`wm-propagation-body-*`: подпись теста — рисунок, и он
        // ложился боком).
        let mut plain = String::new();
        gather_text(nodes, &mut plain);
        // Неразрывный пробел — ТЕКСТ: он даёт строку и её толщину
        // (`<td>&nbsp;</td>` в вертикальном ряду, ch-units-vrl-005), а
        // `trim()` съедал его как юникод-пробел, и абзац уходил
        // горизонтальным путём шириной в один пробел.
        if plain.trim().is_empty() && !plain.contains('\u{a0}') {
            // Pure-atom paragraphs paint physical boxes without a text transform.
            let para = rotated_atom::without_text_turn(|| paragraph(nodes, &horizontal, opts));
            // Строка из одних атомов (картинка, пустая строчная коробка) идёт
            // горизонтальным путём и ложится у ВЕРХНЕГО края коробки абзаца.
            // Верно это, только пока inline-start — верх. Таблица
            // Abstract-Physical Mapping (css-writing-modes-4,
            // Overview.bs:1877-1888) даёт `sideways-lr` line-left = НИЗ, а
            // `direction: rtl` ставит inline-start на line-right
            // (Overview.bs:1673-1675) — у `vertical-*`/`sideways-rl` это тоже
            // НИЗ. Повёрнутый текст это уже знает (`VerticalText::ccw`), а
            // безтекстовая ветка — нет: подпись-картинка стояла у верха
            // (`wm-propagation-body-035/039/043/051`,
            // `block-flow-direction-slr-066`; при rtl —
            // `overconstrained-rel-pos-rtl-*`). Коробка абзаца растянута по
            // строчной оси рядом блочного потока, и колонка с прижимом к
            // концу ставит ряд к её низу; строки-опоры у ряда из одних атомов
            // нет (`inline::as_wrapped_row`), поэтому низ картинки ложится
            // ровно на низ коробки.
            let from_bottom = ccw_line != (inherited.rtl == Some(true));
            if from_bottom && nodes.iter().any(|n| matches!(n, Node::Element(_))) {
                return div()
                    .flex()
                    .flex_col()
                    .justify_end()
                    .child(para)
                    .into_any_element();
            }
            return para;
        }
        let mut flow = horizontal.clone();
        flow.para_vertical = Some(inherited.vertical_rl == Some(true));
        let fallback = inherited.ortho_limit.unwrap_or(opts.viewport.1);
        flow.ortho_limit = Some(fallback);
        flow.orthogonal_inline = native_vertical::constraint(inherited, fallback);
        let built = std::cell::Cell::new(false);
        let request = native_paragraph_route::Request {
            style: &flow,
            built: &built,
        };
        // Build atoms with legacy style; apply native flow only to a text Paragraph.
        let inner = paragraph_routed(nodes, &horizontal, opts, Some(&request));
        if built.get() {
            return inner;
        }
        // Спросить размер у родителя обход не может (замер внутри чужого
        // замера падает — см. `VerticalText::request_layout`). Зато предел
        // ортогонального потока уже принесён вниз стилем: задаём его ШИРИНОЙ
        // до поворота, и после поворота он становится высотой коробки — то
        // есть перенос считается по той оси, по которой идёт строка.
        let limit = inherited.ortho_limit.unwrap_or(opts.viewport.1);
        // …и всё же ОДИН случай жёсткую ширину не терпит: АБСОЛЮТНАЯ коробка
        // со свободной строчной осью. Её размер по этой оси — по содержимому
        // (§10.3.7), а жёсткая ширина делает `natural.width` тождественно
        // равной пределу, и высота выходит во весь предел ортогонального
        // потока — коробка растягивалась на весь содержащий блок и вылезала
        // за него. Гейт узкий: потоковых коробок, на которых мерились четыре
        // отката выше, он не касается.
        let edge = |l: Option<Len>| !matches!(l, None | Some(Len::Auto));
        // Коробка на СТАТИЧЕСКОЙ позиции тоже абсолютна: `position` с неё
        // снято ради отсчёта, и без пометки `abs_static` гейт её не узнавал.
        let free_inline = (matches!(
            inherited.position,
            Some(crate::style::computed::Position::Absolute)
                | Some(crate::style::computed::Position::Fixed)
        ) || inherited.abs_static
            || inherited.hug_inline)
            && !matches!(inherited.height, Some(Len::Px(_)) | Some(Len::Pct(_)))
            && !(edge(inherited.inset.top) && edge(inherited.inset.bottom));
        // Ячейка вертикальной таблицы: предел строки — мера её КОЛОНКИ, а не
        // инлайн-размер всего стола. Колонку решает решётка (дорожка
        // `MinMax(MinContent, Auto)` = «наибольший min-content ячеек
        // колонки», css-tables-3 §computing-column-measures), но для этого ей
        // нужен вклад ячейки по МИНИМАЛЬНОМУ содержимому — а жёсткая ширина
        // до поворота делает `natural.width` тождественно равной пределу, и
        // вклад выходит равен всему столу: у `row-progression-vrl-002` все
        // три дорожки становились 140 и каждая ячейка рвала строку по
        // 7 знаков вместо 3/2/2. Потолок при этом остаётся: колонка не шире
        // инлайн-размера стола.
        let col_min = inherited.ortho_col && inherited.ortho_limit.is_some();
        // Atomic content shares native text's fixed and shrink-to-fit inline sizing.
        let inline_constraint = native_vertical::constraint(inherited, limit);
        let inner = if let Some(constraint) = inline_constraint {
            // A definite CSS inline size is also the percentage basis for anonymous
            // rows (including <br>); available space alone does not establish it.
            if constraint.fixed.is_some() {
                div()
                    .w(px(constraint.used(0.0, 0.0)))
                    .child(inner)
                    .into_any_element()
            } else {
                inner
            }
        } else if free_inline || col_min {
            div().max_w(px(limit)).child(inner).into_any_element()
        } else {
            div().w(px(limit)).child(inner).into_any_element()
        };
        // Intrinsic inline claims are independent of the first-line baseline metrics.
        let em = match inherited.font_size {
            Some(Len::Px(v)) => v,
            Some(Len::Em(k)) => k * opts.base_size(),
            _ => opts.base_size(),
        };
        let lh = match inherited.line_height {
            Some(Len::Px(v)) => Some(px(v)),
            Some(Len::Pct(k)) | Some(Len::Em(k)) => Some(px(k * em)),
            _ => None,
        };
        let central = inherited.sideways != Some(true) && inherited.text_sideways != Some(true);
        let vt = crate::text::vertical::VerticalText::new(inner)
            .counter_clockwise(ccw_line)
            .lines_left_first(inherited.vertical_rl != Some(true) && !ccw_line)
            .first_line(measure_font(inherited, opts), px(em), lh, central)
            // Замер по МИНИМАЛЬНОМУ содержимому: заявленная высота повёрнутой
            // коробки становится вкладом ячейки в дорожку её колонки
            // (см. `col_min` выше). Ниже `fit_within` заявит эту же величину
            // высотой — предел (мера стола) её не режет, потому что
            // min-content колонки заведомо не больше него.
            .column_min(col_min)
            .inline_keyword(native_vertical::keyword(inherited))
            .keyed(crate::text::vertical::vt_seq_key(
                text_id(&plain) ^ opts.doc_salt ^ (nodes.len() as u64).wrapping_mul(0x9E3779B9),
            ));
        // Настоящий предел от родителя (ортогональная ячейка): строка,
        // которая уже влезает, заявляет высоту честно — без неё гибкая
        // ячейка мерила коробку нулём и justify уводил глиф из виду.
        let vt = if let Some(constraint) = inline_constraint {
            vt.inline_constraint(constraint)
        } else if let Some(l) = inherited.ortho_limit {
            vt.fit_within(px(l))
        } else if inherited.hug_claim {
            // Родитель размером в содержимое по строчной оси
            // (`vertical_hug_children`): длину строки решать некому, и
            // коробка заявляет её сама — max-content под пределом `limit`
            // (обёртка выше уже `max_w`, замер по содержимому).
            vt.fit_within(px(limit))
        } else {
            vt
        };
        let vt = if let Some(Len::Px(cap)) = inherited.max_height {
            vt.claiming_height(px(cap))
        } else {
            vt
        };
        return vt.into_any_element();
    }
    // Первая строка со своим стилем: где она кончается, известно только после
    // переноса, поэтому абзац собирается замером (см. `float::FirstLine`).
    if let Some(first) = inherited.first_line.clone() {
        let mut base = inherited.clone();
        base.first_line = None;
        let nodes_owned = nodes.to_vec();
        let opts_owned = opts.clone();
        let mut plain = String::new();
        first_line_text::gather(nodes, inherited.preserve_newlines == Some(true), &mut plain);
        let plain = crate::text::inline::transform_case(&normalize_for_shadow(&plain), inherited);
        if !plain.trim().is_empty() {
            let size = match base.font_size {
                Some(Len::Px(v)) => v,
                Some(Len::Em(k)) => k * opts.base_size(),
                _ => opts.base_size(),
            };
            let line = match base.line_height {
                Some(Len::Px(v)) => v,
                Some(Len::Pct(k)) => size * k,
                _ => size * normal_fraction(&base, opts),
            };
            let for_build = first.clone();
            let depth = defer_depth();
            let build: crate::layout::float::split_flow::Split =
                std::rc::Rc::new(move |at, width| {
                    let _depth = DepthScope::enter(depth);
                    let mut styled = base.clone();
                    styled.first_line = None;
                    let mut para =
                        paragraph_pieces(&nodes_owned, &styled, &opts_owned, at, &for_build);
                    para = div().w(width).child(para).into_any_element();
                    para
                });
            // Мерить надо ТЕМ начертанием, каким строка и будет набрана:
            // жирная первая строка занимает больше места, и разрез по
            // обычному шрифту не помещался бы в неё целиком.
            let mut font = opts.text.font();
            font.fallbacks = crate::style::computed::font_family::fallbacks(&first, font.fallbacks);
            if let Some(w) = first.font_weight {
                font.weight = gpui::FontWeight(w as f32);
            }
            if first.italic == Some(true) {
                font.style = gpui::FontStyle::Italic;
            }
            if let Some(family) = first.font_family.as_ref().filter(|f| !f.is_empty()) {
                font.family = family.clone().into();
            }
            let measure_size = match first.font_size {
                Some(Len::Px(v)) => v,
                Some(Len::Em(k)) => k * size,
                _ => size,
            };
            return crate::layout::float::split_flow::FirstLine::new(
                build,
                SharedString::from(plain.trim().to_string()),
                font,
                measure_size,
                line,
            )
            .into_any_element();
        }
    }
    paragraph_pieces_routed(
        nodes,
        inherited,
        opts,
        0,
        &Computed::default(),
        native_request,
    )
}

/// Свой кегль абзаца в точках — точка отсчёта для строки-опоры и для долей.
///
/// Отсчёт идёт от СВОЕГО кегля, а не от базового кегля документа: у коробки с
/// `font-size: 10px` строка обязана быть в 10 точек, а базовый (16) держал её
/// вдвое выше.
pub(super) fn own_size(inherited: &Computed, opts: &RenderOpts) -> f32 {
    match inherited.font_size {
        Some(Len::Px(v)) => v,
        Some(Len::Em(k)) | Some(Len::Pct(k)) => k * opts.base_size(),
        _ => opts.base_size(),
    }
}

/// Есть ли в абзаце текст ПОМИМО внепоточных кусков.
///
/// Это ровно условие, при котором абзацу доступен ТЕКСТОВЫЙ путь: текст для
/// него собирает `inline::text_and_runs` из кусков `Piece::Text`, а
/// внепоточный кусок в него не входит (`inline.rs:2539`), и на пустом тексте
/// путь закрыт (`inline.rs:2543`). Внепоточный — и абсолют на статической
/// позиции, и абсолют с краями: оба уходят `Piece::Overlay`, оба своего
/// текста в строку не отдают.
pub(super) fn has_flow_text(nodes: &[Node]) -> bool {
    nodes.iter().any(|n| match n {
        Node::Text(t) => !t.trim().is_empty(),
        Node::Element(e) => {
            !matches!(
                e.style.position,
                Some(crate::style::computed::Position::Absolute)
                    | Some(crate::style::computed::Position::Fixed)
            ) && has_flow_text(&e.children)
        }
    })
}

/// Куски текста с `vertical-align: top`/`bottom` (CSS 2.1 §10.8.1): отрезок
/// байт в тексте абзаца, край (`true` — верх) и высота строчной коробки
/// куска — его `line-height`.
///
/// `vertical-align` у нас наследуется (ради ячеек таблицы), поэтому краевым
/// считается только кусок, чьё значение ОТЛИЧАЕТСЯ от значения абзаца: иначе
/// каждый абзац ячейки с `vertical-align: top` прижимался бы весь.
pub(super) fn edge_pieces(
    pieces: &[inline::Piece],
    inherited: &Computed,
    opts: &RenderOpts,
) -> Vec<(std::ops::Range<usize>, bool, f32)> {
    use crate::style::computed::Align;
    let mut out: Vec<(std::ops::Range<usize>, bool, f32)> = Vec::new();
    // A rotated vertical paragraph is laid out in its pre-rotation frame,
    // whose top is the line-over side (css-writing-modes-4 §line-relative
    // directions), so `top`/`bottom` keep their meaning there.
    if inherited.vertical == Some(true) {
        return out;
    }
    let mut at = 0usize;
    for p in pieces {
        let inline::Piece::Text { text, style } = p else {
            continue;
        };
        let end = at + text.len();
        let top = match style.vertical_align {
            Some(Align::Start) => Some(true),
            Some(Align::End) => Some(false),
            _ => None,
        };
        let out_of_flow = style.float.is_some_and(|f| f != 0)
            || matches!(
                style.position,
                Some(crate::style::computed::Position::Absolute)
                    | Some(crate::style::computed::Position::Fixed)
            );
        if let Some(top) = top
            && style.vertical_align != inherited.vertical_align
            && !out_of_flow
            && !text.is_empty()
        {
            let size = match style.font_size {
                Some(Len::Px(v)) => v,
                Some(Len::Em(k)) => k * opts.base_size(),
                _ => own_size(inherited, opts),
            };
            let h = match style.line_height {
                Some(Len::Px(v)) => v,
                Some(Len::Pct(k)) | Some(Len::Em(k)) => k * size,
                _ => size * normal_fraction(style, opts),
            };
            match out.last_mut() {
                // Соседние куски одного края — одна коробка (`<span>` с
                // вложенными кусками).
                Some((r, t, hh)) if r.end == at && *t == top => {
                    r.end = end;
                    *hh = hh.max(h);
                }
                _ => out.push((at..end, top, h)),
            }
        }
        at = end;
    }
    out
}

/// Лежит ли отрезок внутри краевого куска.
pub(super) fn in_edge(
    edges: &[(std::ops::Range<usize>, bool, f32)],
    r: &std::ops::Range<usize>,
) -> bool {
    edges
        .iter()
        .any(|(e, _, _)| e.start < r.end.max(r.start + 1) && r.start < e.end)
}

/// Самый крупный кегль и наибольшая `line-height` кусков ВНЕ краевых: струт
/// строки и её базовая линия от прижатых к краю не зависят (§10.8.1).
pub(super) fn flow_metrics(
    pieces: &[inline::Piece],
    edges: &[(std::ops::Range<usize>, bool, f32)],
    inherited: &Computed,
    opts: &RenderOpts,
) -> (f32, f32) {
    let strut = own_size(inherited, opts);
    let fraction = normal_fraction(inherited, opts);
    let em_base = opts.base_size();
    let (mut size_max, mut lh_max) = (strut, strut * fraction);
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
            _ => size * fraction,
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
pub(super) fn line_box_spans(
    pieces: &[inline::Piece],
    edges: &[(std::ops::Range<usize>, bool, f32)],
    inherited: &Computed,
    opts: &RenderOpts,
) -> Option<(Vec<(std::ops::Range<usize>, f32)>, f32)> {
    if inherited.vertical == Some(true)
        || inherited.rotated_line == Some(true)
        || inherited.text_fit.is_some()
    {
        return None;
    }
    let own = own_size(inherited, opts);
    let strut = match inherited.line_height {
        Some(Len::Px(v)) => v,
        Some(Len::Pct(k)) | Some(Len::Em(k)) => k * own,
        None | Some(Len::Auto) => own * normal_fraction(inherited, opts),
        _ => return None,
    };
    let mut out: Vec<(std::ops::Range<usize>, f32)> = Vec::new();
    let mut mixed = false;
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
            None | Some(Len::Auto) => size * normal_fraction(style, opts),
            _ => return None,
        };
        if (size - own).abs() > 0.01
            || (lh - strut).abs() > 0.01
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
pub(super) fn atoms_fit_line(inherited: &Computed, ruby: bool) -> bool {
    inherited.vertical != Some(true)
        // A rotated paragraph of atoms only is a row whose end edge sits on
        // the end of the paragraph box (`paragraph_routed`, pure-atom
        // branch); a line box would add the strut's descent below the atoms
        // (`wm-propagation-body-035`: the caption image rose by the descent).
        && !(inherited.rotated_line == Some(true) && !rotated_atom::text_turn())
        && (ruby || inherited.rtl != Some(true))
        && inherited.no_select != Some(true)
        && inherited.pointer_events_none != Some(true)
        && crate::text::paragraph::align_for(inherited) != crate::text::paragraph::Align::Justify
        // Обрыв строки многоточием (`text-overflow: ellipsis`) режет текст по
        // знакам, а распорку атома знаком не считает: атом обрывался не там
        // (`text-overflow-016`, `text-overflow-ruby`) — такой абзац в ряду.
        && inherited.ellipsis != Some(true)
}

/// `vertical-align` атома для строки абзаца, если атом туда годится.
///
/// Атом раскладывается ДО замера абзаца по своему содержимому
/// (`Paragraph::lay_atoms`), поэтому в строку идут только атомы, чей размер от
/// ширины строки не зависит: без долей в размерах, полях и отступах. Абсолюты
/// (их место — щуп статической позиции), поля форм и руби остаются в ряду.
pub(super) fn atom_line_align(
    e: &Element,
    inherited: &Computed,
    opts: &RenderOpts,
) -> Option<crate::text::paragraph::AtomAlign> {
    use crate::text::paragraph::AtomAlign;
    let st = &e.style;
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
    // (`interact::ClampCut`) делит высоту абзаца на РАВНЫЕ строки, а строка с
    // аннотацией выше прочих. ★ ЗАМЕРЕНО (03.10, 147 пар руби): в строке
    // `line-clamp-auto-with-ruby-001/003` зеленеют (5.2 → 0.13/0.26), но
    // `-002` (руби за срезом) уходит 0.09 → 5.23 — срез встаёт строкой выше.
    // Возвращать вместе с настоящими низами строк в `ClampEntry`.
    // Ортогональный поток внутри атома меряется от ДОСТУПНОГО места (§7.3
    // css-writing-modes-3), а замер «по содержимому» его не даёт: коробка с
    // `writing-mode: vertical-*` и строчной стороной `auto` внутри атома
    // выходила другой высоты (`inline-box-orthogonal-child-with-margins`).
    // Элемент сетки и гибкого ряда размер берёт от дорожки/ряда, и от
    // доступного места не зависит — такой атом остаётся в строке: иначе абзац
    // теста с `vertical-rl`-элементами сетки шёл прежним рядом, а эталон из
    // простых `inline-block` — строкой (`grid-container-baseline-
    // synthesized-001..004`: 0.00 -> 11.00).
    fn has_vertical(nodes: &[Node], in_box_layout: bool) -> bool {
        nodes.iter().any(|n| match n {
            Node::Element(k) => {
                let sized_by_parent = in_box_layout || k.style.height.is_some();
                // Ломает замер только ортогональный ФЛОАТ (его ширина «по
                // содержимому» берётся от доступного места); ортогональный
                // блок в потоке раскладывается одинаково, и исключать атом
                // ради него значило вести тест рядом, а эталон строкой
                // (`baseline-with-orthogonal-flow-001`).
                let floated = k.style.float.is_some_and(|f| f != 0);
                (k.style.vertical.is_some() && !sized_by_parent && floated)
                    || has_vertical(&k.children, box_layout(&k.style))
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
    // Сам атом с вертикальным письмом допустим, если он сетка или гибкий ряд:
    // размер ему задают дорожки и содержимое, а не доступное место
    // (`grid-container-baseline-synthesized-002/004`).
    if (st.vertical.is_some() && !box_layout(st)) || has_vertical(&e.children, box_layout(st)) {
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
    let merged = inherit(inherited, st);
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
                    let family = merged.font_family.clone().unwrap_or_default();
                    AtomAlign::Shift(crate::text::metrics::spacing_px(Some(l), &family, size))
                } else if let Some(k) = st.vertical_shift {
                    AtomAlign::Shift(-k * size)
                } else if let Some(k) = st.vertical_shift_pct {
                    // Процент — от `line-height` самого атома (§10.8.1).
                    let own = match merged.line_height {
                        Some(Len::Px(v)) => v,
                        Some(Len::Pct(f)) | Some(Len::Em(f)) => f * size,
                        _ => size * normal_fraction(&merged, opts),
                    };
                    AtomAlign::Shift(-k * own)
                } else {
                    AtomAlign::Shift(0.0)
                }
            }
        },
    })
}

/// Абзац с готовым разрезом первой строки: `at` — сколько байт в неё вошло.
fn paragraph_pieces(
    nodes: &[Node],
    inherited: &Computed,
    opts: &RenderOpts,
    first_line_at: usize,
    first_line: &Computed,
) -> AnyElement {
    paragraph_pieces_routed(nodes, inherited, opts, first_line_at, first_line, None)
}
