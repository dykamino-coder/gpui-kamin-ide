//! Колонки таблицы: дорожки и ширины.
// owner: A

use crate::render::*;

/// Дорожки таблицы: все по содержимому, последняя забирает остаток строки.
///
/// Ширины колонок считают ИМЕННО дорожки: это внутренние размеры содержимого,
/// посчитанные раскладкой по настоящим правилам переноса. Прежде поверх них
/// работал свой замер (`MeasuredTable`), меривший ГОЛЫЙ текст ячейки — без
/// переносов, сохранённых пробелов и вложенных коробок; снят по замеру:
/// css-text +1, flexbox +1, css-grid +1, поломок нет.
/// `min-content` снизу не даёт колонке сжаться в ноль на узкой панели.
pub(crate) fn track_list_collapsed(
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

pub(crate) fn track_list(
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
pub(crate) fn col_role(el: &Element) -> Option<bool> {
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
pub(crate) fn col_span(el: &Element) -> usize {
    el.attr("span")
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(1)
        .max(1)
}

pub(crate) fn col_elements(children: &[Node]) -> Vec<Option<&Element>> {
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
pub(crate) fn colgroup_elements(children: &[Node]) -> Vec<Option<&Element>> {
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

/// Полоса слоя на каждую красящую дорожку плюс буфер проб по её индексу.
///
/// Одно тело на слой групп и слой колонок: различаются они только набором
/// элементов и порядком вызова (§17.5.1 — группы ПОД колонками).
pub(crate) fn push_col_bands<'a>(
    els: &[Option<&'a Element>],
    salt: u64,
    have_rows: bool,
    rects_by_col: &mut [Option<crate::interact::RowRects>],
    cells: &mut Vec<AnyElement>,
) {
    let mut seen: Vec<u64> = vec![];
    for (i, el) in els.iter().enumerate() {
        let Some(el) = el else { continue };
        // Полоса красит СВОИ ЯЧЕЙКИ. Ячеек нет — красить нечего, а пустая
        // полоса ещё и просит перерисовку два кадра подряд впустую
        // (`table-column-rendering-001`: колонка сама по себе не рисуется).
        // Дорожка за краем сетки буфера не получает: её пробы не напишет
        // никто.
        if !have_rows || i >= rects_by_col.len() {
            continue;
        }
        let picture = el.style.bg_image.is_some() || el.style.gradient_raw.is_some();
        // Дорожка с одним ЦВЕТОМ тоже красится полосой: своей коробки у неё
        // нет, фон рисуют её ячейки.
        if !(picture || el.style.background.is_some() || !el.style.shadows.is_empty()) {
            continue;
        }
        let rects = crate::interact::row_rects_for(el.node_id ^ salt);
        rects_by_col[i] = Some(rects.clone());
        if !seen.contains(&el.node_id) {
            seen.push(el.node_id);
            let mut band_style = el.style.clone();
            if band_style.bg_image.is_none() {
                band_style.bg_image = band_style.gradient_raw.clone();
            }
            cells.push(crate::interact::CellsClipped::new(rects, band_style).into_any_element());
        }
    }
}

pub(crate) fn col_element_widths(
    children: &[Node],
    base_font: f32,
    family: &str,
) -> (Vec<Option<f32>>, Vec<bool>, Vec<Option<f32>>) {
    let mut widths = vec![];
    let mut collapsed = vec![];
    // Доля ширины колонки (§17.5.2.1): в точках её не выразить, дорожка
    // получает её отдельно.
    let mut pcts: Vec<Option<f32>> = vec![];
    // §17.5.2.1 берёт ИСПОЛЬЗОВАННУЮ ширину колонки, а вычисляет её §10.4:
    // сперва пробная ширина, потом потолок `max-width`, потом пол
    // `min-width`. Прежде читалась одна `width`, и `min-width: 1in` на
    // колонке без ширины оставляла дорожку `Fraction(1)` в столе автоширины
    // — то есть НОЛЬ и пустой холст (`min-width-applies-to-005/006`), а
    // `width: 3in` рядом с `max-width: 1in` держала все 288
    // (`max-width-applies-to-005/006`).
    //
    // Потолок применяется ТОЛЬКО к ЗАЯВЛЕННОЙ ширине: одинокий `max-width`
    // авто-дорожку закреплять не вправе — верхняя грань дорожки живёт в
    // `track_list`, и отдельная пара (пол, потолок) на дорожку — следующий
    // шаг, не этот.
    let used_w = |el: &Element, w: Option<f32>| -> Option<f32> {
        let len_px = |l: Option<Len>| match l {
            Some(Len::Px(v)) => Some(v),
            _ => None,
        };
        let capped = match (w, len_px(el.style.max_width)) {
            (Some(v), Some(mx)) => Some(v.min(mx)),
            (v, _) => v,
        };
        match len_px(el.style.min_width) {
            Some(mn) => Some(capped.map_or(mn, |v| v.max(mn))),
            None => capped,
        }
    };
    for child in children {
        let Node::Element(el) = child else { continue };
        match col_role(el) {
            Some(false) => {
                let w = used_w(
                    el,
                    match el.style.width {
                        Some(Len::Px(v)) => Some(v),
                        // `ch` на колонке считается с ЕЁ письмом: стоячий ноль
                        // продвигается на кегль (ch-units-vrl-003/004). Кегль
                        // колонки — свой или умолчание: шрифт таблицы сюда не
                        // наследуется, а тесты задают его одинаковым.
                        // Только СТОЯЧИЕ (upright): там продвижение — кегль и
                        // сходится с эталоном. Лежачий `ch` (sideways) оставлен
                        // авто-колонке: явный глиф-замер уводил ширину
                        // (ch-units-vrl-007/008 были зелёными на авто).
                        Some(Len::Ch(k))
                            if el.style.upright == Some(true)
                                && el.style.vertical == Some(true) =>
                        {
                            let base = match el.style.font_size {
                                Some(Len::Px(v)) => v,
                                _ => base_font,
                            };
                            let _ = family;
                            Some(k * base)
                        }
                        _ => None,
                    },
                );
                // `visibility: collapse` на колонке — колонка ВЫБРОШЕНА:
                // нулевая дорожка, ячейки не рисуются (css-tables-3
                // §visibility-collapse-cell-rendering).
                let c = el.style.collapsed == Some(true);
                let span = col_span(el);
                let p = match el.style.width {
                    Some(Len::Pct(k)) => Some(k),
                    _ => None,
                };
                widths.extend(std::iter::repeat_n(w, span));
                collapsed.extend(std::iter::repeat_n(c, span));
                pcts.extend(std::iter::repeat_n(p, span));
            }
            Some(true) => {
                let (w, c, p) = col_element_widths(&el.children, base_font, family);
                if w.is_empty() {
                    // Группа без колонок внутри САМА стоит колонкой (см.
                    // `col_elements`), значит и §10.4 к ней применяется тот
                    // же — иначе `min-width-applies-to-005` и
                    // `max-width-applies-to-005` (группа вместо колонки)
                    // остались бы красными при зелёных `-006`.
                    let ww = used_w(
                        el,
                        match el.style.width {
                            Some(Len::Px(v)) => Some(v),
                            _ => None,
                        },
                    );
                    let pp = match el.style.width {
                        Some(Len::Pct(k)) => Some(k),
                        _ => None,
                    };
                    let cc = el.style.collapsed == Some(true);
                    let span = col_span(el);
                    widths.extend(std::iter::repeat_n(ww, span));
                    collapsed.extend(std::iter::repeat_n(cc, span));
                    pcts.extend(std::iter::repeat_n(pp, span));
                } else {
                    widths.extend(w);
                    collapsed.extend(c);
                    pcts.extend(p);
                }
            }
            None => {}
        }
    }
    (widths, collapsed, pcts)
}
