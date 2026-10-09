//! Обтекание флоатов: `wrap_floats` и хвост потока.
// owner: A

use crate::dom::{Element, Node};
use crate::layout::block::struts::{float_only_wrapper, margin_px, through_strut};
use crate::layout::float::band_flow_host::BAND_FL;
use crate::layout::float::band_host::{BandPiece, band_host, band_piece, px_margin_box_em};
use crate::layout::float::band_measured::{band_host_m, band_host_nested};
use crate::layout::float::clear::{bfc_no_fit, clears_side, leading_clear};
use crate::layout::float::initial_letter::{inline_float_host, px_margin_w, split_leading_float};
use crate::layout::float::{band_clearance, float_clear_scope, inline_floats};
use crate::layout::positioned::static_position::at_static_position;
use crate::layout::writing_mode::native_vertical;
use crate::render::{inline_level, inline_level_box, is_blank, out_of_flow, own_context, phantom_inline, replaced_inline};
use crate::style::computed::{Align, Computed, Display, FlexDir};
use crate::style::values::value::Len;
use crate::text::text_box::blank_text;

/// Пустой блок потока, который флоат обязан НАКРЫТЬ (§9.5).
///
/// §9.5 перечисляет ЗАКРЫТЫМ списком, чей border box флоат перекрывать не
/// смеет: таблица, блочный замещаемый элемент и коробка, образующая свой
/// контекст форматирования. Обычный блок потока в список не входит — его
/// коробка стоит там же, где стояла бы без флоата, а сужаются только её
/// СТРОКИ. Приложение E кладёт флоаты (шаг 5) поверх фонов блоков потока
/// (шаг 4), поэтому накрытая часть блока не видна.
///
/// Сегодня `wrap_floats` уводит такую пару на флекс-ряд, и блок встаёт СБОКУ
/// от флоата: в `clear-004` красный квадрат 100×100 выезжает на x = 100 и
/// виден целиком («красное видно» при эталоне «голый зелёный квадрат»).
///
/// Гейт узкий нарочно — берётся ровно тот случай, где итог считается
/// арифметикой, а не раскладкой: у соседа НЕТ строк (внутри только пустой
/// текст), его border box известен точками и ЦЕЛИКОМ ложится внутрь margin
/// box флоата. Тогда после правки на экране остаётся один флоат, и терять
/// нечего. Шире — конвейер F1-F9 (`bands.rs` плюс правила 3 и 7), там уже
/// откачены две лобовые правки (`render.rs:6301`, `:6434`).
///
/// Возвращает `((ширина, высота) margin box флоата, (ширина, высота) border
/// box соседа)`.
///
/// Проба (`target/scout-floatplace-2026-09.md` §5.1): деревья ПОСЛЕ правки
/// для `clear-004`, `block-formatting-contexts-016`, `floats-135` и
/// `floats-008` сведены с НАСТОЯЩИМИ эталонами корпуса и дали 0.00 все
/// четыре; обратный порядок (сосед поверх флоата) даёт «красное видно».
fn covered_flow_tail(
    floater: &Element,
    tail: &Element,
    em: f32,
) -> Option<((f32, f32), (f32, f32))> {
    let zero = |l: &Option<Len>| matches!(l, None | Some(Len::Px(0.0)));
    let no_margins = |c: &Computed| {
        zero(&c.margin.top)
            && zero(&c.margin.right)
            && zero(&c.margin.bottom)
            && zero(&c.margin.left)
    };
    // Флоат: размер числом и никаких своих полей — в поля ляжет подъём
    // соседа. Позиционированный флоат и флоат с формой обтекания идут
    // прежним путём: у первого своя ось (`block-step-size-none-does-not-
    // establish-*`: `position: relative; z-index: -1`), у второго вырезы
    // считает `shape-flow`.
    if !no_margins(&floater.style)
        || floater.style.position.is_some()
        || floater.style.shape_outside.is_some()
    {
        return None;
    }
    let (fw, fh) = px_margin_box_em(&floater.style, em)?;
    // Сосед: обычный блок потока — не свой контекст, не замещаемый, не
    // элемент списка, без `clear`, без позиционирования, без своих полей и
    // без стилей `:hover`/`::first-letter`/`::first-line`.
    if !matches!(tail.style.display, None | Some(Display::Block))
        || own_context(tail)
        || replaced_inline(&tail.tag)
        || tail.list_item.is_some()
        || tail.style.clear.is_some()
        || tail.style.position.is_some()
        || !no_margins(&tail.style)
        || tail.hover.is_some()
        || tail.first_letter.is_some()
        || tail.first_line.is_some()
    {
        return None;
    }
    // Строк у соседа быть не должно: их §9.5 СУЖАЕТ, а не накрывает.
    if !tail.children.iter().all(is_blank) {
        return None;
    }
    let (tw, th) = px_margin_box_em(&tail.style, em)?;
    // Накрыт ЦЕЛИКОМ — только тогда итог правки известен заранее.
    (tw <= fw && th <= fh).then_some(((fw, fh), (tw, th)))
}

