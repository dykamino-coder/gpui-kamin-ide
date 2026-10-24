//! Высота ячейки: ch/em/px-высоты с письмом ряда, вертикальные ячейки, снятие max-height.

use crate::dom::{Element, Node};
use crate::layout::options::RenderOpts;
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
    // em-высоты и ломал больше, чем чинил.
    if let Some(Len::Ch(k)) = cell.style.cascaded().height {
        // Флаги РЯДА, не свои: свой upright ячейки давал кегль там,
        // где эталон меряет лежачим нулём (ch-units-vrl-007/008 —
        // расхождение путей резолва div-эмуляции, вернуться при
        // унификации resolve_em).
        let upright = row
            .style
            .cascaded()
            .text_orientation
            .or(inherited.text_orientation)
            == Some(crate::style::computed::writing_mode::TextOrientation::Upright);
        let vertical = row
            .style
            .cascaded()
            .vertical_decl()
            .or(inherited.vertical_decl())
            == Some(true);
        let base = match inherited.font_size {
            Some(Len::Px(v)) => v,
            _ => opts.base_size(),
        };
        let ch = if vertical && upright {
            base
        } else {
            let family = inherited.font_family.clone().unwrap_or_default();
            crate::text::metrics::ch_ex_px(&family, base, opts.cx.vcx()).0
        };
        cell.style.used_mut().height = Some(Len::Px(k * ch));
    }
    // Ортогональная ячейка (своё письмо вертикально, таблица
    // горизонтальна) живёт в НЕповёрнутой сетке: логический
    // inline-size осел в width (resolve_logical оси не переставляет —
    // подгонка под поворотную модель), но коробку ячейки никто не
    // вращает — её строчная ось физически ВЕРТИКАЛЬНА, и размер
    // обязан лечь высотой (table-cell-align-005/006).
    if cell.style.used().wm().vertical()
        && !e.style.used().wm().vertical()
        && cell.style.used().height.is_none()
        && cell.style.used().width_from_inline
        && cell.style.used().width.is_some()
    {
        cell.style.used_mut().height = cell.style.used_mut().width.take();
    }
    // `em` на высоте ячейки — тем же точечным резолвом, что и `ch`:
    // без него логическая высота `inline-size: 2em` не проходила
    // Px-ветку ниже и min-height ряда не ставился.
    if let Some(Len::Em(k)) = cell.style.used().height {
        let base = match cell.style.used().font_size.or(inherited.font_size) {
            Some(Len::Px(v)) => v,
            _ => opts.base_size(),
        };
        cell.style.used_mut().height = Some(Len::Px(k * base));
    }
    // Предел ортогонального потока пересчитывается ПОСЛЕ переклада
    // inline-size в высоту: блок выше (у `let mut cm`) высоты ещё
    // не видел (table-cell-align-005/006).
    if cell.style.used().wm().vertical()
        && cm.ortho_limit.is_none()
        && let Some(Len::Px(h)) = cell.style.used().height
    {
        cm.ortho_limit = Some(h);
    }
    // Ортогональная ячейка (вертикальный контент от ряда) не уже
    // ТОЛЩИНЫ своей вертикальной строки — вклад стека в дорожку
    // сжимался до колонки в один глиф (ch-units-vrl-001: 19 вместо
    // line-height 100).
    if cell
        .style
        .cascaded()
        .vertical_decl()
        .or(row.style.cascaded().vertical_decl())
        .or(inherited.vertical_decl())
        == Some(true)
        && cell.style.used().min_width.is_none()
        && cell.style.used().width.is_none()
    {
        let upright = row
            .style
            .cascaded()
            .text_orientation
            .or(inherited.text_orientation)
            == Some(crate::style::computed::writing_mode::TextOrientation::Upright);
        let base = match inherited.font_size {
            Some(Len::Px(v)) => v,
            _ => opts.base_size(),
        };
        let lh_raw = cell
            .style
            .used()
            .line_height
            .or(row.style.used().line_height)
            .or(inherited.line_height);
        let lh = match lh_raw {
            Some(Len::Px(v)) => Some(v),
            Some(Len::Em(k)) => Some(k * base),
            Some(Len::Ch(k)) => Some(if upright {
                k * base
            } else {
                let family = inherited.font_family.clone().unwrap_or_default();
                k * crate::text::metrics::ch_ex_px(&family, base, opts.cx.vcx()).0
            }),
            _ => None,
        };
        if let Some(w) = lh {
            cell.style.used_mut().min_width = Some(Len::Px(w));
        }
    }
    // Высота ячейки — МИНИМУМ (css-tables §3.6): содержимое выше
    // растит ячейку, а не режется. `height: 20px` с блоком в 300
    // прятал всё под обрезкой.
    if let Some(Len::Px(h)) = cell.style.used().height {
        // Процентная высота ПРЯМОГО ребёнка решается от ЗАДАННОЙ
        // высоты ячейки (CSS 2.1 §10.5): раскладка под нами при
        // auto-росте ячейки трактует долю как auto, и ребёнок с
        // overflow и height:100% раздувался содержимым вместо
        // прокрутки в заданных ста точках.
        for child in cell.children.iter_mut() {
            if let Node::Element(el) = child
                && let Some(Len::Pct(k)) = el.style.used().height
            {
                el.style.used_mut().height = Some(Len::Px(h * k));
            }
            // Пороги той же долей — от той же заданной высоты ячейки
            // (CSS 2.1 §10.7: доля `max-height`/`min-height` считается
            // как у `height`). Нерешённая доля у нас отбрасывается, и
            // заменяемый ребёнок шёл природным размером: `<canvas
            // 200×200 max-height: 100%>` в ячейке высотой 100 давал
            // 200×200 вместо 100×100
            // (`percent-height-replaced-in-percent-cell-002`).
            if let Node::Element(el) = child
                && let Some(Len::Pct(k)) = el.style.used().max_height
            {
                el.style.used_mut().max_height = Some(Len::Px(h * k));
            }
            if let Node::Element(el) = child
                && let Some(Len::Pct(k)) = el.style.used().min_height
            {
                el.style.used_mut().min_height = Some(Len::Px(h * k));
            }
        }
        cell.style.used_mut().height = None;
        let floor = match cell.style.used().min_height {
            Some(Len::Px(v)) => v.max(h),
            _ => h,
        };
        cell.style.used_mut().min_height = Some(Len::Px(floor));
    } else {
        // Высота ячейки НЕ задана: доля ребёнка решается от высоты
        // ряда, а вклад ряда меряется БЕЗ доли (двухпроходная
        // раздача css-tables-3 §height-distribution). Однопроходное
        // приближение: якорь — собственный min-height ребёнка,
        // прокрутка держит содержимое внутри него.
        for child in cell.children.iter_mut() {
            if let Node::Element(el) = child
                && let Some(Len::Pct(k)) = el.style.used().height
                && el
                    .style
                    .used()
                    .overflow_y
                    .is_some_and(|o| o != crate::style::computed::Overflow::Visible)
                && let Some(Len::Px(m)) = el.style.used().min_height
            {
                el.style.used_mut().height = Some(Len::Px(m * k));
            }
        }
    }
    // Не чинить §17.5.2.1 (ячейка обязана влезть в свою дорожку при
    // фиксированной раскладке) ни `border_box` с нулевым `min_width`, ни
    // одним нулевым `min_width`, ни обрезкой содержимого ячейки её коробкой
    // (`overflow_hidden`): красное в `fixed-table-layout-025..031` держит не
    // минимум ячейки; нулевой минимум — даже с гейтом «`table-layout: fixed`
    // и ячейка без своей ширины» — ломает `margin-bottom-applies-to-001..007`
    // (они тоже с фиксированной раскладкой и ячейкой без ширины, признаком
    // их не отличить), а обрезка режет законно вылезающее содержимое
    // (`fixed-table-layout-003a..003f`, `017..020`).
    //
    // Потолок высоты к ячейке не применяется вовсе (браузеры
    // игнорируют max-height на ячейках): содержимое выше — растит.
    if matches!(cell.style.used().max_height, Some(Len::Px(_))) {
        cell.style.used_mut().max_height = None;
    }
}
