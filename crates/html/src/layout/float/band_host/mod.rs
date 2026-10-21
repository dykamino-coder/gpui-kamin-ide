//! Гейт соседей флоатов по литералам стиля и общие поля полос обтекания. Полосы занятости
//! меряют только числа, поэтому сосед флоатов годится лишь с margin-box, заданным точками
//! (`px_margin_box`), а его роль — `BandPiece` (`band_piece`): атом строки, коробка своего
//! контекста (§9.5, последний абзац) или невидимая распорка. Поля и кегль для полос
//! разрешают `band_margins`, `band_edge`, `band_em` (подмодуль `edges`).

use crate::dom::Node;
use crate::layout::box_kind::{inline_level, is_blank, own_context};
use crate::style::computed::{Computed, Display};
use crate::style::values::value::{Len, px_missing_zero};
mod edges;
pub(super) use edges::band_edge;
pub(super) use edges::band_em;
pub(super) use edges::band_margins;

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
    /// Пустая распорка без ширины: обычный блок в потоке. Флоаты она
    /// перекрывает (обтекают только СТРОКИ), а показать ей нечего — от неё
    /// нужна одна высота. Держит эталоны `floats-wrap-top-below-002*-ref`.
    ///
    /// Проверено: вариант `BfcAuto` — коробка со своим контекстом БЕЗ
    /// заданной ширины, которой ширину назначает само размещение (§10.3.3
    /// плюс §9.5), — ничего не меняет: `floats-wrap-bfc-with-margin-006/007`
    /// держит не ширина.
    Strut,
    // Не добавлять вариант `Flow` — ОБЫЧНЫЙ блочный бокс в потоке, хотя
    // перечисление §9.5 закрытое (таблица, блочный замещаемый, коробка
    // своего контекста) и обычный блок флоат обязан ПЕРЕКРЫВАТЬ, а не
    // отодвигаться; сегодня такой хвост уводит пробег на флекс-ряд.
    // Кусок потока в полосах встаёт по правильной оси, но правила 3 и 7
    // §9.5.1 считают ПОЛОЖЕНИЕ ФЛОАТА относительно предыдущих коробок, а
    // полосы этого не знают (`floats-rule3-outside-right-002`,
    // `floats-rule7-outside-left-001`,
    // `adjoining-float-nested-forced-clearance-003`). Вернуть можно только с
    // правилами 3 и 7 в `bands` и с клиренсом как величиной в потоке.
}

// Точки поля: `auto` считается нулём.
//
// Отдельно от `px_missing_zero`, потому что `margin: auto` у коробки С ЗАДАННОЙ
// шириной поле не растит, а лишь выбирает, к какому краю прижаться
// (§10.3.3); занятость от него не меняется. Без этого гейт бандового хоста
/// не сработал бы вовсе: `floats-wrap-top-below-bfc-001l` — `margin-right:
/// auto`, `-001r` — `margin-left: auto`.
pub(super) fn px_margin(l: &Option<Len>) -> Option<f32> {
    match l {
        Some(Len::Auto) => Some(0.0),
        other => px_missing_zero(*other),
    }
}

/// margin-box коробки в точках, когда ВСЕ стороны заданы точками.
///
/// Полосы занятости меряют только числа (`bands`), и брать их можно лишь
/// у коробки, чей размер известен из стиля целиком.
pub(super) fn px_margin_box(c: &Computed) -> Option<(f32, f32)> {
    let b = c.borders();
    Some((
        px_missing_zero(c.width)?
            + px_missing_zero(c.padding.left)?
            + px_missing_zero(c.padding.right)?
            + px_missing_zero(b.left)?
            + px_missing_zero(b.right)?
            + px_margin(&c.margin.left)?
            + px_margin(&c.margin.right)?,
        px_missing_zero(c.height)?
            + px_missing_zero(c.padding.top)?
            + px_missing_zero(c.padding.bottom)?
            + px_missing_zero(b.top)?
            + px_missing_zero(b.bottom)?
            + px_margin(&c.margin.top)?
            + px_margin(&c.margin.bottom)?,
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
