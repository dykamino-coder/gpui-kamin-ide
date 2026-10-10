//! apply_layout, этап сетки и промежутков: baseline на инлайн-оси сетки, justify-self, проекция выравнивания, aspect-ratio, gap.

use super::*;

pub(super) fn layout_grid_gap(mut d: Div, c: &Computed) -> Div {
    // `baseline` на ИНЛАЙН-оси НАСТОЯЩЕЙ сетки не действует: элементы не
    // разделяют колоночный baseline-контекст (css-align §9.1; тест-ассерт
    // grid-self-baseline-horiz-001: «only align-self should apply») —
    // применение как items двигало содержимое вправо. У ЛУНОК инлайн-ось
    // живёт своим каналом (column-grid-lanes-item-baseline-002 полагается).
    let real_grid = matches!(c.display, Some(Display::Grid) | Some(Display::InlineGrid));
    // Теперь фильтр уже: у ГОРИЗОНТАЛЬНОЙ сетки `baseline` доходит до
    // раскладки — ортогональные (вертикальные) элементы образуют группы по
    // оси x (css-align-3 §9.1), а параллельные его не видят (бит 8 ниже;
    // `grid-justify-baseline-001`: одиночные группы `vertical-rl`/`-lr` берут
    // запасное `safe self-start` — правый и левый край, а не растяжение).
    if let Some(a) = c
        .justify_items
        .filter(|a| *a != Align::Baseline || !real_grid || c.vertical != Some(true))
    {
        d.style().justify_items = Some(self_align(a, c.justify_items_last));
    }
    if let Some(a) = c.justify_self {
        d.style().justify_self = Some(self_align(a, c.justify_self_last));
    }
    alignment_axes::abspos_normal(d.style(), c);
    alignment_axes::grid_self(d.style(), c);
    alignment_axes::project(
        d.style(),
        real_grid && c.vertical == Some(true),
        c.parent_grid >= 2,
    );
    // Биты базовой по оси x для элемента сетки (css-align-3 §9.1; Blink
    // baseline_utils.h `DetermineBaselineWritingMode`/`DetermineBaselineGroup`):
    // письмо базовой — своё у вертикального элемента, у горизонтального —
    // письмо вертикальной сетки (у горизонтальной сетки — `vertical-lr`).
    // Группа у правого края — когда это письмо `vertical-rl`. Синтез
    // центральный, когда у сетки вертикальное письмо не `sideways` (Blink
    // `parent_grid_font_baseline` = `GetFontBaseline()` сетки).
    d.style().baseline_x_flags = item_metadata::baseline_x_flags(c);
    if let Some(r) = c.aspect_ratio
        && !ratio_as_auto_min(c)
    {
        d.style().aspect_ratio = Some(r);
    }
    if let Some((row, col)) = c.gap {
        // При вертикальном письме ось блока горизонтальна: `row-gap` — зазор
        // ПО ГОРИЗОНТАЛИ, `column-gap` — по вертикали. Главную ось выше уже
        // переставили, зазор обязан ехать за ней. Сокращение `gap: 20px`
        // пишет оба поля одинаково и ошибку маскировало.
        let (down, across) = if c.vertical == Some(true) {
            (col, row)
        } else {
            (row, col)
        };
        // `calc(доля + точки)` в зазоре: доля — от своей стороны КОНТЕНТ-бокса
        // (css-gaps-1 §gap-percent: «'gap' always resolves percentages against
        // the corresponding size of the content box»). gpui знает «точки ИЛИ
        // доля», поэтому смесь разрешается здесь, когда сторона в точках
        // (`grid-gutters-011`: `calc(15% + 7px)` от 220 = 40). Сторона не
        // известна — зазор не ставится, как и прежде, когда запись
        // отбрасывалась целиком (`gap-010-ltr` держит ровно это).
        let px_or_0 = |l: Option<Len>| match l {
            Some(Len::Px(v)) => v,
            _ => 0.0,
        };
        let edges = c.borders();
        let content = |size: Option<Len>, sides: [Option<Len>; 4]| match size {
            Some(Len::Px(v)) if c.border_box == Some(true) => {
                Some((v - sides.iter().map(|s| px_or_0(*s)).sum::<f32>()).max(0.0))
            }
            Some(Len::Px(v)) => Some(v),
            _ => None,
        };
        let basis_y = content(
            c.height,
            [c.padding.top, c.padding.bottom, edges.top, edges.bottom],
        );
        let basis_x = content(
            c.width,
            [c.padding.left, c.padding.right, edges.left, edges.right],
        );
        let gap_len = |l: Len, basis: Option<f32>| -> Option<gpui::DefiniteLength> {
            // CSS Gaps 1 gap-percent: grid intrinsic sizing uses a zero
            // percentage basis, then layout resolves against the content box.
            // Preserve calc's constant term and percentage for those two phases.
            if real_grid && basis.is_none() {
                return Some(len_to_gpui(l));
            }
            if let Len::Calc(i) = l
                && let Some((k, add)) = crate::style::values::value::calc_get(i).pct_px()
            {
                return basis.map(|b| px((add + k * b).max(0.0)).into());
            }
            Some(len_to_gpui(l))
        };
        if let Some(r) = down.and_then(|r| gap_len(r, basis_y)) {
            d = d.gap_y(r);
        }
        if let Some(cg) = across.and_then(|cg| gap_len(cg, basis_x)) {
            d = d.gap_x(cg);
        }
    }
    d
}
