//! Запись стопки колонок для одного ребёнка многоколоночника (тело замыкания `multicol_column_stack`).
// owner: A

use crate::dom::{Element, Node};
use crate::layout::block::struts::through_strut;
use crate::layout::float::float_only_box;
use crate::layout::fragment::breaks::{edge_avoid, edge_break};
use crate::layout::fragment::clone::{clone_dec, clone_fragment, solid_box};
use crate::layout::fragment::fragment_size::shape_full;
use crate::layout::fragment::grid_bands::{grid_rows_px, grid_stack};
use crate::layout::fragment::line_shape::{nested_box_w, nested_rows_box, side_margin_wrap, slack_fill, transpose_tree};
use crate::layout::fragment::probe::{plain_block_tree, size_monolith, stacked_flex_tree};
use crate::layout::fragment::shape_contents::strip_through_top;
use crate::layout::fragment::table_bands::{repeat_bands, table_box};
use crate::layout::fragment::{ShapeCx, with_lines};
use crate::layout::grid::place_named_areas;
use crate::layout::multicol::gap_rules::gap_rule_spec;
use crate::layout::multicol::spanner::{multicol_inside, parallel_items_inside};
use crate::layout::page::paged::visible_overflow;
use crate::layout::positioned::predicates::{OOF_OWN, carries_abspos};
use crate::layout::positioned::relative::hoist_relative;
use crate::layout::table::table;
use crate::paint::effects::transform::transformed;
use crate::paint::stacking::fixed_cb_layer_box;
use crate::render::{RenderOpts, blocks, element, is_blank, styled_div_with};
use crate::style::cascade::inherit::inherit;
use crate::style::computed::{Computed, Display};
use crate::style::values::value::Len;
use gpui::{IntoElement, ParentElement, Styled, px};