pub(crate) fn wrap_floats(
    nodes: Vec<Node>,
    parent: &Computed,
    // Открыт ли ВЕРХНИЙ край содержащего блока для схлопывания с полем
    // первого ребёнка (§8.3.1). Только при открытом крае верхнее поле
    // очищающей коробки увозит вниз сам содержащий блок, а вместе с ним —
    // ПРИМЫКАЮЩИЙ флоат (Blink `block_layout_algorithm.cc:1796`).
    cb_top_open: bool,
    // Кегль содержащего блока в точках: по нему `covered_flow_tail` решает
    // `em` у флоата и соседа без своего `font-size`.
    em: f32,
    // Можно ли звать измеряемый бандовый хост (`band_host_m`): блочный
    // контейнер горизонтального письма слева направо.
    measured_ok: bool,
    // Содержащий блок — корень БФК (§10.6.7): его авто-высота обязана
    // охватить флоаты, в том числе внутри вложенных блоков.
    parent_bfc: bool,
    // The parent is a table cell (see `CELL_BFC`): a BFC root whose float
    // host keeps containing floats, while the band-host choices above stay
    // those of an ordinary block (margin-collapse-121..125, 157, 158).
    cell_bfc: bool,
) -> Vec<Node> {
    let cb_width = parent.width;
    // `clear: inherit` — сторона родителя (`clear-005`: `clear: left` на
    // контейнере и `inherit` на ребёнке). Разрешается здесь: своего
    // наследования у ненаследуемого свойства нет, а родительский стиль есть
    // только у вызывающего.
    let nodes = inline_floats::lift(float_clear_scope::used(nodes, parent), parent);
    // Флоат, записанный ВНУТРИ строчной коробки, принадлежит не ей, а
    // ближайшему блочному предку (§10.1, §9.5.1 п.1). Строчная обёртка, в
    // которой кроме флоата ничего нет, снимается ЗДЕСЬ — ДО проверки
    // `floated`: иначе `wrap_floats` про такой флоат не узнает вовсе и выйдет
    // первой же строкой, а флоат уедет в абзац вместе с обвязкой `<span>`.
    let nodes: Vec<Node> = nodes
        .into_iter()
        .map(|n| {
            let hoisted = match &n {
                Node::Element(e) => inline_float_host(e),
                Node::Text(_) => None,
            };
            hoisted.map_or(n, Node::Element)
        })
        .collect();
    // A float at the very START of an inline wrapper that also holds other
    // content (only collapsible white space before it): it is placed before
    // anything of the first line (CSS 2.1 §9.5.1 rule 1 — its top is the
    // top of the line it occurs on, and nothing of that line precedes it),
    // so it is laid out exactly like a float written just before the
    // wrapper. `below-float`: `<span> <div float 100%> x</span>` must push
    // `x` (and its text-indent) below the float; the float was lost.
    let nodes: Vec<Node> = nodes
        .into_iter()
        .flat_map(|n| match &n {
            Node::Element(e) => match split_leading_float(e) {
                Some((float, rest)) => vec![Node::Element(float), Node::Element(rest)],
                None => vec![n],
            },
            Node::Text(_) => vec![n],
        })
        .collect();
    // Примыкающие флоаты во ВЛОЖЕННОЙ обёртке (Blink
    // `HasClearancePastAdjoiningFloats`; §9.5 для нового контекста): первая в
    // потоке — обёртка из одних флоатов (`float_only_wrapper`, §10.6.3 её уже
    // обнулил), следом разделитель — `clear` в сторону её флоатов или коробка
    // своего контекста без места рядом. Верх блока открыт, значит флоаты
    // ПРИМЫКАЮТ: разделитель встаёт ровно под их низом, поле не участвует
    // («No matter how large the margin is, it should still be just below the
    // float» — `adjoining-float-before-clearance`). Обёртке возвращается
    // высота `auto`: наша раскладка держит в ней флоат лоном, то есть его
    // высотой; полю разделителя — ноль. Цепь `leading_chain` поле такого
    // разделителя наверх уже не поднимала.
    let mut nodes = nodes;
    if cb_top_open
        && let Some(a) = nodes.iter().position(|n| !is_blank(n))
        && let Node::Element(w) = &nodes[a]
        && let Some((left, right, min_w)) = float_only_wrapper(w)
        && let Some(b) =
            (a + 1..nodes.len()).find(|&k| !is_blank(&nodes[k]) && !phantom_inline(&nodes[k]))
        && let Node::Element(n) = &nodes[b]
        && (match n.style.clear {
            Some(0) => true,
            Some(c) if c < 0 => left,
            Some(_) => right,
            None => false,
        } || bfc_no_fit(n, min_w))
    {
        if let Node::Element(w) = &mut nodes[a] {
            w.style.height = None;
        }
        if let Node::Element(n) = &mut nodes[b] {
            n.style.margin.top = Some(Len::Px(0.0));
        }
    }
    let floated = nodes.iter().any(|n| match n {
        Node::Element(e) => e.style.float.is_some_and(|f| f != 0),
        Node::Text(_) => false,
    });
    if !floated {
        // Флоатов среди прямых детей нет, но они есть ВНУТРИ блока потока, и
        // за ним идут братья — им эти флоаты видны (одно пространство
        // исключений на БФК, шаг F7: `new-fc-separates-from-float`,
        // `floats-bfc-003`). Хост начинается с такого блока.
        if measured_ok {
            for i in 0..nodes.len() {
                if let Some((mut host, j)) = band_host_nested(&nodes, i, em, parent_bfc) {
                    let mut out: Vec<Node> = nodes[..i].to_vec();
                    band_clearance::mark_start(&mut host, cb_top_open, &out);
                    out.push(Node::Element(host));
                    out.extend(nodes[j..].iter().cloned());
                    return out;
                }
            }
        }
        return nodes;
    }
    let mut nodes = nodes;
    let mut out: Vec<Node> = vec![];
    let mut i = 0usize;
    while i < nodes.len() {
        let Node::Element(e) = &nodes[i] else {
            out.push(nodes[i].clone());
            i += 1;
            continue;
        };
        let Some(side) = e.style.float.filter(|f| *f != 0) else {
            out.push(nodes[i].clone());
            i += 1;
            continue;
        };
        // Бандовый хост: пробег флоатов ОБЕИХ сторон, не обрывающийся на
        // `clear`, и хвост, раскладываемый по полосам занятости вместо
        // флекс-ряда. Гейт узкий (см. `band_host`); не сошёлся — идём
        // сегодняшней веткой ниже, ни строки в ней не меняя.
        // Правило 6 (§9.5.1): верх флоата — верх строки, в которой он
        // объявлен. Прогон АТОМОВ известного размера перед флоатом уходит в
        // хост вместе с ним, иначе флоат встаёт ПОД прогоном (`floats-001`).
        // Прогон ТЕКСТА полосам не отдаём: наборщик строк про них не знает.
        let lead_at = out
            .iter()
            .rposition(|n| !is_blank(n) && band_piece(n) != Some(BandPiece::Atom))
            .map_or(0, |p| p + 1);
        let has_lead = out[lead_at..]
            .iter()
            .any(|n| band_piece(n) == Some(BandPiece::Atom));
        // Флоат посреди строки ТЕКСТА («Hello<float>Kitty»): набранное до
        // него строчное содержимое (от последнего блока или `<br>`) уходит в
        // измеряемый хост началом прогона, флоат — на его строку
        // (`floats-placement-vertical-001a`).
        let text_at = out
            .iter()
            .rposition(|n| match n {
                Node::Text(_) => false,
                Node::Element(c) => {
                    !inline_level(c)
                        || c.tag == "br"
                        || out_of_flow(&c.style)
                        || c.attr("bands").is_some()
                }
            })
            .map_or(0, |p| p + 1);
        // Атомы тоже: статический хост с атомами впереди (`band_host`) мог
        // не сойтись по размерам (`floats-placement-006`).
        let text_lead = out[text_at..].iter().any(|n| !is_blank(n));
        let hosted = has_lead
            .then(|| band_host(&nodes, i, cb_width, &out[lead_at..]))
            .flatten()
            .map(|(h, n, l)| (h, n, Some(lead_at), l))
            .or_else(|| {
                (measured_ok && text_lead)
                    .then(|| band_host_m(&nodes, i, em, &out[text_at..]))
                    .flatten()
                    .map(|(h, n, l)| (h, n, Some(text_at), l))
            })
            .or_else(|| {
                band_host(&nodes, i, cb_width, &[]).map(|(h, n, l)| (h, n, None, l))
            })
            // Статический гейт не сошёлся из-за НЕИЗВЕСТНЫХ стилю размеров
            // (ширина содержащего блока, shrink-to-fit флоата, коробка
            // своего контекста без размеров) — их меряет раскладка
            // (`band_flow.rs`, шаги F2/F3/F5).
            .or_else(|| {
                measured_ok
                    .then(|| band_host_m(&nodes, i, em, &[]))
                    .flatten()
                    .map(|(h, n, l)| (h, n, None, l))
            });
        if let Some((mut host, next, took_lead, lifted)) = hosted {
            // CSS 2.1 §10.6.3: ordinary blocks count in-flow boxes, not floats.
            // A complete unfragmented suffix needs no later float bands, and
            // no later box of the enclosing context may see them (`float_tail`;
            // also excludes float boxes whose own style lost `float` here).
            if !parent_bfc
                && !cell_bfc
                && next == nodes.len()
                && parent.float_tail
                && (!parent.in_multicol || host.attr("bands") == Some("1"))
            {
                host.attrs.push(("inflow-height".into(), "1".into()));
            }
            if let Some(at) = took_lead {
                out.truncate(at);
            }
            // Хост измеряемый и до него в блоке ничего нет — первая строка
            // блока внутри хоста: слой `::first-line` едет с ним
            // (`band_kids` отдаёт его первому строчному прогону).
            // Внепоточные соседи (абсолюты, флоаты) строк не образуют
            // (`below-float3`: абсолют перед флоатом).
            if host.attr("bands") == Some("m")
                && out.iter().all(|n| {
                    is_blank(n) || matches!(n, Node::Element(c) if out_of_flow(&c.style))
                })
            {
                host.first_line = BAND_FL.with(|f| f.borrow().clone());
            }
            band_clearance::mark_start(&mut host, cb_top_open, &out);
            out.push(Node::Element(host));
            out.extend(lifted);
            i = next;
            continue;
        }
        // Подряд идущие плавающие блоки стоят в ОДНОМ ряду, а не каждый в
        // своём: `float: left` у четырёх соседей выстраивает их бок о бок.
        // Прежде каждый начинал свой ряд, и они вставали столбиком.
        let mut floaters: Vec<Element> = vec![];
        // Сторона КАЖДОГО собранного флоата: пробег берёт обе, и левые с
        // правыми стоят в одном ряду. Прежде пробег обрывался на смене
        // стороны, `rest` выходил пустым, и одинокий флоат становился обычным
        // блоком — он съедал строку потока, а всё за ним падало на его высоту
        // (`floats-wrap-top-below-bfc-*` и родня).
        let mut sides: Vec<i8> = vec![];
        let mut j = i;
        while j < nodes.len() {
            if is_blank(&nodes[j]) {
                j += 1;
                continue;
            }
            let Node::Element(next) = &nodes[j] else {
                break;
            };
            let Some(next_side) = next.style.float.filter(|f| *f != 0) else {
                break;
            };
            // `clear` у соседа обрывает ряд: он обязан начать свой. Так
            // написаны эталоны WPT — колонка из `float: right` + `clear: both`.
            if j > i && clears_side(next.style.clear, side) {
                break;
            }
            let mut floater = next.clone();
            native_vertical::claim_float_inline_size(&mut floater.style, parent, &floater.children);
            floater.style.float = None;
            // ПРОБОВАЛИ И ОТКАТИЛИ: помечать плавающий кусок блочным
            // (`display: block` + `inline = false`), как велит CSS 2.1 §9.7.
            // Замер: css-text 1003 → 998, flexbox 318 → 319 — итог в минус.
            // Строчная природа картинки нужна ряду обтекания: как блок она
            // перестаёт участвовать в общей строке текста рядом с собой.
            // Плавающий блок не растягивается и не сжимается — он занимает
            // свою ширину, остальное достаётся соседям.
            floater.style.flex_shrink = Some(0.0);
            // Плавающий блок сжимается ДО СОДЕРЖИМОГО, но не шире доступного
            // места. Ключевым словом `fit-content` это писалось раньше, и
            // выходило дороже: слово заворачивает коробку в сетку, а дорожка
            // сетки не считает БОКОВЫЕ ПОЛЯ ребёнка — `margin: 1px` съедал два
            // пикселя ширины, текст переставал помещаться и рвался посреди
            // слова (`word-space-transform-010`, где эталон — 21 одинаковая
            // коробка). Поэтому коробке С ПОЛЯМИ ширина не задаётся вовсе, а
            // потолком служит родитель.
            //
            // Всем остальным остаётся `fit-content`: потолок в родителя не
            // равен ему по смыслу. В родителе НУЛЕВОЙ ширины он обнуляет
            // коробку, тогда как по CSS плавающая коробка не уже минимального
            // содержимого и просто вылезает наружу
            // (`white-space-intrinsic-size-001`).
            let side_margin = |l: &Option<Len>| !matches!(l, None | Some(Len::Px(0.0)));
            if floater.style.width.is_none() {
                if side_margin(&floater.style.margin.left)
                    || side_margin(&floater.style.margin.right)
                {
                    floater.style.max_width = floater.style.max_width.or(Some(Len::Pct(1.0)));
                } else {
                    floater.style.width = Some(Len::FitContent);
                }
            }
            floaters.push(floater);
            sides.push(next_side);
            j += 1;
        }
        // Соседи до ближайшего `clear` — они и обтекают. Внепоточный
        // (absolute/fixed) сосед НЕ обтекает: в колонке ряда он получил бы
        // её своим содержащим блоком, и `right: 96px` считался от узкой
        // колонки, а не от контейнера (эталоны css-shapes с рядом
        // absolute-коробок выходили пустыми).
        // Кто рвёт ряд: `clear` брата, `clear` в начале прозрачной обёртки
        // (`leading_clear`) и коробка своего контекста, которой рядом с
        // флоатами пробега нет места (`bfc_no_fit`). Все трое обязаны встать
        // ПОД флоатами, а не в колонку сбоку от них.
        let run_min_w = nodes[i..j]
            .iter()
            .filter_map(|n| match n {
                Node::Element(f) if f.style.float.is_some_and(|s| s != 0) => {
                    Some(px_margin_w(&f.style).unwrap_or(0.0))
                }
                _ => None,
            })
            .fold(f32::INFINITY, f32::min);
        let separates = |next: &Element| {
            clears_side(next.style.clear, side)
                || clears_side(leading_clear(next), side)
                || bfc_no_fit(next, run_min_w)
        };
        let mut rest: Vec<Node> = vec![];
        let mut out_of_flow: Vec<Node> = vec![];
        while j < nodes.len() {
            if let Node::Element(next) = &nodes[j]
                && (separates(next) || next.style.float.is_some_and(|f| f != 0))
            {
                break;
            }
            if let Node::Element(next) = &nodes[j]
                && matches!(
                    next.style.position,
                    Some(crate::style::computed::Position::Absolute)
                        | Some(crate::style::computed::Position::Fixed)
                )
                && !at_static_position(&next.style)
            {
                out_of_flow.push(nodes[j].clone());
                j += 1;
                continue;
            }
            rest.push(nodes[j].clone());
            j += 1;
        }
        // ПРОБОВАЛИ И ОТКАТИЛИ: забирать строчный прогон, стоящий ПЕРЕД
        // пробегом флоатов, обратно из `out` в хвост — §9.5.1 п.6 держит
        // верх флоата на верху текущей строчной коробки, а §9.5 сужает саму
        // эту строку. Гейт «только при иначе пустом хвосте». Проба по 319
        // парам семей `floats*`, `float-*`, `clear-float-*`: приобретено 1
        // (`floats-001` 3.84 -> 0.00), потеряно 4 — `float-nowrap-3` 0.14 ->
        // 0.50, `-7` 0.00 -> 0.40, `-9` 0.26 -> 0.67, `floats-114` 0.06 ->
        // 1.14, и `float-nowrap-hyphen-rewind-1` 0.42 -> 2.46. Прогон надо
        // не переносить целиком, а сужать по полосам — этого канала нет.
        //
        // §9.5.2: у очищающей коробки верхнее поле ЗАМЕНЯЕТСЯ зазором, а не
        // складывается с ним: её верх = max(своё место, низ флоатов). Ряд
        // обтекания сам даёт `max(флоаты, колонка)`, поэтому остаток поля
        // переносится распоркой в КОНЕЦ колонки, а у самой коробки гасится —
        // иначе она опускалась на своё поле ниже низа флоата.
        //
        // Оговорка: если брат ПЕРЕД флоатом схлопывается насквозь, его поле
        // ещё не выложено, и верх ряда у нас и так ниже настоящего — тогда
        // перенос только удваивает сдвиг (`clearance-006`).
        let laid_out = out
            .iter()
            .rev()
            .find(|n| !is_blank(n))
            .is_none_or(|n| match n {
                Node::Element(prev) => through_strut(prev).is_none(),
                Node::Text(_) => true,
            });
        // ★ ЗАМЕРЕНО И ОТКАЧЕНО (01.09): убрать подмену целиком. §9.5.2 хочет
        // `max(низ флоатов, своё место + поле)`, а распорка даёт
        // `низ флоатов + поле` и гасит поле — оно выпадает из схлопывания с
        // полем родителя и следующего брата. Но без распорки хуже: срез из 17
        // пар жилы зазора дал 0 зелёных и до, и после, а три пары просели —
        // `margin-collapse-122` 1.55 → 2.94, `-125` 1.54 → 3.82,
        // `-142` 2.47 → «красное видно». Значит распорка держит положение, и
        // чинить надо не её удаление, а канал «поле участвует в схлопывании,
        // не двигая коробку».
        // Распорка — ПРОТЕЗ §9.5.2, а не коробка разметки: она ничего не
        // красит, и накрывать её флоатом (`covered_flow_tail` ниже) нечего.
        // Зато наложение снимает с флоата плавающую природу, и очищающая
        // коробка, ради которой распорка и поставлена, теряет тот нижний
        // край флоата, от которого считает зазор. Флаг гасит наложение
        // ровно на этом случае (проба §2.2: `margin-collapse-clear-003`,
        // `-009` и `nested-clearance-new-formatting-context` — все три
        // потери держит распорка `0 × остаток поля`, прошедшая гейт
        // `covered_flow_tail`, потому что ширины у неё нет, а
        // `px_of2(None) = 0`).
        let mut clearance_strut = false;
        if let Some(Node::Element(next)) = nodes.get(j)
            && separates(next)
            && let Some(top) = margin_px(next.style.margin.top, &next.style).filter(|v| *v > 0.0)
        {
            // §9.5.2 считает клиренс от ГИПОТЕТИЧЕСКОЙ позиции — «where the
            // actual top border edge would have been if the element's 'clear'
            // property had been none». Если пробег флоатов стоит в САМОМ
            // НАЧАЛЕ блока с открытым верхним краем, такой позиции не
            // существует: верхнее поле очищающей коробки схлопнулось бы с
            // полем блока (§8.3.1, «top margin of an in-flow block element
            // collapses with its first in-flow block-level child's top margin
            // if the element has no top border, no top padding, and the child
            // has no clearance») и увезло бы флоат ВНИЗ ВМЕСТЕ С СОБОЙ — то
            // есть мимо флоата не прошло бы ни при каком поле.
            //
            // Blink зовёт это примыкающим флоатом и решает до раскладки:
            // `block_layout_algorithm.cc:156-168`
            // (`HasClearancePastAdjoiningFloats` — «floats that would
            // otherwise (if 'clear' were 'none') be pulled down by the BFC
            // block offset of the child… we know for sure that we get
            // clearance, even before layout»), `:1796-1800` (флоат становится
            // примыкающим ровно при неразрешённом `BfcBlockOffset()`) и
            // `:2355-2367` («the child's margins won't have any effect»;
            // позиция берётся из `ExclusionSpace::ClearanceOffset`, то есть =
            // низ пробега).
            //
            // Низ пробега у нас и так даёт ряд обтекания, поэтому весь ответ —
            // НЕ ставить распорку и погасить поле: коробка встанет ровно под
            // рядом, каким бы большим поле ни было
            // (`negative-clearance-after-adjoining-float`: поле 200 при
            // флоате 50 — коробка обязана стоять на 50, а не на 200).
            let adjoining =
                cb_top_open && out.iter().all(is_blank) && rest.iter().all(is_blank);
            if !adjoining && laid_out {
                rest.push(Node::Element(Element {
                    list_item: None,
                    node_id: 0,
                    anim: None,
                    tag: "div".into(),
                    style: Computed {
                        display: Some(Display::Block),
                        height: Some(Len::Px(top)),
                        ..Computed::default()
                    },
                    hover: None,
                    first_letter: None,
                    first_line: None,
                    children: vec![],
                    attrs: vec![],
                    inline: false,
                }));
                clearance_strut = true;
            }
            if adjoining || clearance_strut {
                if let Some(Node::Element(next)) = nodes.get_mut(j) {
                    next.style.margin.top = Some(Len::Px(0.0));
                }
            }
        }
        // Плавающий блок, рядом с которым НЕЧЕМУ обтекать, рядом не нуждается:
        // он остаётся обычным блоком потока. Ряд в этом случае только вредил —
        // ширину внутри него раскладка мерила по самому узкому слову.
        // ★ ЗАМЕРЕНО И ОТКАЧЕНО: распускать ряд, когда обтекать нечем, для
        // ЛЮБОГО числа плавающих (а не только одного) — css-text 1018 → 1019,
        // но flexbox 320 → **306**. Ряд соседних плавающих блоков нужен: без
        // него они встают столбиком.
        if rest.iter().all(is_blank) && floaters.len() == 1 {
            {
                let mut lone = floaters.remove(0);
                lone.style.flex_shrink = None;
                // ПРОБОВАЛИ И ОТКАТИЛИ: заодно делать строчный по природе тег
                // блочным при `float: right` (обещание комментария ниже, кода
                // не было). Замерено: приобретено 4, потеряно 5 —
                // `float-nowrap-*` и `border-color-006`. Возвращать вместе с
                // §9.7 целиком.
                //
                // Обтекать нечем — но сторону блок обязан держать: `float: right`
                // без соседей всё равно стоит У ПРАВОГО края. Ряда тут нет, и
                // сторону задаёт выравнивание себя в колонке родителя. Оно
                // действует только на ЭЛЕМЕНТ раскладки, поэтому строчный по
                // природе тег (картинка) здесь же делается блочным: иначе он
                // уходит в абзац, и выравнивание достаётся абзацу, а не ему.
                // Выравнивание себя действует только на ЭЛЕМЕНТ раскладки:
                // строчный по природе тег иначе уходит в абзац, и сторона
                // достаётся абзацу, а не картинке. Гейт узкий — только
                // замещаемый тег и только когда перед ним в блоке ничего нет:
                // широкий уже мерился в минус (запись выше).
                // Доля размера у замещаемого считается от содержащего блока,
                // и блокификация его подменяет: `<iframe height="50%">` теряет
                // отсчёт (`float-replaced-height-005`). Такие остаются как есть.
                let pct_size = matches!(lone.style.width, Some(Len::Pct(_)))
                    || matches!(lone.style.height, Some(Len::Pct(_)));
                if replaced_inline(&lone.tag) && !pct_size && out.iter().all(is_blank) {
                    lone.inline = false;
                    lone.style.display = Some(Display::Block);
                }
                // Перед флоатом стоят одни АТОМЫ (замещаемые и строчные
                // блоки): они и флоат обязаны остаться в ОДНОЙ строке, а
                // сторону флоат держит сам. Выражается гибким рядом с
                // раздачей по краям — блокификация тут не годится, она
                // унесла бы флоат на свою строку
                // (`borders/border-color-001-ref`: вторая картинка обязана
                // стоять у правого края той же строки).
                // Атомом здесь считается и замещаемый тег БЕЗ заданных
                // размеров: у картинки они приходят из файла, а `band_piece`
                // требует точек.
                let atom_like = |n: &Node| match n {
                    Node::Text(_) => false,
                    Node::Element(e) => {
                        band_piece(n) == Some(BandPiece::Atom)
                            || (replaced_inline(&e.tag) && e.style.float.is_none())
                    }
                };
                let lead_at = out
                    .iter()
                    .rposition(|n| !is_blank(n) && !atom_like(n))
                    .map_or(0, |p| p + 1);
                let lead_atoms = out[lead_at..].iter().any(atom_like);
                // ЛЕВЫЙ флоат, перед которым в этом же блоке уже вышел
                // строчный прогон (§9.5 п.1 и п.6): его верх — верх ТЕКУЩЕЙ
                // строки, а сама строка вокруг него сужается, то есть на
                // экране он стоит ЛЕВЕЕ прогона, хотя в разметке идёт после.
                // Мы же дописывали его блоком следом, и полосы менялись
                // местами (`box-generation-001`: жёлтая «Float» уезжала на 70
                // точек вправо от оранжевой «Inline box»).
                let inline_run_like = |n: &Node| match n {
                    Node::Text(_) => true,
                    Node::Element(e) => e.style.float.is_none() && inline_level_box(e),
                };
                let run_at = out
                    .iter()
                    .rposition(|n| !is_blank(n) && !inline_run_like(n))
                    .map_or(0, |p| p + 1);
                // Прогон из одних пробелов строки не образует (§16.6.1: они
                // схлопываются), и флоату сужать нечего — он остаётся
                // одиночным блоком со своей стороной ниже. Иначе ряд держал
                // `float: left` у верха колонки `sideways-lr`, где line-left —
                // низ (`shape-outside-*-026-ref`: пробелы вокруг флоата).
                if side < 0
                    && !lead_atoms
                    && out[run_at..].iter().any(|n| !is_blank(n))
                {
                    let row: Vec<Node> = out.split_off(run_at);
                    let mut children = vec![Node::Element(lone)];
                    children.extend(row);
                    out.push(Node::Element(Element {
                        list_item: None,
                        node_id: 0,
                        anim: None,
                        tag: "float-row".into(),
                        style: Computed {
                            display: Some(Display::Flex),
                            ..Computed::default()
                        },
                        hover: None,
                        first_letter: None,
                        first_line: None,
                        children,
                        attrs: vec![],
                        inline: false,
                    }));
                    out.extend(rest);
                    out.extend(out_of_flow);
                    i = j;
                    continue;
                }
                if lead_atoms && side > 0 {
                    let mut row: Vec<Node> = out.split_off(lead_at);
                    lone.style.margin.left = Some(Len::Auto);
                    row.push(Node::Element(lone));
                    out.push(Node::Element(Element {
                        list_item: None,
                        node_id: 0,
                        anim: None,
                        tag: "float-row".into(),
                        style: Computed {
                            display: Some(Display::Flex),
                            ..Computed::default()
                        },
                        hover: None,
                        first_letter: None,
                        first_line: None,
                        children: row,
                        attrs: vec![],
                        inline: false,
                    }));
                    out.extend(rest);
                    out.extend(out_of_flow);
                    i = j;
                    continue;
                }
                // `sideways-lr`: line-left — НИЗ (css-writing-modes-4 §6.3),
                // и `float: left` прижимается к нижнему краю колонки.
                let line_left_bottom = parent.vertical == Some(true)
                    && parent.vertical_rl != Some(true)
                    && parent.sideways == Some(true);
                lone.style.align_self = Some(if (side < 0) != line_left_bottom {
                    Align::Start
                } else {
                    Align::End
                });
                out.push(Node::Element(lone));
            }
            out.extend(rest);
            out.extend(out_of_flow);
            i = j;
            continue;
        }
        // ПРОБОВАЛИ И ОТКАТИЛИ: пробег РАВНОШИРОКИХ флоатов без заданной
        // высоты раскладывать колонкой флекс-рядов по `floor(cb / mw)` штук в
        // ряд (§9.5.1 п.3 и п.5) — высоты для этого знать не нужно. Замерено
        // по всему CSS2: 0 и 0. Пары `c414-flt-fit-002/003/004` держит не
        // раскладка рядов, а что-то ещё.
        //
        // Обтекание ФОРМОЙ (`shape-outside`): ряд-колонка его не выразит —
        // строки должны сужаться каждая по-своему. Плавающие блоки с
        // ИЗВЕСТНЫМИ размерами уходят синтетическим узлом shape-flow:
        // сборка положит их absolute и передаст вырезы абзацу.
        let px_of = |l: &Option<Len>| match l {
            None => Some(0.0),
            Some(Len::Px(v)) => Some(*v),
            _ => None,
        };
        let sized = |e: &Element| -> Option<(f32, f32)> {
            let b = e.style.borders();
            Some((
                px_of(&e.style.width)?
                    + px_of(&e.style.padding.left)?
                    + px_of(&e.style.padding.right)?
                    + px_of(&b.left)?
                    + px_of(&b.right)?
                    + px_of(&e.style.margin.left)?
                    + px_of(&e.style.margin.right)?,
                px_of(&e.style.height)?
                    + px_of(&e.style.padding.top)?
                    + px_of(&e.style.padding.bottom)?
                    + px_of(&b.top)?
                    + px_of(&b.bottom)?
                    + px_of(&e.style.margin.top)?
                    + px_of(&e.style.margin.bottom)?,
            ))
        };
        let img_float = |f: &Element| {
            f.style
                .shape_outside
                .as_deref()
                .is_some_and(|r| r.contains("url("))
                && (f.tag == "img"
                    || f.children
                        .iter()
                        .any(|n| matches!(n, Node::Element(c) if c.tag == "img")))
        };
        // ЗАМЕРЕНО И ОТКАЧЕНО: пускать сюда пробег из флоатов ОБЕИХ сторон
        // (снять этот конъюнкт и вернуть сторону детям перед пушем). Срез из
        // 259 пар семей *shape*: 0 и 0 — тройка `spec-examples/shape-outside-
        // 001…003` как была «красное видно», так и осталась, её держит не
        // односторонность пробега.
        let shaped_run = floaters.iter().any(|f| f.style.shape_outside.is_some())
            && floaters.iter().all(|f| sized(f).is_some() || img_float(f));
        if shaped_run && sides.iter().any(|s| *s != side) {
            // A run of floats on BOTH sides: `shape_flow` reads each float's
            // side from the float itself (css-shapes-1 §1 — every float's
            // shape narrows its own side of the line boxes).
            for (f, s) in floaters.iter_mut().zip(sides.iter()) {
                f.style.float = Some(*s);
            }
        }
        if shaped_run {
            // Whitespace between the run and a preceding block start
            // collapses away (CSS 2.1 §16.6.1); left here it became its own
            // line above the shaped floats (`shape-outside-001`: +16px).
            if out.iter().all(|n| matches!(n, Node::Text(t) if blank_text(t))) {
                out.clear();
            }
            let mut host = Element {
                list_item: None,
                node_id: 0,
                anim: None,
                tag: "shape-flow".into(),
                style: Computed {
                    // Ширина содержащего блока — для долей формы и поля.
                    width: cb_width,
                    // Вертикальное письмо: инлайн-размер содержащего блока —
                    // его ФИЗИЧЕСКАЯ высота (css-writing-modes-4 §6.1). Строкам
                    // ряда (`FlowRow::vertical_rl`) нужен её предел: сам
                    // ряд лежит в автовысотном хосте и иначе получает 0.
                    height: if parent.vertical == Some(true) {
                        parent.height
                    } else {
                        None
                    },
                    ..Computed::default()
                },
                hover: None,
                first_letter: None,
                first_line: None,
                children: Vec::new(),
                // Подготовка выше сняла float с самих блоков — сторона и
                // число уезжают атрибутами.
                attrs: vec![
                    (
                        "side".into(),
                        if side < 0 {
                            "left".into()
                        } else {
                            "right".into()
                        },
                    ),
                    ("count".into(), floaters.len().to_string()),
                ],
                inline: false,
            };
            host.children = floaters.into_iter().map(Node::Element).collect();
            host.children.extend(rest);
            out.push(Node::Element(host));
            out.extend(out_of_flow);
            i = j;
            continue;
        }
        // §9.5: обычный блок потока флоат ПЕРЕКРЫВАЕТ — обходят только его
        // строки, а Приложение E кладёт флоат (шаг 5) поверх фонов потока
        // (шаг 4). Флекс-ряд наложения не выражает вовсе и ставит блок СБОКУ.
        //
        // Берётся ровно тот случай, где итог считается арифметикой: ОДИН
        // ЛЕВЫЙ флоат с margin box в точках и хвост из ОДНОГО пустого блока
        // потока, border box которого целиком ложится внутрь этого margin
        // box (гейт — `covered_flow_tail`). Наложение выражается двумя
        // ОБЫЧНЫМИ блоками потока: сосед, а следом флоат с подъёмом на
        // высоту соседа. Порядок обязателен — флоат ВТОРЫМ, иначе он ляжет
        // ПОД соседа (проба §5.1: «красное видно»). Поле снизу возвращает
        // поток на НИЗ СОСЕДА: §10.6.3 — флоат высоты родителя не растит
        // (проба §5.2: 0.00 на случае, где флоат выше соседа).
        //
        // Прогон текста ПЕРЕД флоатом правку отменяет: там правило 6 §9.5.1
        // держит верх флоата на верху текущей строки, а этого канала здесь
        // нет.
        let covered = {
            let mut only: Option<&Element> = None;
            let mut single = true;
            for n in &rest {
                match n {
                    Node::Text(t) if blank_text(t) => {}
                    Node::Element(e) if only.is_none() => only = Some(e),
                    _ => single = false,
                }
            }
            let run_before = out.iter().rev().find(|n| !is_blank(n)).is_some_and(|n| match n {
                Node::Element(e) => inline_level_box(e),
                Node::Text(_) => true,
            });
            match (single && !run_before, only, floaters.first()) {
                (true, Some(tail), Some(f)) => covered_flow_tail(f, tail, em),
                _ => None,
            }
        };
        if side < 0
            && floaters.len() == 1
            && out_of_flow.is_empty()
            // Хвост из одной распорки клиренса наложением не выражается:
            // §9.5.2 держит зазор следующей коробки от НИЗА ФЛОАТА, а
            // конструкция ниже флоат из потока убирает.
            && !clearance_strut
            // Следом РАЗДЕЛИТЕЛЬ (`separates`): его верх §9.5.2 считает от
            // НИЗА ФЛОАТА, а наложение возвращает поток на низ соседа — без
            // поля распорки нет, и очищающий брат вставал на низ соседа.
            && !nodes
                .get(j)
                .is_some_and(|n| matches!(n, Node::Element(next) if separates(next)))
            && let Some(((_, fh), (_, th))) = covered
        {
            let mut lone = floaters.remove(0);
            // Ряда нет — сжатие плавающего куска, заданное подготовкой выше,
            // здесь не при чём.
            lone.style.flex_shrink = None;
            lone.style.margin.top = Some(Len::Px(-th));
            lone.style.margin.bottom = Some(Len::Px(th - fh));
            out.extend(rest);
            out.push(Node::Element(lone));
            i = j;
            continue;
        }
        // ★ ЗАМЕРЕНО И ОТКАЧЕНО (01.09): §9.5 говорит, что коробка блочного
        // уровня флоат НЕ обходит — обходят только её строки. Пробовал
        // выразить это наложением: когда в хвосте нет ни одного текстового
        // узла, блоки идут своим чередом, а флоат кладётся поверх них
        // абсолютом в относительной обёртке. Срез из 299 пар флоатов:
        // 198 → 189 без гейта и 198 → 193 с гейтом «хвост не образует
        // своего контекста». Приобретение одно (`floats-rule3-outside-
        // right-001` 1.84 → 0.00), потери — `floats-rule7-outside-left-001`
        // 0.00 → 1.79, `floats-wrap-bfc-001/003-*-table` 0.00 → 2-7,
        // `floats-wrap-bfc-with-margin-008/009` 0.00 → 1.05. Наложение
        // рушит вертикальное место: у нас флоат в ряду задаёт высоту, а
        // абсолют её больше не держит.
        // Хвост со СВОИМИ боковыми полями остаётся на прежней основе: остаток
        // ряда достаётся ему без учёта этих полей, и коробка выходит у́же
        // нужного (`floats-wrap-bfc-with-margin-004/005/008/009`).
        let хвост_с_полями = rest.iter().any(|n| match n {
            Node::Element(e) => [e.style.margin.left, e.style.margin.right]
                .iter()
                .any(|m| !matches!(m, None | Some(Len::Px(0.0)))),
            Node::Text(_) => false,
        });
        let mut column = Element {
            list_item: None,
            node_id: 0,
            anim: None,
            tag: "div".into(),
            style: Computed {
                flex_grow: Some(1.0),
                // ЗАМЕРЕНО: CSS2 5336 → 5338 (+10/−8), CSS3 2418 → 2417
                // (+1/−2), итого +1. Приобретения — обтекание текстом
                // (`floats-rule3-outside-right-001` 1.84 → 0.00,
                // `floats-wrap-bfc-002/003-*-overflow` 5-8 → 0.00, четвёрка
                // `float-nowrap-*`). Потери — БФК со СВОИМИ полями рядом с
                // флоатом (`floats-wrap-bfc-with-margin-004/005/008/009`,
                // `floats-132`, `floats-rule7-outside-left-001`): им остаток
                // ряда достаётся без учёта их полей. Чинится каналом «поле
                // БФК входит в остаток», которого в ряду нет.
                // Колонка обтекания берёт ОСТАТОК ряда, а не своё содержимое:
                // при основе «по содержимому» её max-content складывался с
                // шириной флоата, ряд переносился, и `float: right` уезжал
                // ПОД текст к левому краю вместо правого края той же строки
                // (проба: `float:right` 60 точек и три слова в двухстах).
                flex_basis: (!хвост_с_полями).then_some(Len::Px(0.0)),
                flex_shrink: Some(1.0),
                min_width: (!хвост_с_полями).then_some(Len::Px(0.0)),
                ..Computed::default()
            },
            hover: None,
            first_letter: None,
            first_line: None,
            children: rest,
            attrs: vec![],
            inline: false,
        };
        column.style.display = Some(Display::Block);
        let mut row_children: Vec<Node> = vec![];
        let paired: Vec<(i8, Element)> = sides.iter().copied().zip(floaters).collect();
        row_children.extend(
            paired
                .iter()
                .filter(|(s, _)| *s < 0)
                .map(|(_, f)| Node::Element(f.clone())),
        );
        row_children.push(Node::Element(column));
        // Прижатые вправо идут справа налево в порядке разметки.
        //
        // Правило 9 §9.5.1: правый флоат — «as far to the right as possible».
        // Ряд переносит, и правый, не влезший рядом с левым (правило 3),
        // уезжает на свою строку — там флекс ставит его в НАЧАЛО строки
        // (`c414-flt-fit-005/006`: x = 0 вместо 5em). `margin-left: auto`
        // первому правому ряда возвращает прижим: авто-поле получает только
        // остаток ПОСЛЕ гибких длин (taffy `flexbox.rs:311` раньше `:358`),
        // поэтому на строке с колонкой `flex-grow: 1` оно нулевое и колонку не
        // сжимает. Флоат без своей ширины (`fit-content` — обёртка сеткой) и
        // с авторским левым полем не трогаем.
        let mut first_right = true;
        for (_, f) in paired.iter().rev().filter(|(s, _)| *s > 0) {
            let mut f = f.clone();
            if first_right
                && matches!(f.style.margin.left, None | Some(Len::Px(0.0)))
                && !matches!(f.style.width, None | Some(Len::FitContent))
            {
                f.style.margin.left = Some(Len::Auto);
            }
            first_right = false;
            row_children.push(Node::Element(f));
        }
        out.push(Node::Element(Element {
            list_item: None,
            node_id: 0,
            anim: None,
            // Метка для сборщика дерева: у ряда обтекания текст ещё режется
            // по нижнему краю плавающего блока (см. `float_flow`).
            tag: "kamin-float".into(),
            style: Computed {
                display: Some(Display::Flex),
                flex_dir: Some(FlexDir::Row),
                align_items: Some(Align::Start),
                // Плавающие блоки, которым не хватило ширины, уходят НИЖЕ
                // (CSS 2.1 §9.5.1): ряд обязан переносить.
                flex_wrap: Some(true),
                ..Computed::default()
            },
            hover: None,
            first_letter: None,
            first_line: None,
            children: row_children,
            attrs: vec![],
            inline: false,
        }));
        out.extend(out_of_flow);
        i = j;
    }
    out
}
