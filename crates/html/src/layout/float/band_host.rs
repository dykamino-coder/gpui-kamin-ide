//! Хозяин полос обтекания.
// owner: A

use crate::dom::{Element, Node};
use crate::layout::positioned::static_position::at_static_position;
use crate::paint::effects::grouped::px_of2;
use crate::render::{inline_level, is_blank, own_context};
use crate::style::computed::{Computed, Display};
use crate::style::values::value::Len;

/// Многоколоночный поток из сплошного текста.
///
/// Годится, когда всё содержимое — строчное: тогда режется сам текст. Если
/// внутри блоки, колонки набираются из них сеткой, как и раньше.
/// Начертание, которым НАБИРАЕТСЯ текст этого места.
///
/// Разрез на колонки и обтекание считают, сколько текста влезает в строку.
/// Меряли базовым шрифтом окна — и на любом документе со своей типографикой
/// (`body { font: 13px system-ui }`) разрез уезжал: мерилось одно, рисовалось
/// другое.
/// Роль соседа плавающих блоков в бандовом хосте.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum BandPiece {
    /// Инлайн-блок с известным margin-box — строчный поток атомов
    /// (`FlowRow`): такие коробки стоят В СТРОКУ и делят её.
    Atom,
    /// Коробка со СВОИМ контекстом форматирования: её border-box не
    /// перекрывает флоаты вовсе (§9.5, последний абзац) — она ищет окно и
    /// съезжает вниз.
    Bfc,
    /// ПРОБОВАЛИ И ОТКАТИЛИ: вариант `BfcAuto` — коробка со своим контекстом
    /// и БЕЗ заданной ширины, которой ширину назначает само размещение
    /// (§10.3.3 плюс §9.5). Замерено по семьям *float*, *clear*, *shape*:
    /// 0 и 0 — обе ожидавшиеся пары (`floats-wrap-bfc-with-margin-006/007`)
    /// держит не ширина.
    ///
    /// Пустая распорка без ширины: обычный блок в потоке. Флоаты она
    /// перекрывает (обтекают только СТРОКИ), а показать ей нечего — от неё
    /// нужна одна высота. Держит эталоны `floats-wrap-top-below-002*-ref`.
    Strut,
    // ★ ЗАМЕРЕНО И ОТКАЧЕНО (03.09): вариант `Flow` — ОБЫЧНЫЙ блочный бокс в
    // потоке. Перечисление §9.5 закрытое (таблица, блочный замещаемый,
    // коробка своего контекста), значит обычный блок флоат обязан
    // ПЕРЕКРЫВАТЬ, а не отодвигаться от него; сегодня такой хвост уводит
    // пробег на флекс-ряд, который наложение выразить не умеет.
    //
    // Написано и работало: кусок с известной высотой (пустой коробке
    // `height: auto` даёт ноль по §10.6.3 — отдельная `px_margin_box_float`),
    // без требования ширины (она равна ширине содержащего блока), встающий
    // на потолок потока в инлайн-начало БЕЗ `place_among`; порядок краски
    // Приложения E соблюдён — куски потока уходят в хост ПЕРЕД флоатами,
    // коробки своего контекста после них.
    //
    // Срез флоатов (1685 пар, 1539 зелёных): 1536, потом 1538 после снятия
    // полей и со СЛИТОГО стиля (без этого коробка сдвигалась на поле
    // дважды, `floats-015` 0.00 -> 0.73). Итог: приобретено 2
    // (`floats-rule3-outside-right-001` 1.84 -> 0.11,
    // `block-formatting-contexts-016` «красное видно» -> 0.00), потеряно 3
    // (`floats-rule3-outside-right-002` 0.33 -> 2.06,
    // `floats-rule7-outside-left-001` 0.00 -> 1.79,
    // `adjoining-float-nested-forced-clearance-003` 0.00 -> «красное видно»).
    //
    // Зеркальность приобретения и потери у `rule3-outside-right-001/002`
    // говорит, что кусок потока встаёт по правильной оси, но правила 3 и 7
    // §9.5.1 считают ПОЛОЖЕНИЕ ФЛОАТА относительно предыдущих коробок, а
    // полосы этого не знают. Возвращать вместе с правилами 3 и 7 в
    // `bands.rs` и с клиренсом как величиной в потоке (шаги B-D и F6 из
    // `target/scout-floats-cluster-2026-09.md`). Патч целиком —
    // `target/a2-bands.patch`.
}

