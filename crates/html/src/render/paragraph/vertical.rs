//! Выбор физического маршрута вертикального абзаца.

mod upright;
pub(super) use upright::upright_paragraph;

use super::{paragraph, paragraph_routed};
use crate::dom::Node;
use crate::layout::writing_mode::{native_vertical, rotated_atom};
use crate::render::*;
use crate::style::computed::Computed;
use crate::style::values::value::Len;
use gpui::{AnyElement, IntoElement, ParentElement, Styled, div, px};

#[allow(clippy::too_many_arguments)]
pub(crate) fn vertical_paragraph(
    nodes: &[Node],
    inherited: &Computed,
    opts: &RenderOpts,
    _native_request: Option<&native_paragraph_route::Request<'_>>,
) -> AnyElement {
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
        return upright_paragraph(nodes, inherited, opts);
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
    } else if crate::render::paragraph::paragraph_style(inherited).hug_claim {
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
    vt.into_any_element()
}
