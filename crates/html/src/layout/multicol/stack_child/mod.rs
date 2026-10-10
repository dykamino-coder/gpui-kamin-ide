//! Запись стопки колонок для одного ребёнка многоколоночника (тело замыкания `multicol_column_stack`).
// owner: A

use crate::dom::Element;
use crate::layout::block::struts::through_strut;
use crate::layout::float::float_only_box;
use crate::layout::fragment::clone::clone_dec;
use crate::layout::fragment::shape_contents::strip_through_top;
use crate::layout::positioned::relative::hoist_relative;
use crate::render::RenderOpts;
use crate::style::cascade::inherit::inherit;
use crate::style::computed::Computed;
use crate::style::values::value::Len;
mod finish;
use finish::{finish_stack_child, nest_row_of};
mod cuts;
use cuts::{copy_cuts_and_height, stack_child_of};
mod build;
mod fixed;
use build::build_copy;
mod whole;
use whole::{copy_kids, copy_whole_box};
mod frag_div;
use frag_div::frag_box_div;

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
    let frag_geom: Vec<(f32, f32)> = clone_plan.get(ix).cloned().unwrap_or_default();
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
    let (cuts, forced, solid, h, over) = copy_cuts_and_height(
        ix, h, cuts, forced, solid, opts, kid_par, col_vert, col_rl, line_col_w, fixed, merged,
        &copy,
    );
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
    let nest_row = nest_row_of(
        ix,
        kid_par,
        col_vert,
        rows,
        fixed_nest,
        balanced_frag,
        nest_at,
        nested_auto,
        &copy,
    );
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
    let whole = nest_row.is_none() && nested_whole.borrow().contains(&copy.node_id);
    let build = |first: bool, part: usize| {
        build_copy(
            e,
            opts,
            col_vert,
            col_rl,
            line_col_w,
            &frag_geom,
            dec,
            merged,
            &copy,
            h,
            over,
            nest_row,
            nest_phase_k,
            &inner,
            &copy_ix,
            whole,
            first,
            part,
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
    finish_stack_child(
        ix,
        mt,
        mb,
        e,
        kid_par,
        col_vert,
        rows,
        copies,
        fixed,
        measured_kids,
        dec,
        &copy,
        cuts,
        forced,
        solid,
        h,
        over,
        rel,
        nest_row,
        whole,
        build,
    )
}