// Точки поля: `auto` считается нулём.
//
// Отдельно от `px_of2`, потому что `margin: auto` у коробки С ЗАДАННОЙ
// шириной поле не растит, а лишь выбирает, к какому краю прижаться
// (§10.3.3); занятость от него не меняется. Без этого гейт бандового хоста
/// не сработал бы вовсе: `floats-wrap-top-below-bfc-001l` — `margin-right:
/// auto`, `-001r` — `margin-left: auto`.
pub(super) fn px_margin(l: &Option<Len>) -> Option<f32> {
    match l {
        Some(Len::Auto) => Some(0.0),
        other => px_of2(other),
    }
}

/// margin-box коробки в точках, когда ВСЕ стороны заданы точками.
///
/// Полосы занятости меряют только числа (`bands.rs`), и брать их можно лишь
/// у коробки, чей размер известен из стиля целиком.
pub(super) fn px_margin_box(c: &Computed) -> Option<(f32, f32)> {
    let b = c.borders();
    Some((
        px_of2(&c.width)?
            + px_of2(&c.padding.left)?
            + px_of2(&c.padding.right)?
            + px_of2(&b.left)?
            + px_of2(&b.right)?
            + px_margin(&c.margin.left)?
            + px_margin(&c.margin.right)?,
        px_of2(&c.height)?
            + px_of2(&c.padding.top)?
            + px_of2(&c.padding.bottom)?
            + px_of2(&b.top)?
            + px_of2(&b.bottom)?
            + px_margin(&c.margin.top)?
            + px_margin(&c.margin.bottom)?,
    ))
}

/// `px_margin_box`, но `em` решается по кеглю: своему, если он задан
/// точками, иначе по кеглю содержащего блока `em`. Нужен гейту наложения
/// (`covered_flow_tail`): `floats-135` пишет флоат и соседа в `5em`, и
/// арифметика наложения известна так же точно, как в точках.
pub(super) fn px_margin_box_em(c: &Computed, em: f32) -> Option<(f32, f32)> {
    let own = match c.font_size {
        None => em,
        Some(Len::Px(v)) => v,
        _ => return None,
    };
    let len = |l: &Option<Len>| match l {
        None => Some(0.0),
        Some(Len::Px(v)) => Some(*v),
        Some(Len::Em(k)) => Some(*k * own),
        _ => None,
    };
    let b = c.borders();
    Some((
        len(&c.width)?
            + len(&c.padding.left)?
            + len(&c.padding.right)?
            + len(&b.left)?
            + len(&b.right)?
            + len(&c.margin.left)?
            + len(&c.margin.right)?,
        len(&c.height)?
            + len(&c.padding.top)?
            + len(&c.padding.bottom)?
            + len(&b.top)?
            + len(&b.bottom)?
            + len(&c.margin.top)?
            + len(&c.margin.bottom)?,
    ))
}

/// Чем сосед флоатов может быть в бандовом хосте; `None` — не может ничем,
/// и весь хост отменяется.
///
/// Порядок проверок важен: `own_context` истинен и для `inline-block`
/// (`:3087`), а строчную коробку блочной веткой ставить нельзя — она встанет
/// на свою строку вместо общей.
pub(super) fn band_piece(n: &Node) -> Option<BandPiece> {
    let Node::Element(c) = n else {
        // Непустой текст рядом с флоатом бандовый хост не набирает: это
        // работа наборщика строк, а он про полосы ещё не знает.
        return None;
    };
    // Внепоточный сосед получил бы хост своим содержащим блоком; `clear`
    // требует зазора, которого шаг B1 не считает.
    if matches!(
        c.style.position,
        Some(crate::style::computed::Position::Absolute)
            | Some(crate::style::computed::Position::Fixed)
    ) || c.style.float.is_some_and(|f| f != 0)
        || c.style.clear.is_some()
    {
        return None;
    }
    let (mw, mh) = px_margin_box(&c.style)?;
    // ОБЕ стороны обязаны стоять в стиле точками. Одного `px_margin_box` мало:
    // `width: auto` там `None` и складывается как НОЛЬ, а по §10.3.5 это
    // shrink-to-fit, которого полосы не считают. Без этой проверки коробка с
    // одними полями прошла бы гейт с нулевой шириной.
    let sized = matches!(c.style.height, Some(Len::Px(_)))
        && matches!(c.style.width, Some(Len::Px(_)))
        && mw > 0.0
        && mh > 0.0;
    if matches!(
        c.style.display,
        Some(Display::InlineBlock) | Some(Display::InlineFlex)
    ) || (c.tag == "img" && inline_level(c))
    {
        // CSS 2 section 9.5: a replaced inline participates in the shortened
        // line and moves below floats when its entire box cannot fit.
        return sized.then_some(BandPiece::Atom);
    }
    if own_context(c) {
        return sized.then_some(BandPiece::Bfc);
    }
    // Распорка: своей ширины нет, внутри пусто, краски нет — видно её нечем,
    // и место она занимает только по высоте.
    let invisible = c.style.background.is_none()
        && c.children.iter().all(is_blank)
        && matches!(c.style.width, None | Some(Len::Auto))
        && matches!(c.style.height, Some(Len::Px(_)))
        && mw == 0.0;
    (invisible && mh > 0.0).then_some(BandPiece::Strut)
}

