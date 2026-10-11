//! Колонки таблицы: дорожки и ширины.
// owner: A

use crate::dom::{Element, Node};
use gpui::px;
mod widths;
pub(super) use widths::col_element_widths;
pub(super) use widths::push_col_bands;

/// Дорожки таблицы: все по содержимому, последняя забирает остаток строки.
///
/// Ширины колонок считают ИМЕННО дорожки: это внутренние размеры содержимого,
/// посчитанные раскладкой по настоящим правилам переноса. Прежде поверх них
/// работал свой замер (`MeasuredTable`), меривший ГОЛЫЙ текст ячейки — без
/// переносов, сохранённых пробелов и вложенных коробок; снят по замеру:
/// css-text +1, flexbox +1, css-grid +1, поломок нет.
/// `min-content` снизу не даёт колонке сжаться в ноль на узкой панели.
pub(super) fn track_list_collapsed(
    cols: u16,
    fixed: bool,
    first_row: &[Option<f32>],
    col_widths: &[(Option<f32>, Option<f32>)],
    collapsed: &[bool],
    pcts: &[Option<f32>],
    table_px: Option<f32>,
) -> Vec<gpui::GridTrack> {
    let mut tracks = track_list(cols, fixed, first_row, col_widths, pcts, table_px);
    for (i, t) in tracks.iter_mut().enumerate() {
        if collapsed.get(i).copied().unwrap_or(false) {
            *t = gpui::GridTrack::Pixels(px(0.0));
        }
    }
    tracks
}

fn track_list(
    cols: u16,
    fixed: bool,
    first_row: &[Option<f32>],
    col_widths: &[(Option<f32>, Option<f32>)],
    pcts: &[Option<f32>],
    table_px: Option<f32>,
) -> Vec<gpui::GridTrack> {
    // `table-layout: fixed` — ширины из первого ряда, безразмерные колонки
    // делят остаток поровну; содержимое не меряется.
    if fixed {
        // Доли колонок (§17.5.2.1): доля берётся от ширины таблицы, а базис
        // всех дорожек здесь нулевой — значит свободное место равно ей самой,
        // и долю точно выражает `Fraction`. Остаток делят безразмерные.
        let pct_sum: f32 = (0..cols as usize)
            .filter_map(|i| pcts.get(i).copied().flatten())
            .sum();
        let auto_n = (0..cols as usize)
            .filter(|i| {
                pcts.get(*i).copied().flatten().is_none()
                    && first_row.get(*i).copied().flatten().is_none()
            })
            .count();
        // Доли переводятся в точки только рядом с ПИКСЕЛЬНОЙ колонкой: без
        // неё свободное место равно ширине стола, и `Fraction` точен сам.
        let в_точках = table_px.is_some() && first_row.iter().any(|w| w.is_some());
        let share = if auto_n > 0 {
            ((1.0 - pct_sum).max(0.0)) / auto_n as f32
        } else {
            0.0
        };
        return (0..cols as usize)
            .map(|i| {
                match (
                    pcts.get(i).copied().flatten(),
                    first_row.get(i).copied().flatten(),
                ) {
                    // Доля колонки берётся от ширины ТАБЛИЦЫ, а `Fraction`
                    // делит только СВОБОДНОЕ место: пока все дорожки долевые,
                    // это одно и то же, но рядом с ПИКСЕЛЬНОЙ колонкой
                    // расходится — 13 % от трёхсот выходило 39 вместо 52
                    // (`fixed-table-layout-022/023` против зелёной `-021`,
                    // которая отличается ровно отсутствием `col{width}`).
                    (Some(p), _) if в_точках => {
                        gpui::GridTrack::Pixels(px(p * table_px.unwrap_or(0.0)))
                    }
                    (Some(p), _) => gpui::GridTrack::MinMax(Box::new((
                        gpui::GridTrack::Pixels(px(0.0)),
                        gpui::GridTrack::Fraction(p),
                    ))),
                    (None, Some(w)) => gpui::GridTrack::Pixels(px(w)),
                    // Когда доли уже переведены в точки, безразмерной
                    // колонке достаётся ОСТАТОК: равные доли делят его
                    // поровну, а прежний `share` считал его от всей ширины
                    // стола и отдавал 69 вместо 124.
                    (None, None) if в_точках => gpui::GridTrack::MinMax(Box::new((
                        gpui::GridTrack::Pixels(px(0.0)),
                        gpui::GridTrack::Fraction(1.0),
                    ))),
                    (None, None) if pct_sum > 0.0 => gpui::GridTrack::MinMax(Box::new((
                        gpui::GridTrack::Pixels(px(0.0)),
                        gpui::GridTrack::Fraction(share),
                    ))),
                    // Пол дорожки — ноль, а не содержимое: фиксированная
                    // раскладка содержимое НЕ меряет (CSS 2.1 §17.5.2.1), и
                    // колонка вправе быть у́же него. Голая доля брала минимумом
                    // вклад `min-content`, из-за чего сумма колонок перерастала
                    // заданную ширину таблицы (`fixed-table-layout-003a01`).
                    (None, None) => gpui::GridTrack::MinMax(Box::new((
                        gpui::GridTrack::Pixels(px(0.0)),
                        gpui::GridTrack::Fraction(1.0),
                    ))),
                }
            })
            .collect();
    }
    // Все колонки по содержимому. Остаток строки НЕ отдаётся последней:
    // раньше она забирала его целиком, и таблица из двух коротких ячеек
    // расползалась по краям окна (видно на `shaping-join-001`). Излишек
    // раздаёт раскладка между дорожками `auto` — это ближе к табличной
    // раздаче «пропорционально разнице полной и минимальной ширины».
    (0..cols as usize)
        .map(
            |i| match col_widths.get(i).copied().unwrap_or((None, None)) {
                // Заявленная ширина, но не уже содержимого: minmax с потолком
                // ниже пола отдаёт пол (правило сетки), то есть
                // max(min-content, ширина).
                (Some(w), _) => gpui::GridTrack::MinMax(Box::new((
                    gpui::GridTrack::MinContent,
                    gpui::GridTrack::Pixels(px(w)),
                ))),
                // Процентная колонка забирает долю ОСТАТКА: соседние колонки по
                // содержимому, свободное место делится по долям.
                (None, Some(k)) => gpui::GridTrack::Fraction(k),
                (None, None) => gpui::GridTrack::MinMax(Box::new((
                    gpui::GridTrack::MinContent,
                    gpui::GridTrack::Auto,
                ))),
            },
        )
        .collect()
}

