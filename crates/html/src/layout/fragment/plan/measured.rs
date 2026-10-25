//! План укладки с повторной мерой содержимого в новом фрагментаинере.
use super::{Plan, Space, fill_pieces, open_kid};
use crate::layout::fragment::types::Kid;

/// Ограничения и курсор содержимого перед мерой очередной копии. Полная высота
/// фрагментаинера отличается от оставшегося места и от процентной базы.
#[derive(Clone, Copy, Debug)]
pub(crate) struct MeasureRequest {
    pub(crate) kid: usize,
    pub(crate) copy: usize,
    pub(crate) col: usize,
    pub(crate) from: f32,
    pub(crate) block_size: f32,
    pub(crate) page: Option<crate::layout::fragment::token::space::PageIdentity>,
}

/// IFC принадлежат снимку страничного consumer; общий Kid не хранит их адреса.
#[derive(Default)]
pub(crate) struct Measurement {
    pub(crate) copy: Option<(Kid, f32)>,
    pub(crate) inline: crate::layout::fragment::flow::inline::Geometry,
}

// Filling returns column geometry and continuation metadata together.
pub(crate) fn fill_measured(
    kids: &[Kid],
    target_at: &dyn Fn(usize) -> f32,
    space: Space,
    remeasure: &mut super::Remeasure<'_>,
) -> (usize, f32, Plan) {
    // Потребители копий сохраняют прежний контракт; token consumer ошибок не скрывает.
    fill_resumed(
        kids,
        &|col| crate::layout::fragment::token::Space {
            block_size: Some(target_at(col)),
            ..space.space
        },
        space,
        &|_, _| None,
        &mut |request| Measurement {
            copy: remeasure(request),
            inline: Default::default(),
        },
    )
    .expect("copy planner does not execute tokens")
}

