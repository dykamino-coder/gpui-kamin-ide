//! Запись стопки колонок для одного ребёнка многоколоночника (тело замыкания
//! `multicol_column_stack`).

use crate::dom::Element;
use crate::layout::fragment::clone::clone_dec;
use crate::layout::options::RenderOpts;
use crate::layout::positioned::relative::hoist_relative;
use crate::style::computed::BoxStyle;
mod factory;
mod finish;
use finish::finish_stack_child;
pub(in crate::layout::multicol) use finish::nest_candidate;
mod cuts;
use cuts::stack_child_of;
mod prepare;
pub(in crate::layout::multicol) use prepare::{Prepared, prepare};
mod breaks;
mod build;
mod fixed;
mod floats;
mod vertical;
mod whole;
use whole::{copy_kids, copy_whole_box};
mod frag_div;
mod paginate;
mod regrow;
mod through_top;
mod trim;
use crate::style::cascade::materialize::box_style;
use frag_div::frag_box_div;

#[allow(clippy::too_many_arguments)]
pub(super) fn multicol_stack_child(
    ix: usize,
    c: Element,
    prepared: Prepared,
    mt: f32,
    mb: f32,
    merged: &BoxStyle,
    opts: &RenderOpts,
    kid_parent: &[Option<BoxStyle>],
    kid_par: &[crate::layout::fragment::types::Par],
    col_vert: bool,
    col_rl: bool,
    line_col_w: Option<f32>,
    kid_w: Option<f32>,
    rows: Option<crate::layout::fragment::types::Rows>,
    copies: usize,
    fixed: Option<f32>,
    nest: Option<(f32, f32)>,
    nested_whole: &std::cell::RefCell<Vec<u64>>,
    measured_kids: &std::cell::RefCell<Vec<(u64, f32)>>,
) -> crate::layout::fragment::types::StackChild {
    // `box-decoration-break: clone` — фрагменты ЭТОГО ребёнка по итоговому плану
    // укладки (`CloneGeom`). Пока план коробку не разрезал, копия — вся коробка:
    // вид тот же, а её переполнение остаётся параллельным потоком (`clone-003`).
    let frag_geom: crate::layout::fragment::types::CloneGeom = Default::default();
    let dec = clone_dec(&c).filter(|_| !col_vert);
    // Элемент строки flex (`split_flex_lines`) наследует от
    // СВОЕГО контейнера, а не от многоколоночника.
    let merged_k = kid_parent.get(ix).cloned().flatten();
    let merged: &BoxStyle = merged_k.as_ref().unwrap_or(&merged);
    let Prepared {
        mut copy,
        empty_floats,
        float_zero,
        cuts,
        forced,
        solid,
        h,
        over,
    } = prepared;
    // Относительный сдвиг — не коробке, а фрагменту
    // (css-break-3 §5.5): его кладёт `ColumnStack`
    // вместе со срезом.
    let rel = hoist_relative(&mut copy);
    let line_trim = {
        let inner = box_style(&merged, &copy.style, opts.cx.vcx());
        let (from, need) = trim::break_trim(&copy, &merged, &inner, opts);
        // Своя первая строка коробки уже срезана (`blocks()`), верхний край
        // держит её на месте: точки перед ней нет.
        let s = copy.style.used();
        let edge = |l: &Option<crate::style::values::value::Len>| {
            !matches!(l, None | Some(crate::style::values::value::Len::Px(0.0)))
        };
        let top = !s.text_box_trim_start
            && !copy.marks.host_trim_start
            && !edge(&s.padding.top)
            && !edge(&s.borders().top)
            && matches!(
                s.height,
                None | Some(crate::style::values::value::Len::Auto)
            );
        (from > 0.0 || need > 0.0).then_some((from, need, top))
    };
    // Срез строки хоста (`Marks::host_trim_*`) — рисует
    // `blocks()` копии по её флагам.
    if copy.marks.host_trim_start {
        copy.style.used_mut().text_box_trim_start = true;
    }
    if copy.marks.host_trim_end {
        copy.style.used_mut().text_box_trim_end = true;
    }
    // Вложенный многоколоночник режут ряды внешних колонок (`nest_candidate`):
    // ряд — внешний фрагментаинер высоты `hh`, первый ряд — его остаток от места
    // ребёнка `phase` (`ColumnStack::resume_nested`). Внешняя стопка режет коробку
    // краем колонки (`fill_at`, не монолит) точно по рядам.
    let nest_row = nest
        .filter(|_| nest_candidate(ix, kid_par, col_vert, &copy))
        .map(|n| n.0)
        .filter(|hh| *hh > 0.0);
    let nest_phase_k = nest.map_or(0.0, |n| n.1);
    let report: crate::layout::fragment::token::space::sequence::NestReport = Default::default();
    let mut inner = box_style(&merged, &copy.style, opts.cx.vcx());
    // External clipping belongs to Scope.parent, not the copied subtree.
    inner.fragment_clip_ancestor = false;
    // Копии на случай разреза между колонками: элемент GPUI рисуется один раз,
    // а фрагмент нужен свой в каждой колонке. Больше, чем колонок, ребёнок
    // занять не может.
    let whole = nest_row.is_none() && nested_whole.borrow().contains(&copy.node_id);
    let own = dec.is_some() || whole || nest_row.is_some();
    let own = own || breaks::trims_at_breaks(&copy, &merged);
    let (watched, breaks_w) = breaks::recorder(
        &copy,
        measured_kids,
        opts,
        col_vert,
        col_rl,
        kid_w,
        own,
        empty_floats,
    );
    let measured_table = breaks::repeatable(&watched, &copy, breaks_w, measured_kids);
    let build = factory::new(
        opts,
        col_vert,
        col_rl,
        kid_w,
        frag_geom.clone(),
        merged,
        &copy,
        h,
        over,
        nest_row,
        nest_phase_k,
        inner,
        whole,
        watched.clone(),
        report.clone(),
    );
    // CSS Break 3 section 4.1: scroll containers, replaced and atomic boxes
    // are monolithic. Measured line edges provide breaks for ordinary flow.
    let paginate = paginate::paginate_of(opts, col_vert, col_rl, line_col_w, merged, &copy);
    let mut child = finish_stack_child(
        ix,
        mt,
        mb,
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
        build.clone(),
        measured_table,
        opts.cx.frame(),
    );
    child.clone_frags = dec.map(|_| frag_geom);
    child.line_trim = line_trim;
    let source = copy.clone();
    child.copier = Some(std::rc::Rc::new(move |part| {
        factory::copy(build.clone(), &source, col_vert, part)
    }));
    let measured = regrow::Measured {
        paginate,
        watched,
        breaks_w,
        own,
        laid_h: !float_zero,
        push_w: kid_w,
    };
    measured.attach(
        &mut child,
        opts,
        (col_vert, col_rl, kid_w),
        merged,
        &copy,
        (h, over),
    );
    // CSS 2.1 §10.3.3: у переопределённых полей отбрасывается конец по
    // направлению СОДЕРЖАЩЕГО блока, и в ltr-колонке rtl-коробка стоит у
    // левого края (Blink `ComputeMarginsFor` по стилю контейнера).
    child.par.rtl_root_in_ltr = !col_vert
        && !merged.wm().rtl()
        && !merged.wm().vertical()
        && copy.style.cascaded().wm().rtl()
        && !copy.style.cascaded().wm().vertical();
    child.nest = nest_row.map(|hh| (hh, nest_phase_k));
    child.nest_report = nest_row.map(|_| report);
    child
}