/// Ширины колонок из элементов `<col>`/`<colgroup>` (атрибут `span`
/// повторяет запись): при фиксированной раскладке они СТАРШЕ ячеек первого
/// ряда (CSS 2.1 §17.5.2.1).
/// Элементы `<col>` по индексам колонок (повтор на span): фон колонки
/// рисуется в её ячейках (css-tables-3 §drawing-backgrounds).
/// Колоночная роль элемента: тег ИЛИ `display` (§17.2.1). `Some(false)` —
/// колонка, `Some(true)` — группа колонок.
pub(super) fn col_role(el: &Element) -> Option<bool> {
    match el.tag.as_str() {
        "col" => Some(false),
        "colgroup" => Some(true),
        _ => match el.style.col_role {
            Some(0) => Some(false),
            Some(1) => Some(true),
            _ => None,
        },
    }
}

/// Пролёт колонки: атрибут `span` — только HTML-ный, у элемента с колоночным
/// `display` его нет, и пролёт всегда единичный.
fn col_span(el: &Element) -> usize {
    el.attr("span")
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(1)
        .max(1)
}

pub(super) fn col_elements(children: &[Node]) -> Vec<Option<&Element>> {
    let mut out: Vec<Option<&Element>> = vec![];
    for child in children {
        let Node::Element(el) = child else { continue };
        match col_role(el) {
            Some(false) => out.extend(std::iter::repeat_n(Some(el), col_span(el))),
            Some(true) => {
                let inner = col_elements(&el.children);
                if inner.is_empty() {
                    // Группа без колонок внутри сама стоит колонкой: её
                    // пролёт и стиль ложатся на каждую дорожку.
                    out.extend(std::iter::repeat_n(Some(el), col_span(el)));
                } else {
                    out.extend(inner);
                }
            }
            None => {}
        }
    }
    out
}

/// Группы колонок по индексам дорожек: слой группы лежит ПОД слоем колонки
/// (§17.5.1) и красится отдельной полосой.
///
/// Длина и индексы совпадают с `col_elements` дорожка в дорожку: обе идут по
/// одному дереву и кладут ровно столько же записей.
///
/// Группа без своих колонок внутри уже стоит колонкой в `col_elements` —
/// второй раз её сюда не берём: буфер проб выдаётся по `node_id`, и обе
/// полосы делили бы один набор прямоугольников. Первая забрала бы его себе,
/// вторая осталась бы пустой.
pub(super) fn colgroup_elements(children: &[Node]) -> Vec<Option<&Element>> {
    let mut out: Vec<Option<&Element>> = vec![];
    for child in children {
        let Node::Element(el) = child else { continue };
        match col_role(el) {
            Some(false) => out.extend(std::iter::repeat_n(None, col_span(el))),
            Some(true) => {
                let inner = col_elements(&el.children);
                if inner.is_empty() {
                    out.extend(std::iter::repeat_n(None, col_span(el)));
                } else {
                    out.extend(std::iter::repeat_n(Some(el), inner.len()));
                }
            }
            None => {}
        }
    }
    out
}