#[allow(clippy::type_complexity)]
pub(crate) fn fill_resumed(
    kids: &[Kid],
    space_at: &dyn Fn(usize) -> crate::layout::fragment::token::Space,
    space: Space,
    leaf_at: &dyn Fn(
        usize,
        crate::layout::fragment::token::Space,
    ) -> Option<crate::layout::fragment::flow::blocks::leaf::Leaf>,
    remeasure: &mut super::ResumeMeasure<'_>,
) -> Result<(usize, f32, Plan), crate::layout::fragment::token::Error> {
    let target_at = &|col| space_at(col).block_size.expect("definite fragmentainer");
    let limit = space.limit;
    let paged = space.space.is_paginated();
    let not_top = space.space.not_at_start;
    let mut col = 0usize;
    let mut y = 0.0f32;
    let mut prev_mb = 0.0f32;
    let mut first = true;
    let mut placed = not_top > 0;
    let mut shortage = f32::MAX;
    let mut out = Plan::default();
    out.try_reserve(kids.len())
        .map_err(|_| crate::layout::fragment::token::Error::ResourceExhausted)?;
    let mut force_next = false;
    // Группа параллельных строк (`Par`): место начала `(col, y, placed)` и
    // самый дальний конец строки `(col, y)`.
    let mut group: Option<((usize, f32, bool), (usize, f32))> = None;
    // Перенесённый флоат (`Par::float`): его колонки `первая..=последняя`
    // и конец в последней — поток, дошедший туда, встаёт под ним.
    let mut float_hold: Option<(usize, usize, f32)> = None;
    for (kid, k) in kids.iter().enumerate() {
        let (snap, lead) = open_kid(
            &mut col,
            &mut y,
            &mut prev_mb,
            &mut first,
            not_top,
            &mut placed,
            &mut force_next,
            &mut group,
            &mut float_hold,
            k,
        );
        let cur = y + lead;
        if leaf_at(kid, space_at(col)).is_some() {
            let fragments = crate::layout::fragment::flow::fragment_leaf(
                &|space| leaf_at(kid, space).expect("leaf consumer is stable"),
                kid,
                col,
                cur,
                space_at,
            )?;
            if let Some(end) = fragments.last() {
                col = end.col;
                y = end.y + end.h;
                placed = true;
            }
            out.try_reserve(fragments.len())
                .map_err(|_| crate::layout::fragment::token::Error::ResourceExhausted)?;
            out.extend(fragments);
            prev_mb = k.mb;
            first = false;
            continue;
        }
        let from = 0.0f32;
        let copy = 0usize;
        // Параллельный поток (css-break-3 §3 «parallel flows»):
        // содержимое, переполняющее коробку с ЗАДАННОЙ высотой,
        // продолжается в следующем фрагментаинере само по себе, а
        // следующий СОСЕД встаёт сразу под коробкой, в той же колонке
        // (Blink `fragmentation_utils.cc`: «If the block-size is
        // constrained / fixed … we know that we're at the end»). Значит
        // режем по `flow`, а курсор соседа откатываем на конец коробки
        // (ниже, перед `prev_mb = k.mb`). Монолит не трогаем: его
        // переполнение по css-break-3 §4.1 остаётся в своей колонке
        // целиком.
        // У `clone` поток не включается: `from`/`h` его кусков — в других
        // координатах (содержимое / готовый фрагмент), и откат курсора
        // ниже их бы не понял. Неразрезанная `clone`-коробка с потоком
        // рисуется `slice` (`layout::multicol::stack_child`, `frag_geom.len() > 1`).
        let flow = if k.over > k.h + 0.01 && !k.monolith && k.clone_dec.is_none() {
            k.over
        } else {
            k.h
        };
        // Полосы повтора таблицы (`Repeat::leads`); у прочих детей нули, и
        // укладка тождественна прежней.
        let rg = k.repeat;
        fill_pieces(
            target_at,
            limit,
            paged,
            &mut col,
            &mut y,
            not_top,
            &mut placed,
            &mut shortage,
            &mut out,
            &mut float_hold,
            kid,
            k,
            cur,
            from,
            copy,
            flow,
            rg,
            &mut |mut request| {
                request.page = space_at(request.col).page;
                remeasure(request)
            },
        )?;
        // Флоат ушёл целиком в следующую колонку, а в текущей осталось
        // место: следующие коробки продолжают её (`Par::float`).
        if k.par.float && k.par.group == 0 && float_hold.is_none() {
            let mut mine = out.iter().filter(|f| f.kid == kid);
            let f0 = mine.next().copied();
            let fl = mine.next_back().copied().or(f0);
            if let (Some(f0), Some(fl)) = (f0, fl)
                && f0.col > snap.0
                && snap.1 < target_at(snap.0) - 0.01
            {
                float_hold = Some((f0.col, fl.col, fl.y + fl.h));
                (col, y, placed, prev_mb, first) = snap;
                continue;
            }
        }
        // Откат курсора на конец КОРОБКИ: параллельный поток уехал
        // дальше, но сосед по css-break-3 §3 продолжается там, где
        // кончилась коробка. Ищем кусок ЭТОГО ЖЕ ребёнка, внутрь
        // которого попал `k.h`; если поток оборвался раньше (кончились
        // копии), курсор остаётся где был.
        if flow > k.h + 0.01
            && let Some(f) = out
                .iter()
                .rev()
                .take_while(|f| f.kid == kid)
                .find(|f| f.from <= k.h + 0.01 && k.h <= f.from + f.h + 0.01)
        {
            col = f.col;
            y = f.y + (k.h - f.from);
            placed = true;
        }
        prev_mb = k.mb;
        first = false;
        // Конец группы строк: дальше поток идёт с самого дальнего конца
        // строки (контейнер кончается вместе с последним фрагментом своих
        // строк; поля контейнера нулевые).
        if k.par.group != 0
            && k.par.group_end
            && let Some((_, end)) = group.take()
        {
            if (col, y) < end {
                col = end.0;
                y = end.1;
            }
            placed = true;
            prev_mb = 0.0;
            // `break-after` последних элементов строк — разрыв ПОСЛЕ
            // контейнера (там же, :1915-1918); его несёт последний элемент.
            force_next = k.force_after;
        }
    }
    // Курсор мог быть откачен назад параллельным потоком: колонок нужно
    // столько, сколько занял самый дальний КУСОК, а не сколько прошёл
    // курсор. Без потока значение тождественно прежнему: курсор всегда
    // не меньше любого `f.col`.
    let last = out.iter().map(|f| f.col).fold(col, usize::max);
    let pages = last
        .checked_add(1)
        .ok_or(crate::layout::fragment::token::Error::ResourceExhausted)?;
    Ok((pages, shortage, out))
}
