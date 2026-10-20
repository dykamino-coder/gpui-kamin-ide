//! Многоколоночная ветвь `element()`.
// owner: A

use crate::dom::Element;
use crate::hooks::IntoHooked;
use crate::layout::fragment::Shape;
use crate::layout::multicol::gap_rules::multicol_gap_rule_spec;
use crate::layout::options::RenderOpts;
use crate::paint::gap_rules::painter::GapRulePainter;
use crate::paint::stacking::fixed_cb_layer_box;
use crate::render::element;
use crate::style::computed::Computed;
use crate::style::values::value::Len;
use gpui::{AnyElement, ParentElement};
mod kids;
mod parallel;
use kids::{column_kids, stack_children};
mod clone_plan;
use clone_plan::{clone_plan_of, column_rule_of};
mod oof;
use oof::{column_stack_div, finish_column_stack, place_oof_statics};
mod spanner;
pub(crate) use spanner::multicol_spanner_segments;
mod fixed_height;
use fixed_height::fixed_column_height;

#[allow(clippy::too_many_arguments, clippy::needless_return)]
pub(crate) fn multicol_column_stack(
    d: gpui::Div,
    e: &Element,
    inherited: &Computed,
    merged: Computed,
    opts: &RenderOpts,
    kids: Vec<(Element, Shape)>,
    cols: u16,
    column_width: Option<Len>,
    used_gap: f32,
    row_gap: f32,
    col_axis: crate::layout::fragment::types::StackAxis,
    col_vert: bool,
    col_rl: bool,
    col_h: Option<f32>,
    line_col_w: Option<f32>,
    rows: Option<crate::layout::fragment::types::Rows>,
    nest_rows: Option<f32>,
    nest_phase: f32,
    direct_oof: Vec<Element>,
    oof_static: Vec<(usize, Element)>,
    nested_auto: std::cell::RefCell<Vec<u64>>,
    nested_whole: std::cell::RefCell<Vec<u64>>,
    measured_kids: std::cell::RefCell<Vec<(u64, f32)>>,
) -> AnyElement {
    let vcx = opts.cx.vcx();
    // Копии: без рядов `cols`, с рядами — по высоте, исключая спаннеры.
    let copies = match rows {
        Some(r) => {
            let per = r.h.unwrap_or(f32::MAX).max(1.0);
            let span = kids
                .iter()
                .filter(|(c, _)| c.style.column_span != Some(true))
                .map(|(_, s)| (s.height / per).ceil() as usize)
                .max()
                .unwrap_or(0);
            // A spanner can leave a short fragment in every column of the first row.
            // Reserve those copies in addition to the full-height rows.
            (span + cols as usize + 1).min(48)
        }
        None => cols.max(1) as usize,
    };
    // `column-fill: auto`: колонки заполняются подряд до
    // `column-height`, а без него — до высоты коробки
    // (css-multicol-1 §3.3, как прежде).
    // `column-fill: auto` заполняет колонку до БЛОЧНОГО
    // размера коробки — в вертикальном письме это ширина.
    let fixed = fixed_column_height(e, col_vert, col_h, nest_rows);
    // Распорки монолитов и параллельные flex-строки — только при fixed auto-fill.
    // Баланс по самой длинной группе был откатан: +0/−2, css-break/flexbox 319,
    // `multi-line-row-flex-fragmentation-037/038` 0.07 → «красное видно»; 033-035, 048
    // не взяты. Только колонки (без рядов) при балансе — 0/0.
    // Балансу нужен свой подбор высоты по строкам, а не оценка.
    // Строки flex и распорки роста (`grow_pushed`) меряют
    // ФИЗИЧЕСКОЕ дерево и знают только вертикальную ось —
    // в вертикальном письме их нет (следующий шаг).
    let (kid_parent, kid_starts, kid_par, kids) =
        crate::layout::fragment::with_lines(&merged, line_col_w, opts, |opts| {
            let vcx = opts.cx.vcx();
            column_kids(
                &merged, kids, cols, used_gap, col_vert, rows, copies, fixed, vcx,
            )
        });
    // `box-decoration-break: clone`: геометрия фрагментов —
    // ДО сборки копий: каждая копия такой коробки строится
    // отдельной коробкой своей высоты (`clone_fragment`).
    // Щуп — как у `grow_pushed`, но с НАСТОЯЩИМ параллельным
    // потоком соседей (та же мера, что у `StackChild` ниже):
    // иначе план соседа с потоком разошёлся бы с укладкой.
    // Без `clone` среди детей не считается вовсе.
    let clone_plan = clone_plan_of(cols, col_vert, rows, copies, fixed, &kid_par, &kids, vcx);
    let rule = column_rule_of(e, &merged, opts);
    // С рядами линейки (`column-rule` со втяжкой/разрывом,
    // `row-rule` — css-multicol-2 §rg/§crc → css-gaps-1)
    // красит `GapRulePainter` по границам колонок и
    // спаннеров из стопки (`ColumnStack::gap_items`);
    // простая полоса `rule` тогда не рисуется. Без рядов —
    // как прежде (`multicol-rule-*` не трогаются).
    let extended_rules = rows.is_some_and(|r| r.wrap)
        || e.style.row_rule_visible == Some(true)
        || e.style.column_rule_break.is_some()
        || e.style.column_rule_inset.is_some()
        || e.style.column_rule_visibility.is_some();
    let gap_spec = extended_rules
        .then(|| multicol_gap_rule_spec(e, &merged, opts, used_gap, row_gap))
        .flatten();
    let gap_items = gap_spec
        .as_ref()
        .map(|_| crate::paint::gap_rules::GapItems::default());
    let rule = rule.filter(|_| gap_spec.is_none());
    // Где начинается ребёнок в первой внешней колонке — для
    // вложенного рядами с заданной высотой (`nest_row`): все
    // предыдущие встают целиком в первую колонку, без
    // принудительных разрывов и параллельных строк flex.
    // Внешний многоколоночник с БАЛАНСОМ и единственным ребёнком —
    // вложенным заданной высоты `h` без точек разреза: баланс
    // делит его поровну, и высота внешней колонки известна до
    // укладки — `h / cols` (не выше потолка коробки; css-multicol-1
    // §7.1; `multicol-breaking-005`: 300 в трёх колонках по 100).
    // Коробка многоколоночника — содержащий блок `position: fixed`
    // (трансформ, `contain: layout|paint`): фиксированный потомок копии
    // встаёт от НЕЁ, а не от ближайшего позиционированного предка внутри
    // копии (CSS 2.1 §10.1 п.3, css-transforms-1 §transform-rendering;
    // Blink `fixedpos_containing_block`, out_of_flow_layout_part.cc).
    // Слой берёт только `fixed`: абсолюты копий режутся колонками на месте.
    let fixed_layer = fixed_cb_layer_box(&merged).then(|| {
        crate::layout::positioned::layer_scope::LayerScope::fixed_only(&opts.cx.positioning)
    });
    let children = stack_children(
        e,
        &merged,
        opts,
        cols,
        col_vert,
        col_rl,
        line_col_w,
        rows,
        nested_auto,
        nested_whole,
        measured_kids,
        copies,
        fixed,
        kid_parent,
        kid_par,
        kids,
        clone_plan,
    );
    // Щуп статической позиции — НУЛЕВОЙ записью стопки на
    // месте позиционированного ребёнка: высоты нет, полей
    // нет, точек разреза нет, монолитных диапазонов нет —
    // план укладки от него не двигается (`fill_at`:
    // `rest = 0` всегда влезает в остаток колонки), а
    // холст `interact::spot_probe` запоминает экранную
    // дырку. Сама коробка в стопку НЕ идёт: она рисуется
    // после стопки заместителем `spot_place` и сдвигается
    // в эту дырку. Тем это отличается от прежней пробы
    // «нулевая запись в стопке», о которой говорит
    // комментарий у `direct_oof`: там в колонку уходил САМ
    // элемент, и его резала маска
    // (`out-of-flow-in-multicolumn-094…097`).
    //
    // Blink берёт статическую позицию оттуда же —
    // `out_of_flow_layout_part.cc`,
    // `LayoutFragmentainerDescendants`: позиция кандидата
    // считается относительно ФРАГМЕНТАИНЕРА.
    let (children, oof_spots) = place_oof_statics(&merged, opts, &oof_static, kid_starts, children);
    // Стопка тянется по СТРОЧНОЙ оси: в вертикальном письме
    // это высота, значит коробка кладёт её гибким рядом
    // (поперечная ось растягивает высоту), у `vertical-rl` —
    // от ПРАВОГО края (`flex_row_reverse`), там начало
    // блочной оси.
    // Gap decorations paint under the content (css-gaps-1 §paint, Blink).
    let painter = gap_items.clone().zip(gap_spec);
    let d =
        d.children(painter.map(|(b, s)| GapRulePainter::new(b, s, opts.cx.paint.clone()).hooked()));
    let mut d = column_stack_div(
        d,
        e,
        inherited,
        &merged,
        cols,
        column_width,
        used_gap,
        col_axis,
        col_vert,
        col_rl,
        rows,
        nest_rows,
        nest_phase,
        fixed,
        &gap_items,
        rule,
        children,
        opts,
    );
    if let Some(layer) = fixed_layer {
        d = d.children(layer.finish());
    }
    // Флоаты — прежним ходом, соседями стопки.
    for oof in &direct_oof {
        d = d.child(element(oof, &merged, opts));
    }
    // Заместитель на месте щупа: рисуется ПОСЛЕ стопки и
    // после флоатов (позиционированная коробка выше и
    // поточного содержимого, и плавающих — CSS 2.1 §9.9,
    // шаг 8 против шагов 4 и 5; на этом держится
    // `abspos-after-spanner`, где под зеленью поточная
    // красная коробка), а встаёт туда, где щуп стоял в
    // колонке.
    // Процентная высота абсолюта — от высоты отбивки
    // содержащего блока (CSS 2.1 §10.5, §10.1 п. 4), а здесь им
    // служит САМ многоколоночник. Заместитель же кладёт коробку
    // в нулевую обёртку (`spot_place`), и раскладка под нами
    // считала проценты от неё — коробка схлопывалась в ноль
    // (`single-line-row-flex-fragmentation-019/020`: `height:
    // 50%` без `top`). Пересчитываем в точки заранее, когда
    // высота многоколоночника известна в точках.
    finish_column_stack(e, merged, opts, col_vert, oof_static, oof_spots, d)
}