/// Синтетический узел бандового хоста для пробега флоатов, начинающегося на
/// `i`; вернуть хост и номер узла ЗА хвостом.
///
/// Отличий от сегодняшнего пробега (`wrap_floats:1985-2038`) три:
///
/// 1. пробег НЕ обрывается ни на смене стороны, ни на `clear` — обе стороны
///    и очистка уезжают в один хост;
/// 2. `style.float` и `style.clear` с детей НЕ снимаются: их читает
///    `shape_flow`, который гасит их сам перед сборкой (`:5091`);
/// 3. флоатам не задаются ни `flex_shrink`, ни `fit-content` — ряда, ради
///    которого это делалось, здесь нет, а размер и так обязан быть в точках.
///
/// Гейт (все условия разом, иначе `None`):
///
/// * ширина содержащего блока известна ТОЧКАМИ и больше нуля — без неё
///   полосам негде поставить дальнюю стенку (`shape_flow` ставит
///   недостижимую `NO_WALL`, и сужения не будет вовсе);
/// * у каждого флоата пробега margin-box в точках;
/// * хвост непустой и состоит ТОЛЬКО из пустого текста и кусков `BandPiece`.
///
/// Последнее условие и держит радиус поражения: любой абзац, любой блок без
/// размеров, любой текст рядом с флоатом уводит на сегодняшний флекс-ряд.
pub(super) fn band_host(
    nodes: &[Node],
    i: usize,
    cb_width: Option<Len>,
    lead: &[Node],
) -> Option<(Element, usize, Vec<Node>)> {
    let cb_w = match cb_width {
        Some(Len::Px(v)) if v > 0.0 => v,
        _ => return None,
    };
    let mut floaters: Vec<Element> = vec![];
    let mut j = i;
    while j < nodes.len() {
        if is_blank(&nodes[j]) {
            j += 1;
            continue;
        }
        let Node::Element(next) = &nodes[j] else {
            break;
        };
        if !next.style.float.is_some_and(|f| f != 0) {
            break;
        }
        // Размер флоата обязан быть в точках: полосы ничего не мерят, а
        // `width: auto` у флоата — это shrink-to-fit (§10.3.5), который
        // `px_margin_box` сложил бы как НОЛЬ и посадил флоат нулевой ширины.
        if !matches!(next.style.width, Some(Len::Px(_)))
            || !matches!(next.style.height, Some(Len::Px(_)))
        {
            return None;
        }
        px_margin_box(&next.style)?;
        floaters.push(next.clone());
        j += 1;
    }
    if floaters.is_empty() {
        return None;
    }
    let mut rest: Vec<Node> = vec![];
    let mut lifted: Vec<Node> = vec![];
    while j < nodes.len() {
        if let Node::Element(next) = &nodes[j]
            && (next.style.float.is_some_and(|f| f != 0) || next.style.clear.is_some())
        {
            break;
        }
        // Внепоточный сосед (absolute/fixed не на статической позиции) ни
        // строк, ни полос не занимает (CSS 2.1 §9.6): он уходит ЗА хост, как
        // и на пути `wrap_floats`. Прежде он отменял хост целиком, и пробег
        // правых флоатов падал во флекс-ряд, ставивший их бок о бок даже без
        // места (`shape-outside-circle-034-ref`: второй флоат 120 рядом с
        // первым в блоке 200 вместо места под ним, §9.5.1 п.3).
        if let Node::Element(next) = &nodes[j]
            && matches!(
                next.style.position,
                Some(crate::style::computed::Position::Absolute)
                    | Some(crate::style::computed::Position::Fixed)
            )
            && !at_static_position(&next.style)
        {
            lifted.push(nodes[j].clone());
            j += 1;
            continue;
        }
        rest.push(nodes[j].clone());
        j += 1;
    }
    if !lead.is_empty() {
        // Прогон отдаём полосам ЦЕЛИКОМ и только когда он вместе с флоатами
        // помещается в одну строку: разъехавшийся прогон — это перенос, а
        // его считает наборщик строк, не полосы.
        if !lead
            .iter()
            .all(|n| is_blank(n) || band_piece(n) == Some(BandPiece::Atom))
        {
            return None;
        }
        let mut row = 0.0f32;
        for n in lead {
            if let Node::Element(e) = n {
                row += px_margin_box(&e.style)?.0;
            }
        }
        for f in &floaters {
            row += px_margin_box(&f.style)?.0;
        }
        if row > cb_w + 0.01 {
            return None;
        }
        let mut all = lead.to_vec();
        all.extend(rest);
        rest = all;
    }
    // Пустой хвост хост НЕ отменяет, если флоатов НЕСКОЛЬКО: лесенку
    // (§9.5.1 п.5) и правило 3 флекс-ряд не выражает вовсе. Одинокий флоат с
    // пустым хвостом полосам не нужен — его кладёт ветка ниже, и хост ей
    // только мешал (замерено: с пустым хвостом при любом числе флоатов
    // приобретено 10, потеряно 9).
    // ЗАМЕРЕНО: отсекать здесь ещё и пробеги с `clear` — 5065 -> 5064.
    if (floaters.len() < 2 && !rest.iter().any(|n| !is_blank(n)))
        || !rest.iter().all(|n| is_blank(n) || band_piece(n).is_some())
    {
        return None;
    }
    let side = floaters[0].style.float.unwrap_or(-1);
    let mut host = Element {
        list_item: None,
        node_id: 0,
        anim: None,
        tag: "shape-flow".into(),
        style: Computed {
            // Ширина содержащего блока — дальняя стенка полос.
            width: cb_width,
            ..Computed::default()
        },
        hover: None,
        first_letter: None,
        first_line: None,
        children: Vec::new(),
        attrs: vec![
            // Сторона первого флоата — только запасной ответ для живого пути
            // `shape-outside`: у бандового хоста сторона лежит на КАЖДОМ
            // ребёнке, и `shape_flow` читает её оттуда.
            (
                "side".into(),
                if side < 0 {
                    "left".into()
                } else {
                    "right".into()
                },
            ),
            ("count".into(), floaters.len().to_string()),
            // Метка бандового хоста: включает блочную ветку `shape_flow`.
            ("bands".into(), "1".into()),
        ],
        inline: false,
    };
    host.children = floaters.into_iter().map(Node::Element).collect();
    host.children.extend(rest);
    Some((host, j, lifted))
}

