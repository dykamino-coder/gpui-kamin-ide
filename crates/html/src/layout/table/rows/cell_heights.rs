//! Высота ячейки: ch/em/px-высоты с письмом ряда, вертикальные ячейки, снятие max-height.

use crate::dom::{Element, Node};
use crate::render::RenderOpts;
use crate::style::computed::Computed;
use crate::style::values::value::Len;

pub(super) fn resolve_cell_heights(
    opts: &RenderOpts,
    e: &Element,
    row: &Element,
    inherited: &Computed,
    cm: &mut Computed,
    cell: &mut Element,
) {
    // `ch` на высоте ячейки разрешается с письмом РЯДА: при
    // vertical + upright продвижение нуля — кегль (css-values-3,
    // ch-units-vrl-*). Только ch: полный resolve_em здесь двигал
    // em-высоты и был нетто-минусом (замерено, откат 8b59418-ядра).
    if let Some(Len::Ch(k)) = cell.style.height {
        // Флаги РЯДА, не свои: свой upright ячейки давал кегль там,
        // где эталон меряет лежачим нулём (ch-units-vrl-007/008 —
        // расхождение путей резолва div-эмуляции, вернуться при
        // унификации resolve_em).
        let upright = row.style.upright.or(inherited.upright) == Some(true);
        let vertical = row.style.vertical.or(inherited.vertical) == Some(true);
        let base = match inherited.font_size {
            Some(Len::Px(v)) => v,
            _ => opts.base_size(),
        };
        let ch = if vertical && upright {
            base
        } else {
            let family = inherited.font_family.clone().unwrap_or_default();
            crate::text::metrics::ch_ex_px(&family, base).0
        };
        cell.style.height = Some(Len::Px(k * ch));
    }
    // Ортогональная ячейка (своё письмо вертикально, таблица
    // горизонтальна) живёт в НЕповёрнутой сетке: логический
    // inline-size осел в width (resolve_logical оси не переставляет —
    // подгонка под поворотную модель), но коробку ячейки никто не
    // вращает — её строчная ось физически ВЕРТИКАЛЬНА, и размер
    // обязан лечь высотой (table-cell-align-005/006).
    if cell.style.vertical == Some(true)
        && e.style.vertical != Some(true)
        && cell.style.height.is_none()
        && cell.style.width_from_inline
        && cell.style.width.is_some()
    {
        cell.style.height = cell.style.width.take();
    }
    // `em` на высоте ячейки — тем же точечным резолвом, что и `ch`:
    // без него логическая высота `inline-size: 2em` не проходила
    // Px-ветку ниже и min-height ряда не ставился.
    if let Some(Len::Em(k)) = cell.style.height {
        let base = match cell.style.font_size.or(inherited.font_size) {
            Some(Len::Px(v)) => v,
            _ => opts.base_size(),
        };
        cell.style.height = Some(Len::Px(k * base));
    }
    // Предел ортогонального потока пересчитывается ПОСЛЕ переклада
    // inline-size в высоту: блок выше (у `let mut cm`) высоты ещё
    // не видел (table-cell-align-005/006).
    if cell.style.vertical == Some(true)
        && cm.ortho_limit.is_none()
        && let Some(Len::Px(h)) = cell.style.height
    {
        cm.ortho_limit = Some(h);
    }
    // Ортогональная ячейка (вертикальный контент от ряда) не уже
    // ТОЛЩИНЫ своей вертикальной строки — вклад стека в дорожку
    // сжимался до колонки в один глиф (ch-units-vrl-001: 19 вместо
    // line-height 100).
    if cell
        .style
        .vertical
        .or(row.style.vertical)
        .or(inherited.vertical)
        == Some(true)
        && cell.style.min_width.is_none()
        && cell.style.width.is_none()
    {
        let upright = row.style.upright.or(inherited.upright) == Some(true);
        let base = match inherited.font_size {
            Some(Len::Px(v)) => v,
            _ => opts.base_size(),
        };
        let lh_raw = cell
            .style
            .line_height
            .or(row.style.line_height)
            .or(inherited.line_height);
        let lh = match lh_raw {
            Some(Len::Px(v)) => Some(v),
            Some(Len::Em(k)) => Some(k * base),
            Some(Len::Ch(k)) => Some(if upright {
                k * base
            } else {
                let family = inherited.font_family.clone().unwrap_or_default();
                k * crate::text::metrics::ch_ex_px(&family, base).0
            }),
            _ => None,
        };
        if let Some(w) = lh {
            cell.style.min_width = Some(Len::Px(w));
        }
    }
    // Высота ячейки — МИНИМУМ (css-tables §3.6): содержимое выше
    // растит ячейку, а не режется. `height: 20px` с блоком в 300
    // прятал всё под обрезкой.
    if let Some(Len::Px(h)) = cell.style.height {
        // Процентная высота ПРЯМОГО ребёнка решается от ЗАДАННОЙ
        // высоты ячейки (CSS 2.1 §10.5): раскладка под нами при
        // auto-росте ячейки трактует долю как auto, и ребёнок с
        // overflow и height:100% раздувался содержимым вместо
        // прокрутки в заданных ста точках.
        for child in cell.children.iter_mut() {
            if let Node::Element(el) = child
                && let Some(Len::Pct(k)) = el.style.height
            {
                el.style.height = Some(Len::Px(h * k));
            }
            // Пороги той же долей — от той же заданной высоты ячейки
            // (CSS 2.1 §10.7: доля `max-height`/`min-height` считается
            // как у `height`). Нерешённая доля у нас отбрасывается, и
            // заменяемый ребёнок шёл природным размером: `<canvas
            // 200×200 max-height: 100%>` в ячейке высотой 100 давал
            // 200×200 вместо 100×100
            // (`percent-height-replaced-in-percent-cell-002`).
            if let Node::Element(el) = child
                && let Some(Len::Pct(k)) = el.style.max_height
            {
                el.style.max_height = Some(Len::Px(h * k));
            }
            if let Node::Element(el) = child
                && let Some(Len::Pct(k)) = el.style.min_height
            {
                el.style.min_height = Some(Len::Px(h * k));
            }
        }
        cell.style.height = None;
        let floor = match cell.style.min_height {
            Some(Len::Px(v)) => v.max(h),
            _ => h,
        };
        cell.style.min_height = Some(Len::Px(floor));
    } else {
        // Высота ячейки НЕ задана: доля ребёнка решается от высоты
        // ряда, а вклад ряда меряется БЕЗ доли (двухпроходная
        // раздача css-tables-3 §height-distribution). Однопроходное
        // приближение: якорь — собственный min-height ребёнка,
        // прокрутка держит содержимое внутри него.
        for child in cell.children.iter_mut() {
            if let Node::Element(el) = child
                && let Some(Len::Pct(k)) = el.style.height
                && el
                    .style
                    .overflow_y
                    .is_some_and(|o| o != crate::style::computed::Overflow::Visible)
                && let Some(Len::Px(m)) = el.style.min_height
            {
                el.style.height = Some(Len::Px(m * k));
            }
        }
    }
    // ПРОБОВАЛИ И ОТКАТИЛИ (§17.5.2.1, ячейка обязана влезть в свою
    // дорожку при фиксированной раскладке): `border_box` + нулевой
    // `min_width` — потеряно 7 (`margin-bottom-applies-to-001..007`),
    // приобретено 0; один нулевой `min_width` — 0 и 0. Красное в
    // `fixed-table-layout-025..031` держит не минимум ячейки.
    // ПЕРЕПРОВЕРЕНО УЖЕ: сужение гейта до `table-layout: fixed` плюс
    // ячейка без своей ширины — те же семь потерь
    // (`margin-bottom-applies-to-001..007` 0.03 -> 3.80), флипов ноль.
    // Они тоже с фиксированной раскладкой; ячейка там без ширины, и
    // отличить их от `025..031` этим признаком нельзя.
    // ТРЕТЬЯ ПОПЫТКА, ЗАМЕРЕНА И ОТКАЧЕНА (01.09): не трогать размер
    // вовсе, а обрезать содержимое ячейки её коробкой
    // (`overflow_hidden` при `table-layout: fixed` и ячейке без своей
    // ширины). Срез из 81 пары `fixed-table-layout-*` +
    // `margin-bottom-applies-to-*`: зелёных 69 → 12. Обрезка режет
    // законно вылезающее содержимое — вся семья `003a..003f`
    // ушла 0.00 → 2.00, `017..020` 0.00 → 1.55.
    //
    // Потолок высоты к ячейке не применяется вовсе (браузеры
    // игнорируют max-height на ячейках): содержимое выше — растит.
    if matches!(cell.style.max_height, Some(Len::Px(_))) {
        cell.style.max_height = None;
    }
}