#[allow(clippy::too_many_arguments, clippy::needless_borrow)]
pub(super) fn multicol_stack_child(
    ix: usize,
    c: Element,
    h: f32,
    mt: f32,
    mb: f32,
    cuts: Vec<(f32, f32)>,
    forced: Vec<f32>,
    solid: Vec<(f32, f32)>,
    e: &Element,
    merged: &Computed,
    opts: &RenderOpts,
    clone_plan: &[Vec<(f32, f32)>],
    kid_parent: &[Option<Computed>],
    kid_par: &[crate::layout::fragment::types::Par],
    col_vert: bool,
    col_rl: bool,
    line_col_w: Option<f32>,
    rows: Option<crate::layout::fragment::types::Rows>,
    copies: usize,
    fixed: Option<f32>,
    fixed_nest: Option<f32>,
    balanced_frag: Option<f32>,
    nest_at: &[Option<f32>],
    nested_auto: &std::cell::RefCell<Vec<u64>>,
    nested_whole: &std::cell::RefCell<Vec<u64>>,
    measured_kids: &std::cell::RefCell<Vec<(u64, f32)>>,
) -> crate::layout::fragment::types::StackChild {
    // `box-decoration-break: clone` — фрагменты ЭТОГО
    // ребёнка по плану. Неразрезанная коробка идёт
    // `slice`: вид тот же, а её переполнение остаётся
    // параллельным потоком (`clone-003`: ребёнок 185
    // в коробке 70 продолжается во второй колонке).
    let frag_geom: Vec<(f32, f32)> =
        clone_plan.get(ix).cloned().unwrap_or_default();
    let dec = clone_dec(&c).filter(|_| frag_geom.len() > 1 && !col_vert);
    // Элемент строки flex (`split_flex_lines`) наследует от
    // СВОЕГО контейнера, а не от многоколоночника.
    let merged_k = kid_parent.get(ix).cloned().flatten();
    let merged: &Computed = merged_k.as_ref().unwrap_or(&merged);
    let mut copy = c;
    // Поля кладёт укладка колонок, не коробка. В
    // вертикальном письме блочные поля — левое и
    // правое; схлопывание сквозь верх (`strip_through_top`)
    // там не считается вовсе (мера повёрнутая).
    if col_vert {
        copy.style.margin.left = None;
        copy.style.margin.right = None;
    } else {
        copy.style.margin.top = None;
        copy.style.margin.bottom = None;
        // И поле, схлопнутое СКВОЗЬ верх (`through` в
        // `mt`): стопка уже положила его `lead`-ом,
        // второй раз его вставил бы корень копии.
        strip_through_top(&mut copy, 4);
    }
    // Коробка из одних флоатов меряется высотой их
    // ряда (`float_only_box`), но сама по §10.6.3
    // высотой НОЛЬ — вне колонок это делает
    // `collapse_margins`, а копия фрагмента
    // строится мимо него. Без нуля в каждой
    // колонке проступил бы фон контейнера
    // (`floats-clear-multicol-*`: `background:
    // red`); флоаты переполняют нулевую коробку, и
    // маска колонки режет их по разрезам меры.
    if !col_vert && float_only_box(&copy).is_some() && through_strut(&copy).is_some() {
        copy.style.height = Some(Len::Px(0.0));
    }
    // Параллельный поток (css-break-3 §3):
    // содержимое, переполняющее коробку с заданной
    // высотой, продолжается в следующей колонке
    // САМО ПО СЕБЕ, а сосед встаёт сразу под
    // коробкой. Протяжённость потока — та же мера
    // без обрезки высотой; вместе с ней берём её
    // НЕусечённые точки разреза и монолитные
    // диапазоны (в пределах `h` они совпадают с
    // обычными: усечение только отбрасывает записи
    // ниже `h`).
    // ТРОЕ ворот, все замерены:
    //  1) `column-fill: auto` — иначе поток уходит
    //     в балансировку и меняет высоту колонки
    //     (`single-line-column-flex-
    //     fragmentation-051`);
    //  2) поддерево обычных блоков — иначе мера
    //     `shape_full` приближённая и
    //     протяжённость выдуманная;
    //  3) коробка своё переполнение показывает —
    //     у обрезающей и прокручиваемой потока нет.
    // Четвёртые ворота — внутри меры: бюджет
    // разжатия ОДИН на путь (`ShapeCx::unclamped`,
    // перепривязка `cx` в `shape_full`). Без него
    // разъезжался ЭТАЛОН четырёх пар
    // `flex-item-content-overflow-*`.
    let copy_m = if col_vert { transpose_tree(&copy, col_rl) } else { None };
    let (over, cuts, forced, solid) = match with_lines(&merged, line_col_w, opts, || shape_full(
        copy_m.as_ref().unwrap_or(&copy),
        4,
        ShapeCx {
            unclamped: true,
            ..ShapeCx::COLUMNS
        },
    ))
    .filter(|_| {
        // Сетка-стопка с обычными блочными элементами
        // меряется так же точно, как блок (`grid_stack`:
        // ряд = элемент), а `overflow-x: clip` блочное
        // переполнение не прячет (css-overflow-3:
        // `clip` парой к `visible` не делает коробку
        // прокручиваемой). `grid-container-
        // fragmentation-006`: сетка 200 с содержимым
        // 400 в четырёх колонках.
        let plain = plain_block_tree(&copy, 4)
            || stacked_flex_tree(&copy, 4)
            || (grid_stack(&copy)
                && copy.children.iter().all(|n| match n {
                    Node::Element(k) => k.inline || plain_block_tree(k, 3),
                    _ => true,
                }));
        let ov = copy_m.as_ref().unwrap_or(&copy);
        let block_visible = matches!(
            ov.style.overflow_y,
            None | Some(crate::style::computed::Overflow::Visible)
        ) && matches!(
            ov.style.overflow_x,
            None | Some(crate::style::computed::Overflow::Visible)
                | Some(crate::style::computed::Overflow::Clip)
        );
        fixed.is_some() && plain && block_visible
    })
    .filter(|s| s.0 > h + 0.01)
    {
        Some(s) => (s.0, s.3, s.4, s.5),
        None => (h, cuts, forced, solid),
    };
    // Мера с дотягом внепоточных — поток, а не
    // коробка (`OOF_OWN`): сосед встаёт под концом
    // коробки, абсолют продолжается в колонках.
    let (h, over) = match OOF_OWN.with(|m| m.borrow().get(&copy.node_id).copied()) {
        // Только когда за коробкой в стопке есть
        // сосед: последней коробке её дотяг —
        // мера колонок многоколоночника.
        Some((own, full))
            if (full - h).abs() < 0.01
                && !solid_box(&copy)
                && ix + 1 < kid_par.len() =>
        {
            (own, over.max(full))
        }
        _ => (h, over),
    };
    // Относительный сдвиг — не коробке, а фрагменту
    // (css-break-3 §5.5): его кладёт `ColumnStack`
    // вместе со срезом.
    let rel = hoist_relative(&mut copy);
    // Срез строки хоста (`kamin-host-trim-*`) — рисует
    // `blocks()` копии по её флагам.
    if copy.attr("kamin-host-trim-start").is_some() {
        copy.style.text_box_trim_start = true;
    }
    if copy.attr("kamin-host-trim-end").is_some() {
        copy.style.text_box_trim_end = true;
    }
    // Вложенный многоколоночник с ВЕРХА внешней колонки
    // (первый ребёнок без поля) и заданной высотой —
    // рядами во внешний фрагментаинер (`flow::OUTER_ROW`).
    // С верха колонки граница k-го ряда — ровно k·H, и
    // внешняя стопка режет коробку краем колонки
    // (`fill_at`, не монолит) точно по рядам; мера
    // коробки — её заданная высота. Сдвинутый вниз
    // (первый ряд = остаток колонки) — следующий шаг.
    let nest_row = fixed_nest.filter(|hh| {
        *hh > 0.0
            && (rows.is_none() || balanced_frag.is_some())
            && !col_vert
            && kid_par.get(ix).is_none_or(|p| p.group == 0)
            && nested_rows_box(&copy)
            && match nest_at.get(ix).copied().flatten() {
                // С верха колонки — и заданная высота, и
                // `auto` с мерой рядами.
                Some(y0) if y0 < 0.01 => {
                    matches!(copy.style.height, Some(Len::Px(_)))
                        || nested_auto.borrow().contains(&copy.node_id)
                }
                // Ниже верха — только заданная высота: мера
                // коробки от рядов не зависит.
                Some(_) => matches!(copy.style.height, Some(Len::Px(_))),
                None => false,
            }
    });
    let nest_phase_k = nest_at.get(ix).copied().flatten().unwrap_or(0.0);
    let inner = inherit(&merged, &copy.style);
    // Копии на случай разреза между колонками:
    // элемент GPUI рисуется один раз, а фрагмент
    // нужен свой в каждой колонке. Больше, чем
    // колонок, ребёнок занять не может.
    // ★ ЗАМЕРЕНО И ОТКАЧЕНО (04.09): строить копию
    // через `element(&copy, &merged, opts)`, чтобы сетка
    // и гибкий контейнер внутри стопки рисовались
    // (`grid-container-fragmentation-*`): срез
    // фрагментации 445 -> 405 (+12/−52) — рамки, тени,
    // `break-between-avoid-*`, `fieldset` ушли в
    // красное: общий путь элемента кладёт слои и
    // выносит абсолюты иначе, чем ждёт стопка.
    // Возвращать узкой веткой только для сетки.
    /// css-position-3 §abspos-breaking: «User
    /// agents must not paginate the content of
    /// fixed-positioned boxes». Копия фрагмента —
    /// ПОЛНЫЙ клон поддерева, и `position: fixed`
    /// внутри неё уезжает в слой ICB из КАЖДОЙ
    /// копии (`render.rs:1790` -> `:1823` ->
    /// `icb_push`). Слой лежит вне коробки
    /// многоколоночника, маска колонки
    /// (`flow.rs:998`) его не режет — на экране
    /// вышло бы столько зелёных коробок, сколько
    /// колонок. Оставляем фиксированного потомка
    /// только в ПЕРВОЙ копии: там же, где стоит
    /// его щуп статической позиции.
    /// Blink делает это тем же разделением —
    /// `out_of_flow_layout_part.cc:1607`: «This
    /// does not include repeated fixed-positioned
    /// elements».
    /// Устанавливает ли коробка содержащий блок для
    /// `position: fixed` (css-position-3 §fixed-cb:
    /// «the nearest ancestor box that establishes a
    /// fixed positioning containing block»;
    /// css-transforms-1 §3: трансформ даёт
    /// «containing block for all descendants … and
    /// fixed-position descendants»). Список ДОСЛОВНО
    /// тот же, что в `inline::inherit`
    /// (`transform_ancestor`): `transform`,
    /// `contain: layout`, `contain: paint`. Шире
    /// брать нельзя — `blocks` решает по `under_tf`
    /// из `inherit`, и расхождение дало бы коробку и
    /// в копии, и в слое ICB.
    fn fixed_cb_box(c: &crate::style::computed::Computed) -> bool {
        c.transform.is_some()
            || c.contain_layout == Some(true)
            || c.contain_paint == Some(true)
            || c.will_change & crate::style::computed::wc::CB_FIXED != 0
    }
    /// css-position-3 §abspos-breaking: «User
    /// agents must not paginate the content of
    /// fixed-positioned boxes». Копия фрагмента —
    /// ПОЛНЫЙ клон поддерева, и `position: fixed`
    /// внутри неё уезжает в слой ICB из КАЖДОЙ
    /// копии (`render.rs:1790` -> `:1823` ->
    /// `icb_push`). Слой лежит вне коробки
    /// многоколоночника, маска колонки
    /// (`flow.rs:998`) его не режет — на экране
    /// вышло бы столько зелёных коробок, сколько
    /// колонок. Оставляем фиксированного потомка
    /// только в ПЕРВОЙ копии: там же, где стоит
    /// его щуп статической позиции.
    /// Blink делает это тем же разделением —
    /// `out_of_flow_layout_part.cc:1607`: «This
    /// does not include repeated fixed-positioned
    /// elements».
    ///
    /// Запрет этот — про коробки, чей содержащий
    /// блок ОКНО. Если содержащий блок `fixed`
    /// лежит ВНУТРИ контекста фрагментации
    /// (трансформированный или обособленный предок
    /// внутри копии, либо сама коробка
    /// многоколоночника), коробка — обычный абсолют
    /// того предка, и §abspos-breaking выше требует
    /// обратного: «positioned relative to its
    /// containing block ignoring any fragmentation
    /// breaks … may subsequently be broken over
    /// several fragmentation containers». В слой ICB
    /// такая коробка у нас и не уходит: `blocks`
    /// (`render.rs:3966`) считает её `abs_like` при
    /// `under_tf` и оставляет НА МЕСТЕ, значит копия
    /// ≥ 1 без неё теряет единственную отрисовку.
    /// Blink делит так же:
    /// `fixedpos_containing_block`
    /// (`out_of_flow_layout_part.cc:1369, :2978`) —
    /// обычный фрагментаинерный потомок, и только
    /// оконные попадают в
    /// `repeated_fixedpos_descendants` (:1515).
    ///
    /// `fixed_cb` — встретился ли по пути ВНИЗ от
    /// коробки многоколоночника предок, который
    /// устанавливает содержащий блок для `fixed`.
    /// Предки ВЫШЕ многоколоночника сюда не входят:
    /// их содержащий блок вне контекста, коробка по
    /// спеке одна, и место ей — копия 0.
    fn drop_viewport_fixed(n: &Node, fixed_cb: bool) -> Option<Node> {
        match n {
            Node::Element(k)
                if !fixed_cb
                    && k.style.position
                        == Some(crate::style::computed::Position::Fixed) =>
            {
                None
            }
            Node::Element(k) => {
                let mut c = k.clone();
                let deeper = fixed_cb || fixed_cb_box(&k.style);
                c.children = k
                    .children
                    .iter()
                    .filter_map(|kid| drop_viewport_fixed(kid, deeper))
                    .collect();
                Some(Node::Element(c))
            }
            other => Some(other.clone()),
        }
    }
    // Линейки промежутков (css-gaps-1) у копии
    // фрагмента: слой строится ТОЛЬКО при заданном
    // стиле линейки (`gap_rule_spec`), как в
    // `element()`, — ни одна старая пара сюда не
    // попадает. Буфер проб — СВОЙ на копию: пробы
    // всех копий одного узла иначе сливаются в один
    // буфер, первая копия забирает всё (`take`) и
    // строит дорожки по смеси поднятых на `from`
    // копий. Отрезки красятся в координатах полной
    // раскладки копии, маска колонки (`flow.rs`,
    // `with_content_mask`) режет их вместе с
    // содержимым — вид `slice` css-break-3 §4.
    let copy_ix = std::cell::Cell::new(0usize);
    let whole = nest_row.is_none()
        && nested_whole.borrow().contains(&copy.node_id);
    let build = |first: bool, part: usize| {
        // `box-decoration-break: clone`: копия — САМ
        // фрагмент (`clone_fragment`), и корень, и
        // наследуемый стиль берутся у НЕГО. ★ Откат
        // 07.09 (v153) держался ровно здесь: корень
        // строился со стилем ИСХОДНОЙ коробки, и
        // каждая копия выходила полной коробкой
        // (`clone-007` 2.08 = ровно квадрат 100×100,
        // `-026` 4.98 = 240×25×4 вылета). НЕпоследний
        // фрагмент режется по съеденному содержимому,
        // последний — по концу видимого переполнения
        // (`over`, `clone-002`). Лишние копии (фрагментов
        // меньше, чем копий) не ставятся и остаются
        // исходной коробкой.
        let frag = match (dec, frag_geom.get(part)) {
            (Some((dt, db)), Some(&(from, fh))) => {
                let clip = frag_geom.get(part + 1).map_or(
                    (over - dt - db - from).max(fh - dt - db),
                    |n| n.0 - from,
                );
                Some(clone_fragment(&copy, dt, db, from, fh, clip))
            }
            _ => None,
        };
        let frag_inner =
            frag.as_ref().map(|f| inherit(&merged, &f.style));
        let src: &Element = frag.as_ref().unwrap_or(&copy);
        let src_inner: &Computed = frag_inner.as_ref().unwrap_or(&inner);
        let kids: Vec<Node> = if first {
            src.children.clone()
        } else {
            // Семя — только коробка многоколоночника
            // и корень копии. `e` считается наравне
            // с предками внутри копии: содержащий
            // блок `fixed` на самой коробке контекста
            // фрагментации — это Blink
            // `fixedpos_containing_block`
            // (`out_of_flow_layout_part.cc:1369`),
            // фрагментаинерный потомок, а не
            // повторяемая коробка. Предки ВЫШЕ `e`
            // не в счёт: их содержащий блок вне
            // контекста. `hoist_relative` выше
            // снимает лишь ВСТАВКИ,
            // `transform`/`contain` остаются на
            // месте — проверка по `copy.style`
            // законна.
            let fixed_cb_root =
                fixed_cb_box(&e.style) || fixed_cb_box(&copy.style);
            src.children
                .iter()
                .filter_map(|n| drop_viewport_fixed(n, fixed_cb_root))
                .collect()
        };
        let frag_gap_rules = gap_rule_spec(&copy, &inner, opts);
        let frag_gap_key = frag_gap_rules.as_ref().map(|_| {
            let ix = copy_ix.get();
            copy_ix.set(ix + 1);
            (copy.node_id ^ opts.doc_salt)
                ^ (ix as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        });
        let frag_gap_guard =
            frag_gap_key.map(crate::paint::gap_rules::GapGuard::enter);
        // css-break-3 §5.5: «Fragmentation … occurs
        // before relative positioning, transforms,
        // and any other graphical effects. Such
        // effects are applied per fragment». Разрезы
        // трансформ не двигают (`shape_full` его и не
        // читает), но САМ трансформ обязан быть на
        // каждом фрагменте. Общий путь вешает его
        // через `transformed()` (render.rs:1719,
        // :5938, :8185); узкая ветка копии шла мимо
        // всех трёх, и `transform` у ребёнка
        // многоколоночника пропадал целиком
        // (`transform-000…005`: `translateX(60px)`
        // контейнера гасил `left:-60px` потомков, а
        // без него содержимое уезжало из колонки).
        // Начало отсчёта пока общее на всю коробку,
        // а не своё на фрагмент, — для `translate`
        // это точно, для `rotate`/`scale` нет.
        // ★ ЗАМЕРЕНО И ОТКАЧЕНО (05.09): строить эту
        // копию через общий `element()` вместо узкой
        // ветки `styled_div_with`. Срез 3029 пар:
        // 1900 -> 1902 (+14/-12), и семь потерь —
        // грубые (99.00, страница разъезжается):
        // `multi-line-column-flex-fragmentation-035`,
        // `multi-line-row-flex-fragmentation-039/040/
        // 059`, `multicol-nested-013/021`,
        // `multicol-fill-balance-nested-000`. Тот же
        // путь, на котором прежде мерился откат -52.
        // Копия ТАБЛИЦЫ — своим рисователем.
        // `styled_div_with` + `blocks` кладут детей
        // таблицы обычными блоками, и `element()`
        // заворачивает КАЖДЫЙ ряд в СВОЮ анонимную
        // таблицу (ветка `TableRowGroup | TableRow |
        // TableCell`, render.rs:11622): дорожки
        // считаются по одному ряду, ячейка сжимается
        // по содержимому, а фон ряда и ячейки
        // теряется вовсе. Фрагментация идёт ДО
        // графических эффектов и применяется к
        // каждому фрагменту (css-break-3 §5.5), но
        // РАСКЛАДКА фрагмента — та же табличная
        // (css-tables-3 §fragmentation).
        // ЗАМЕРЕНО пробами (`target/probe-bt/`,
        // стенд v150): фон САМОЙ таблицы рисуется
        // (`p4-2col-bgtbl` 0.00) и блок с шириной в
        // точках рисуется (`p5-b-w100-div50` 0.00), а
        // фон ячейки (`p6-td-bg`), фон ряда
        // (`p6-tr-bg`), `width: auto`
        // (`p5-c-w100-divauto`) и `width: 100%`
        // (`p6-div-w100pct`) не рисуются НИЧЕМ —
        // 15625 красных точек из 15625.
        // `transformed` — как в общей ветке ниже:
        // `table()` его не вешает (в `element()`
        // таблица идёт мимо него), а копия обязана
        // нести трансформ на каждом фрагменте.
        if let Some(hh) = nest_row {
            let mut mc = copy.clone();
            mc.children = kids;
            // Своя метка узла на каждую копию: буфер линеек
            // промежутков (`gap_items_for` по `node_id`) у
            // копий одного узла сливался в один, и первая
            // копия забирала линейки всех рядов — во
            // втором и третьем ряду их не было
            // (`multicol-breaking-002`, 0.65).
            mc.node_id ^= (part as u64 + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15);
            // Ширина `auto` — по колонке (CSS 2.1 §10.3.3):
            // копия кладётся корнем, и её многоколоночнику
            // нужна ширина в точках для меры строк.
            if matches!(mc.style.width, None | Some(Len::Auto))
                && let Some(w) = line_col_w.and_then(|cw| nested_box_w(&mc, cw))
            {
                mc.style.width = Some(Len::Px(w));
            }
            // `height: auto` — высота из меры рядами
            // (`nested_rows_shape`): стопка с рядами
            // отдаёт полный последний ряд, а коробка
            // кончается на сбалансированном хвосте.
            if matches!(mc.style.height, None | Some(Len::Auto)) {
                let b = mc.style.borders();
                let px = |l: &Option<Len>| match l {
                    Some(Len::Px(v)) => *v,
                    _ => 0.0,
                };
                let bot = px(&mc.style.padding.bottom) + px(&b.bottom);
                mc.style.height = Some(Len::Px((h - bot).max(0.0)));
                mc.style.border_box = None;
            }
            drop(frag_gap_guard);
            crate::layout::fragment::types::set_outer_row(Some((hh, nest_phase_k)));
            let el = element(&mc, &merged, opts);
            crate::layout::fragment::types::set_outer_row(None);
            return el;
        }
        if whole {
            let mut mc = copy.clone();
            mc.children = kids;
            if matches!(mc.style.width, None | Some(Len::Auto))
                && let Some(w) = line_col_w.and_then(|cw| nested_box_w(&mc, cw))
            {
                mc.style.width = Some(Len::Px(w));
            }
            drop(frag_gap_guard);
            return element(&mc, &merged, opts);
        }
        if table_box(&copy) {
            let mut tc = copy.clone();
            tc.children = kids;
            drop(frag_gap_guard);
            return transformed(
                table(&tc, &inner, opts),
                &inner,
                &merged,
            );
        }
        // Абсолютный потомок ищет ближайшего
        // позиционированного предка (CSS 2.1
        // §10.1), а раскладка под нами знает только
        // непосредственного родителя: коробка, чей
        // родитель содержащим блоком НЕ является,
        // уезжает в слой (`cb_push`,
        // render.rs:4230 `to_cb`). Общий путь
        // `element()` слой заводит
        // (render.rs:13511-13529), а узкая ветка
        // копии фрагмента возвращается из
        // `element()` раньше
        // (`return d.into_any_element()`,
        // render.rs:13274) — и до сих пор такая
        // коробка либо всплывала в ЧУЖОЙ внешний
        // слой (позиционированный предок ВЫШЕ
        // многоколоночника), либо, слоя нет,
        // рисовалась на месте: от края случайного
        // родителя вместо содержащего блока.
        //
        // css-position-3 §abspos-breaking: «In a
        // fragmented flow, an absolutely positioned
        // box is positioned relative to its
        // containing block ignoring any
        // fragmentation breaks (as if the flow were
        // continuous). The box may subsequently be
        // broken over several fragmentation
        // containers». Копия и есть этот
        // непрерывный поток: `flow.rs` `prepaint`
        // кладёт её `layout_as_root(Definite(col_w),
        // Definite(full_h))` во всю высоту и
        // поднимает на срез, а колонку вырезает
        // маска — коробке, попавшей в слой КОРНЯ
        // КОПИИ, фрагментация достаётся даром. То же
        // деление у Blink: кандидат, чей содержащий
        // блок внутри контекста, идёт
        // `LayoutFragmentainerDescendants`
        // (`out_of_flow_layout_part.cc:1498`).
        //
        // Предикат — тот же `establishes_cb`, что в
        // `element()`, и по стилю КОПИИ:
        // `hoist_relative` выше снимает только
        // ВСТАВКИ, сам `position: relative` (как и
        // `transform`/`contain`) на копии остаётся.
        // Прямые дети копии ничего не меняют: у них
        // `establishes_cb(inherited)` истинно, они и
        // раньше рисовались на месте.
        // Имена областей сетки — в номера линий, как в
        // общем `element()` (`place_named_areas`): ни
        // GPUI, ни taffy имён не знают, и копия
        // фрагмента клала элементы автоматически
        // (`grid-item-fragmentation-026`: оба в
        // области `a`, второй уезжал во 2-й ряд).
        let kids = match &copy.style.grid_areas {
            Some(areas)
                if matches!(
                    copy.style.display,
                    Some(Display::Grid) | Some(Display::InlineGrid)
                ) =>
            {
                place_named_areas(areas, kids)
            }
            _ => kids,
        };
        let frag_cb_layer = crate::text::inline::establishes_cb(&inner);
        if frag_cb_layer {
            crate::layout::positioned::containing_block::cb_open_with(fixed_cb_layer_box(&inner));
        }
        let mut body = blocks(&kids, src_inner, opts);
        if frag_cb_layer {
            body.extend(crate::layout::positioned::containing_block::cb_close());
        }
        drop(frag_gap_guard);
        let mut d = styled_div_with(src, src_inner);
        // Ось блочного потока ВНУТРИ копии — горизонтальная
        // (css-writing-modes-4 §3.1), как у вертикального
        // блока в `element()`: гибкий ряд, у `vertical-rl`
        // обратный. Гибкому и сеточному ось ставит `apply`.
        if col_vert && matches!(src.style.display, None | Some(Display::Block)) {
            d = d.flex();
            d = if col_rl { d.flex_row_reverse() } else { d.flex_row() };
        }
        // Голый `styled_div_with` — БЛОК taffy (`apply.rs`
        // `apply_layout`: блоку вызова нет, gpui `Display::Block`
        // → taffy Block), а блок общего пути — гибкая колонка
        // (`d.flex().flex_col()` при пустом `display`, та же
        // оболочка у спаннера выше). На колонку опирается
        // `blocks()`: коробке с `aspect-ratio`, auto-шириной и
        // высотой в точках он ставит `Align::Start` (css-sizing-4
        // §5.1 «calculated the same as for a replaced element
        // with a natural aspect ratio»; Blink `length_utils.cc:
        // 535-562` → `FitContent`), а блочный алгоритм taffy
        // `align-self` не читает и тянет её во всю ширину
        // родителя. Прежде вылет прятала маска шириной в
        // колонку; после multicol-rest P7 (css-multicol-1 §8.1:
        // «visibly overflows and is not clipped to the column
        // box») он виден (`block-aspect-ratio-052`: зелёный 345
        // вместо 25, четыре фрагмента — 420×100). Гейт узкий —
        // только копия с таким ребёнком; колонка для ЛЮБОЙ
        // копии блока — отдельным замером.
        let ratio_kid = |n: &Node| {
            matches!(n, Node::Element(k)
                if !k.inline
                    && k.style
                        .aspect_ratio
                        .is_some_and(|r| r.is_finite() && r > 0.0)
                    && matches!(k.style.width, None | Some(Len::Auto))
                    && matches!(k.style.height, Some(Len::Px(_))))
        };
        if src.style.display.is_none()
            && src_inner.vertical != Some(true)
            && kids.iter().any(ratio_kid)
        {
            d = d.flex().flex_col();
        }
        // Для ЛЮБОЙ flex/grid-копии, не только с линейками:
        // эталоны css-gaps (`…-fragmentation-008-ref`) кладут
        // ту же сетку без правил, и с гейтом «только с
        // линейками» тест рисовал сетку, а эталон — нет
        // (v93: 008 3.75, 009 5.18, 010 4.50).
        // ★ ЗАМЕРЕНО И ОТКАЧЕНО (06.09, v94): то же для
        // flex-копий. css-break 2874: +26/−15, и девять потерь
        // — 99.00 (`multi-line-row-flex-fragmentation-084…090`,
        // `multi-line-column-flex-fragmentation-056/057`:
        // страница разъезжается), ещё 065–071 на 1.5–13.
        // Сетка даёт +17 в css-break без потерь.
        // Flex-копия — во всю ширину колонки, как и
        // сетка: flex-корень с `width: auto` taffy
        // кладёт шириной СОДЕРЖИМОГО (`flexbox.rs`
        // `determine_container_main_size`, ветвь
        // `Definite` → `longest_line_length`), и
        // элемент `width: 100%` выходил нулевым, а
        // `width: 100px` в колонке 50 не сжимался.
        // Замер одного этого (v94): +9/−15, все
        // потери — `row-gap`, их закрывает мера
        // гибкой стопки (`flex_items` в `shape_full`).
        if matches!(copy.style.width, None | Some(Len::Auto))
            && matches!(
                copy.style.display,
                Some(Display::Grid)
                    | Some(Display::InlineGrid)
                    | Some(Display::Flex)
            )
        {
            d = d.w_full();
        }
        // Пустая сетка: taffy раскладывает бездетный
        // узел ЛИСТОМ (vendor/taffy/src/tree/
        // taffy_tree.rs: `(_, false) =>
        // compute_leaf_layout`), явные дорожки ему
        // не видны, и копия выходила нулевой при
        // мере 200 (`grid-container-fragmentation-
        // 002`: `grid-template-rows: 200px`). Дорожка
        // существует без элементов (css-grid-1
        // §7.1) — высота копии та же, что в мере.
        if kids.is_empty()
            && matches!(
                copy.style.display,
                Some(Display::Grid) | Some(Display::InlineGrid)
            )
            && grid_rows_px(&copy.style).is_some()
        {
            d = d.min_h(px(h));
        }
        if let (Some(key), Some(spec)) = (frag_gap_key, frag_gap_rules) {
            // Копия кладётся `layout_as_root(Definite(col_w), …)`
            // (`flow.rs` `ColumnStack::prepaint`), а taffy у
            // flex/grid-КОРНЯ с `width: auto` берёт размер
            // содержимого, не доступное место
            // (`vendor/taffy/src/compute/flexbox.rs`
            // `determine_container_main_size`, ветвь
            // `Definite` → `longest_line_length`): дорожки
            // `1fr` выходили нулевыми, и вся сетка была
            // невидима (`grid-gap-decorations-fragmentation-
            // 008/010/016`: только серый фон). Блок
            // растягивается сам; flex/grid получают 100% —
            // корень разрешает долю против `available_space`
            // (`taffy/src/compute/mod.rs` `compute_root_layout`).
            // Под детьми копии — как в `element()`
            // (css-gaps-1: «just above the border»).
            body.insert(
                0,
                crate::paint::gap_rules::painter::GapRulePainter::new(
                    crate::paint::gap_rules::gap_items_for(key),
                    spec,
                )
                .into_any_element(),
            );
        }
        transformed(
            d.children(body).into_any_element(),
            &inner,
            &merged,
        )
    };
    // Монолиты (css-break-3 §4.1) — их разрыв
    // запрещён, и в следующую колонку они уходят
    // целиком: `break-inside: avoid`,
    // прокручиваемая или обрезающая коробка,
    // замещаемый элемент, таблица и ячейка,
    // атомарная строчная коробка. Сюда же —
    // сплошной СТРОЧНЫЙ набор: резать его можно
    // только между строками, а строк укладка
    // колонок не видит, и разрез приходился бы
    // посреди строки.
    // Монолитен ПРОКРУЧИВАЕМЫЙ контейнер (css-break-4
    // §4.1 «scroll containers»); `hidden`/`clip` —
    // обрезка, не прокрутка, и режется как блок
    // (корень A4).
    let scrolls = |o: Option<crate::style::computed::Overflow>| {
        matches!(o, Some(crate::style::computed::Overflow::Scroll))
    };
    let block_kid = |n: &Node| {
        matches!(n, Node::Element(k)
            if !k.inline || k.style.display == Some(Display::Block))
    };
    // ★ ЗАМЕРЕНО И ОТКАЧЕНО (04.09): `contain: size` как
    // монолит (Blink `IsMonolithic`) — срез фрагментации
    // 469 -> 467 (+1/−3): `single-line-column-flex-
    // fragmentation-051/063` режутся у Blink иначе (рост
    // элемента от фрагментации, корень R5 скаута).
    // Рост лёг `705fd58`; `contain: size` — `size_monolith`,
    // тот же предикат, что у `solid_box` в мере и пробе
    // `grow_pushed` (`scout-break-2026-09e.md`).
    let monolith = nest_row.is_none() && (size_monolith(&copy)
        || copy.style.break_inside_avoid
        || scrolls(copy.style.overflow_x)
        || scrolls(copy.style.overflow_y)
        || matches!(
            copy.tag.as_str(),
            "img"
                | "svg"
                | "canvas"
                | "video"
                | "embed"
                | "object"
                | "iframe"
        )
        // Таблица и ячейка — не монолиты
        // (css-break-4 §4.1).
        || matches!(
            copy.style.display,
            Some(Display::InlineBlock)
                | Some(Display::InlineFlex)
                | Some(Display::InlineGrid)
        )
        // Сплошной СТРОЧНЫЙ набор тоже монолит:
        // резать его можно лишь между строками, а
        // строк укладка колонок не видит, и разрез
        // приходился бы посреди строки.
        // ПУСТАЯ коробка с высотой режется по своей
        // высоте (css-break-4 §4.2; корень A3).
        || (copy.children.iter().any(|n| !is_blank(n))
            && !copy.children.iter().any(block_kid)
            // Строки измерены (`line_run_shape`) — режется
            // между строк, не монолит.
            && cuts.is_empty()));
    // Высоту меряет раскладка копии (`StackChild::measure`):
    // точек разреза мера не дала, и строчный набор без них
    // не монолит — режется краем колонки. Монолит — только
    // по собственным причинам коробки (css-break-3 §4.1).
    let measure = measured_kids
        .borrow()
        .iter()
        .find(|(id, _)| *id == copy.node_id)
        .map(|(_, w)| *w);
    let monolith = if whole {
        true
    } else if measure.is_some() {
        nest_row.is_none()
            && (size_monolith(&copy)
                || copy.style.break_inside_avoid
                || scrolls(copy.style.overflow_x)
                || scrolls(copy.style.overflow_y))
    } else {
        monolith
    };
    // Пока строятся копии — «внутри стопки»: вложенный
    // многоколоночник со спаннером остаётся на
    // сегментном пути (см. `unified` выше).
    let _nested = crate::layout::fragment::types::StackScope::enter();
    let span = copy.style.column_span == Some(true) && !copy.inline;
    // Переполняющие колонки (css-multicol-1 §8.2: «A multicol
    // container can have more columns than it has room for due
    // to: a declaration that constrains the column height … In
    // this case, additional column boxes are created in the
    // inline direction») — ТОЛЬКО ребёнку, который несёт
    // абсолютного потомка: его содержащий блок сплошной, и
    // абсолют режется по колонкам сам (css-position-3
    // §abspos-breaking), а копий у ребёнка было ровно
    // `column-count` — хвост уходил «за кадр» (`flow.rs`
    // `fill_at`, `copy + 1 >= limit`;
    // `out-of-flow-in-multicolumn-007`: CB 300 при колонке 100,
    // копий 2 из 3). ★ Прежний патч без гейта «несёт абсолют»
    // (scout-fragoof-2026-09d §7) замерен +7/−14: все потери —
    // дети БЕЗ абсолютов (вложенные многоколоночники, флекс,
    // `multicol-fill-balance-*`), у которых мера `shape_full`
    // не совпадает с рисунком. Гейт `plain_block_tree` — тот же,
    // что у параллельного потока выше. Прочим детям — прежнее
    // число копий, и стопка без такого ребёнка байт-в-байт
    // прежняя (`ColumnStack::new` берёт наибольшее число копий).
    let kid_copies = match fixed {
        Some(per)
            if rows.is_none()
                && !span
                && per > 0.0
                && visible_overflow(&e.style)
                && visible_overflow(&copy.style)
                && plain_block_tree(&copy, 4)
                && carries_abspos(&copy, 4) =>
        {
            ((h.max(over) / per).ceil() as usize + 1)
                .min(16)
                .max(copies)
        }
        // Высота неизвестна до раскладки: копий — на
        // переполняющие колонки (css-multicol-1 §8.2).
        Some(per) if measure.is_some() && rows.is_none() && !span && per > 0.0 => {
            copies.max(8).min(16)
        }
        _ => copies,
    };
    crate::layout::fragment::types::StackChild {
        measure,
        el: side_margin_wrap(build(true, 0), &copy, col_vert),
        frags: if span {
            Vec::new()
        } else {
            (1..kid_copies)
                .map(|i| side_margin_wrap(build(false, i), &copy, col_vert))
                .collect()
        },
        monolith,
        cuts,
        // Тот же подъём, что в мере (Х5): иначе
        // укладка колонок не увидит разрыва,
        // который мера уже посчитала.
        force_before: edge_break(&copy, false),
        force_after: edge_break(&copy, true),
        avoid_before: edge_avoid(&copy, false),
        avoid_after: edge_avoid(&copy, true),
        forced,
        solid,
        h,
        mt,
        mb,
        span,
        over,
        rel,
        clone_dec: dec,
        // Монолит с верха колонки переполняет её
        // (`flow.rs` `fill_at`, `overflow_to`) —
        // только при `column-fill: auto` без рядов
        // и без элементов ряда в поддереве.
        overflow_top: fixed.is_some()
            && rows.is_none()
            && !parallel_items_inside(&copy, 4),
        // Вложенный многоколоночник — маска режет вбок
        // (`flow.rs` `StackChild::nested_cols`).
        nested_cols: multicol_inside(&copy, 4),
        par: kid_par[ix],
        positioned: !span
            && (matches!(
                copy.style.position,
                Some(crate::style::computed::Position::Relative)
                    | Some(crate::style::computed::Position::Sticky)
            ) || copy.style.transform.is_some())
            && copy.style.z_index.unwrap_or(0) == 0,
        // Хвост непоследнего фрагмента таблицы — её фоном
        // (`flow.rs` `StackChild::slack`).
        slack: if col_vert {
            None
        } else if table_box(&copy) {
            copy.style.background.map(|c| c.to_hsla())
        } else if dec.is_none() && over <= h + 0.01 {
            // Продолжение одного лишь параллельного
            // потока (`over`) — не продолжение коробки:
            // она кончилась, хвоста у неё нет.
            slack_fill(&copy)
        } else {
            None
        },
        laid_w: Default::default(),
        // Повтор шапки/подвала таблицы — полосы своими
        // копиями (`flow::Repeat`); та же мера, что у
        // щупов (`repeat_leads`).
        repeat: repeat_bands(&copy, fixed, rows)
            .filter(|_| !span && !col_vert)
            .map(|(head, foot, geom)| crate::layout::fragment::types::Repeat {
                head,
                foot,
                geom,
                head_els: match head {
                    Some(_) => (1..kid_copies).map(|i| build(false, i)).collect(),
                    None => Vec::new(),
                },
                foot_els: match foot {
                    Some(_) => (0..kid_copies).map(|i| build(false, i)).collect(),
                    None => Vec::new(),
                },
            }),
    }
}