/// Поле для измеряемого хоста: точки, доля ширины содержащего блока или
/// `em` по кеглю `em`; `auto` — ноль (у флоата так велит §10.3.5, у куска
/// хвоста — как `px_margin` статического хоста). Прочее (`calc`, `vw`…) —
/// `None`, хост отменяется.
pub(super) fn band_edge(l: &Option<Len>, em: f32) -> Option<crate::layout::float::band_flow::Edge> {
    use crate::layout::float::band_flow::Edge;
    match l {
        None | Some(Len::Auto) => Some(Edge::Px(0.0)),
        Some(Len::Px(v)) => Some(Edge::Px(*v)),
        Some(Len::Pct(k)) => Some(Edge::Pct(*k)),
        Some(Len::Em(k)) => Some(Edge::Px(*k * em)),
        _ => None,
    }
}

/// Кегль коробки для `em` её полей; относительный `font-size` берёт
/// кегль родителя, в том числе проценты (CSS 2.1 §15.7).
pub(super) fn band_em(c: &Computed, em: f32) -> Option<f32> {
    match c.font_size {
        None => Some(em),
        Some(Len::Px(v)) => Some(v),
        Some(Len::Em(k)) | Some(Len::Pct(k)) => Some(k * em),
        _ => None,
    }
}

/// Все четыре поля коробки разрешимы для измеряемого хоста.
pub(super) fn band_margins(
    c: &Computed,
    em: f32,
) -> Option<[crate::layout::float::band_flow::Edge; 4]> {
    let em = band_em(c, em)?;
    Some([
        band_edge(&c.margin.top, em)?,
        band_edge(&c.margin.right, em)?,
        band_edge(&c.margin.bottom, em)?,
        band_edge(&c.margin.left, em)?,
    ])
}
