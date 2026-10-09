//! Общий рукав `element()`: блочная коробка без особого тега.
// owner: A

use crate::dom::{Element, Node};
use crate::layout::block::reorder::{orthogonal_vertical_children, resolve_inline_pct};
use crate::layout::block::struts::{margin_px, zero_len};
use crate::layout::block::vertical_flow_margins;
use crate::layout::float::block_like_float;
use crate::layout::fragment::clone::clone_wrapper_item;
use crate::layout::fragment::fragment_size::shape_full;
use crate::layout::fragment::line_shape::{group_inline_runs, inline_content, nested_rows_box, nested_rows_shape, resolved_lengths, transpose_tree};
use crate::layout::fragment::probe::forced_inside;
use crate::layout::fragment::{Shape, ShapeCx, with_lines};
use crate::layout::grid::place_named_areas;
use crate::layout::multicol::column_flow::column_flow;
use crate::layout::multicol::container::{multicol_column_stack, multicol_spanner_segments};
use crate::layout::multicol::gap_rules::gap_rule_spec;
use crate::layout::multicol::spanner::{has_deep_spanner, hoist_spanners, multicol_container, spanner_box};
use crate::layout::positioned::absolute_overflow;
use crate::layout::replaced::limits::auto_clamp_limit;
use crate::layout::table::anon::has_box_style_probe;
use crate::layout::writing_mode::orthogonal_children::orthogonal_children;
use crate::layout::writing_mode::vertical_hug;
use crate::paint::stacking::fixed_cb_layer_box;
use crate::render::*;
use crate::style::cascade::inherit::inherit;
use crate::style::computed::{Computed, Display};
use crate::style::values::value::Len;
use crate::text::text_box::text_box_trim_px;
use gpui::{AnyElement, IntoElement, ParentElement, Styled, px};

pub(crate) fn generic_box(
    e: &Element,
    merged: Computed,
    inherited: &Computed,
    opts: &RenderOpts,
    outer_row: Option<(f32, f32)>,
) -> AnyElement {
    let mut d = styled_div_with(e, &merged);
    let overflow_plan = absolute_overflow::Plan::new(&merged, inherited);
    if let Some(plan) = &overflow_plan {
        plan.prepare(d.style());
    }
    // ЗАМЕРЕНО И ЗАКРЕПЛЕНО: минимума высоты в видимую область на
    // коробке корня БОЛЬШЕ НЕТ. §10.6.3 — высота корня `auto`, ростом
    // с окно обязан быть начальный содержащий блок, а не коробка:
    // рамка и фон корня уходили полосами до низа окна
    // (`background-root-011`, `normal-flow/root-box-001`). Абсолютные
    // потомки всплывают в слой ICB (`icb_open`/`icb_close`), и
    // содержащим блоком им служит корневой `div` стенда.
    //
    // Замер по семьям backgrounds/*, *root*, positioning/*,
    // containing-block*, normal-flow/*, *margin*: приобретено 11,
    // потеряна одна (`background-root-024`: слой донора-тела лежит в
    // детях корня и отсчитывался от его padding-box).
    // Многоколоночный поток. Своей многоколоночной раскладки нет, но
    // сетка даёт то же расположение: число рядов считаем по числу
    // детей, а заполнение идёт по колонкам — тогда порядок совпадает
    // с браузерным (сверху вниз, затем в следующую колонку).
    // Число колонок бывает задано и КОСВЕННО — их шириной: сколько
    // целых колонок этой ширины влезает в коробку, столько их и будет
    // (css-multicol-1 §7.3). Ширина коробки нужна заданная: без неё
    // считать не от чего, и остаётся прежняя дорожечная раскладка.
    // Умолчание `column-gap: normal` — один кегль (css-align §8.3).
    // Вычисленные значения, а не заданные: `resolve_em` живёт в
    // `inline::inherit`, поэтому точки лежат в `merged`
    // (`render.rs:13235`), а в `e.style` остаётся `Len::Em`. Кегль
    // для `column-gap: normal` — СОБСТВЕННЫЙ кегль элемента
    // (css-align §8.3; Blink `length_utils.cc:1356`
    // `style.GetFontDescription().ComputedPixelSize()`), и он тоже
    // разрешён только в `merged`.
    let used_gap = match merged.column_gap {
        Some(Len::Px(v)) => v,
        _ => match merged.font_size {
            Some(Len::Px(size)) => size,
            _ => opts.base_size(),
        },
    };
    // `column-*` — только у блочных контейнеров (css-multicol-1 §2):
    // сетка ими не режется (`grid-multicol-001`).
    let multicol = multicol_container(&e.style);
    // css-multicol-1 §3.4 шаги (05)-(07): N считается по ВЫЧИСЛЕННЫМ
    // 'column-width', 'column-gap' и используемой ширине коробки.
    // Пока брались заданные значения, `column-width: 6em` не
    // проходил гейт `Len::Px`, `count_from_width` был `None`,
    // `width_driven` — ложью, и многоколоночник не включался вовсе:
    // `multicol-width-001` рисовался одним абзацем в 30 знаков
    // вместо пяти колонок по шесть (снимок обеих сторон в
    // `target/scout-mctextflow-2026-09.md` §3.4). Blink
    // абсолютизирует 'column-width' в `float` ещё на вычисленном
    // значении (`css_properties.json5:7513`
    // `ConvertComputedLength<float>`) и в `length_utils.cc:1311`
    // `ResolveUsedColumnCount` читает уже точки.
    let column_width = merged.column_width.filter(|_| multicol);
    let column_count = merged.column_count.filter(|_| multicol);
    // Ось прогрессии колонок — СТРОЧНАЯ ось многоколоночника
    // (css-multicol-1 §2, `Overview.bs:375-379`: «The column boxes are
    // ordered in the inline base direction»). В вертикальном письме она
    // вертикальна, блочная — горизонтальна, у `vertical-rl` от ПРАВОГО
    // края (css-writing-modes-4 §3.1). `inline-size`/`block-size`
    // разложены в физические `height`/`width` ещё в каскаде, поэтому
    // строчный размер коробки здесь — `height`, блочный — `width`.
    // `direction: rtl` в вертикали (колонки снизу вверх) — прежним путём.
    let col_rl = merged.vertical_rl == Some(true);
    let col_axis = if merged.vertical == Some(true) && merged.rtl != Some(true) {
        if col_rl {
            crate::layout::fragment::types::StackAxis::VerticalRl
        } else {
            crate::layout::fragment::types::StackAxis::VerticalLr
        }
    } else {
        crate::layout::fragment::types::StackAxis::Horizontal
    };
    let col_vert = col_axis.is_vertical();
    let col_inline_size = if col_vert { merged.height } else { merged.width };
    let count_from_width = match (column_width, col_inline_size) {
        (Some(Len::Px(w)), Some(Len::Px(box_w))) if w > 0.0 => {
            Some((((box_w + used_gap) / (w + used_gap)).floor().max(1.0)) as u16)
        }
        _ => None,
    };
    // Used column-count (css-multicol §3.4, как ResolveUsedColumnCount
    // в blink): заданы оба — МЕНЬШЕЕ из числа и «сколько влезает»;
    // только ширина — сколько влезает.
    let used_count = match (column_count, count_from_width) {
        (Some(c), Some(fw)) => Some(c.min(fw)),
        (Some(c), None) => Some(c),
        (None, fw) => fw,
    };
    let width_driven = column_count.is_none()
        && count_from_width.is_none()
        && matches!(column_width, Some(Len::Px(w)) if w > 0.0);
    // Заданный `column-height` без числа колонок — одна колонка, но с
    // рядами (`column-height-012`: `column-height:40px` и 80px
    // содержимого — два ряда по 40).
    let height_driven =
        e.style.column_height.is_some() && used_count.is_none_or(|n| n <= 1);
    // `column-count: 1` — тоже многоколоночник (css-multicol-1 §2; Blink
    // `ComputedStyle::SpecifiesColumns`: «!HasAutoColumnCount()»), и
    // спаннер в нём режет содержимое на ряды: ряд до спаннера — свой
    // контекст форматирования и держит свои флоаты
    // (`multicol-span-float-002`: `Pink` вставал между флоатами первой
    // строки), рамка предка режется по фрагментам
    // (`multicol-span-all-children-height-008`). Только при спаннере:
    // без него одна колонка по-прежнему рисуется обычным блоком
    // (переполнения одной колонки вбок здесь нет —
    // `scout-multicol-2026-09.md` §10). Элемент списка — мимо:
    // сегментный путь маркер не рисует
    // (`multicol-span-all-list-item-001/002`).
    let lone_span = used_count == Some(1)
        && !height_driven
        && e.style.column_wrap.is_none()
        && e.list_item.is_none()
        && has_deep_spanner(e);
    if let Some(cols) = used_count
        .filter(|n| *n > 1)
        .or(width_driven.then_some(0))
        .or(height_driven.then_some(1))
        .or(lone_span.then_some(1))
    {
        // Сплошной текст режется на колонки по строкам, а не по детям:
        // один длинный абзац иначе оставался в первой колонке целиком.
        // `columns: auto <w>` без ширины коробки решается в замере —
        // туда уходит и число, и ширина колонки (§3.4).
        let col_w_px = match column_width {
            Some(Len::Px(w)) if w > 0.0 => Some(w),
            _ => None,
        };
        let want = (cols > 0).then_some(cols as usize);
        // Ряды колонок (css-multicol-2 §ch, §cwr): высота ряда —
        // `column-height`, а при `column-wrap: wrap` без него —
        // высота коробки (Blink `RowHeight()`:
        // `remaining_content_block_size_`, issue 11754 вариант 2);
        // `column-wrap: auto` = `wrap` при заданном
        // `column-height`. `row-gap: normal` в колонках — 1em (§rg).
        let col_h = match e.style.column_height {
            Some(Len::Px(h)) if h >= 0.0 => Some(h),
            _ => None,
        };
        // Блочный размер коробки: в вертикальном письме — ширина.
        let box_h = match if col_vert { e.style.width } else { e.style.height } {
            Some(Len::Px(h)) if h > 0.0 => Some(h),
            _ => None,
        };
        let wrap = e.style.column_wrap.unwrap_or(col_h.is_some());
        let em = match e.style.font_size {
            Some(Len::Px(size)) => size,
            _ => opts.base_size(),
        };
        let row_gap = match e.style.gap.and_then(|g| g.0) {
            Some(Len::Px(v)) => v.max(0.0),
            Some(Len::Em(k)) => k * em,
            _ => em,
        };
        let rows = (col_h.is_some() || wrap).then_some(crate::layout::fragment::types::Rows {
            h: col_h.or(if wrap { box_h } else { None }),
            gap: row_gap,
            wrap,
            cap: false,
        });
        // Потолок баланса (`Rows::cap`): СОБСТВЕННАЯ высота коробки в
        // точках. ★ Прежний MC-BALANCE-CAP (float.rs, v164, +3/−9)
        // спускал внешний потолок во ВЛОЖЕННЫЕ многоколоночники без
        // своей высоты — все девять потерь такие; здесь потолок не
        // передаётся вниз (`in_stack`: внутри копии другой стопки —
        // нет), не трогает `column-fill: auto` (там `fixed`) и ряды
        // (`column-height`). `grid-container-fragmentation-004`: 350 в
        // 4 колонках по 100 — баланс уходил в 125, в четвёртой
        // колонке красное 50..100.
        // Потолок баланса — ИСПОЛЬЗУЕМАЯ высота коробки, а не голая `height`:
        // Blink `ConstrainColumnBlockSize` (`column_layout_algorithm.cc:
        // 1774-1793`) берёт `max = min(max-height, height)`, затем
        // `max = max(max, min-height)` («A specified min-block-size may
        // increase the maximum length»; CSS 2.1 §10.7). `multicol-fill-
        // balance-005`: `height:20px; max-height:40px; min-height:100px` —
        // коробка 100, баланс 200/2 = 100 ровно в неё; с потолком 20 колонки
        // выходили по 20, и красный фон 20..100 был виден. `min-height` не в
        // точках (доля, `em`) — потолка нет, как до P5: ниже используемой
        // высоты резать нельзя, а её здесь не знаем.
        let (max_block, min_block) = if col_vert {
            (e.style.max_width, e.style.min_width)
        } else {
            (e.style.max_height, e.style.min_height)
        };
        let cap_h = box_h.and_then(|h| {
            let h = match max_block {
                Some(Len::Px(m)) if m >= 0.0 => h.min(m),
                _ => h,
            };
            match min_block {
                None | Some(Len::Auto) => Some(h),
                Some(Len::Px(m)) => Some(h.max(m)),
                _ => None,
            }
        });
        // Вложенный многоколоночник во внешней колонке (`flow::OUTER_ROW`):
        // ряды высотой во внешний фрагментаинер без зазора — граница
        // ряда совпадает с границей внешней колонки (css-break-4 §2.1;
        // Blink `ConstrainColumnBlockSize`, `LayoutLine` :1009-1026
        // «wrap … if we're participating in an outer fragmentation
        // context»). Последний ряд балансируется (css-multicol-1 §7.1
        // «only the last fragment is balanced»).
        let nest_phase = outer_row.map_or(0.0, |r| r.1);
        let nest_rows = outer_row.map(|r| r.0).filter(|_| {
            col_h.is_none() && e.style.column_wrap.is_none() && !col_vert
        });
        let rows = match nest_rows {
            Some(hh) => Some(crate::layout::fragment::types::Rows {
                h: Some(hh),
                gap: 0.0,
                wrap: true,
                cap: false,
            }),
            None => rows,
        };
        let rows = rows.or_else(|| {
            (cap_h.is_some()
                && e.style.column_fill_auto != Some(true)
                && !crate::layout::fragment::types::in_stack())
            .then_some(crate::layout::fragment::types::Rows {
                h: cap_h,
                gap: row_gap,
                wrap: false,
                cap: true,
            })
        });
        // Спаннер среди инлайнового потока: режем детей на сегменты,
        // каждый сегмент — свой поток колонок, спаннер — блок между
        // ними (css-multicol §6). С рядами (`column-wrap: wrap` +
        // `column-height`) сегменты не годятся: у каждого свои ряды от
        // нуля, а Blink ведёт ОДИН курсор по коробке
        // (`column_layout_algorithm.cc` `intrinsic_block_size_`,
        // `LayoutSpanner`: спаннер, не влезший в остаток ряда, — со
        // следующего ряда; `column-height-006/013/017…020`). Такой
        // многоколоночник идёт единой стопкой, спаннер — её ребёнком
        // (`StackChild::span`). Внутри копии другой стопки — по-прежнему
        // сегментами: перенос ряда во внешнюю колонку не написан
        // (`column-height-029`, scout-columnwrap-2026-09b.md §2.3).
        // Спаннер бывает НЕ прямым ребёнком: css-multicol-1
        // §column-span (`Overview.bs:1497-1499`) — «A spanning element
        // may be lower than the first level of descendants as long as
        // they are part of the same formatting context, and there is
        // nothing between the spanning element and multicol container
        // that establishes a containing block for fixed position
        // descendants». Спаннер выносится ИЗ ПОТОКА и режет
        // многоколоночник на «до», «спаннер во всю ширину» и «после»,
        // а его предки внутри многоколоночника разрезаются вместе с
        // ним. Поднимаем таких потомков к прямым детям ОДИН раз, до
        // всех решений ниже: дальше и сегментный путь, и единая
        // стопка, и текстовый `column_flow` видят спаннер прямым
        // ребёнком. Blink ведёт для этого путь `ColumnSpannerPath`
        // (`column_spanner_path.h`), у нас пути нет — предки режутся
        // прямо в дереве (`hoist_spanners`).
        // Сегментный путь ниже: метка родителя предыдущего спаннера,
        // если между ними не легло ни одного ряда колонок, и выложен
        // ли уже хоть один кусок (для сторожей полей).
        let span_prev: Option<String> = None;
        let seg_open = false;
        let hoisted;
        let e = match hoist_spanners(&e.children) {
            Some(kids) => {
                let mut c = e.clone();
                c.children = kids;
                hoisted = c;
                &hoisted
            }
            None => e,
        };
        let is_span = |n: &Node| matches!(n, Node::Element(c) if spanner_box(c));
        let unified = rows.is_some_and(|r| r.wrap && r.h.is_some()) && !crate::layout::fragment::types::in_stack();
        // Хвостовой ряд колонок при `column-fill: auto` НЕ стоит перед
        // спаннером, и §column-fill («content in a multi-column line that
        // does not immediately precede a spanner») велит заполнять его
        // подряд до высоты коробки. Сегмент ниже теряет высоту
        // (`sub.style.height = None`) и уходил в балансировку:
        // `no-balancing-after-column-span` — 200×50 двумя колонками
        // вместо 100×100 одной. Высота хвоста — остаток коробки после
        // спаннера (Blink `ConstrainColumnBlockSize`:
        // `max -= CurrentContentBlockOffset(line_offset)`). Только когда
        // до хвоста стоит ОДИН измеримый спаннер и больше ничего:
        // высоту сбалансированных рядов до спаннера здесь не знаем.
        let rest_h: Option<f32> = match (e.style.column_fill_auto, box_h, rows) {
            (Some(true), Some(total), None) => e
                .children
                .iter()
                .rposition(&is_span)
                .and_then(|j| {
                    let px = |l: &Option<Len>| match l {
                        None => Some(0.0),
                        Some(Len::Px(v)) => Some(*v),
                        _ => None,
                    };
                    let mut used = 0.0f32;
                    let mut spans = 0usize;
                    for n in &e.children[..=j] {
                        if is_blank(n) {
                            continue;
                        }
                        let Node::Element(sp) = n else {
                            return None;
                        };
                        if !is_span(n) {
                            return None;
                        }
                        spans += 1;
                        let st = inherit(&merged, &sp.style);
                        used += shape_full(sp, 4, ShapeCx::COLUMNS)?.0
                            + px(&st.margin.top)?
                            + px(&st.margin.bottom)?;
                    }
                    (spans == 1).then(|| (total - used).max(0.0))
                }),
            _ => None,
        };
        if e.children.iter().any(&is_span) && !unified {
            return multicol_spanner_segments(d, e, merged, opts, col_w_px, want, lone_span, rest_h, span_prev, seg_open, is_span);
        }
        // Текстовый путь `text-box-trim` не знает (ни у краёв колонок, ни у
        // первой/последней строки) — такой многоколоночник идёт стопкой
        // со строками (`line_run_shape`; `text-box-trim-multicol-009/010`).
        let trim_host = merged.text_box_trim_start || merged.text_box_trim_end;
        if let Some(el) = column_flow(e, &merged, opts, want, col_w_px)
            .filter(|_| nest_rows.is_none() && !trim_host)
        {
            // Коробка элемента остаётся своей: отступы и фон
            // принадлежат ей, поток живёт внутри.
            return d.child(el).into_any_element();
        }
        if cols == 0 {
            // Число колонок при `columns: auto <w>` решается только в
            // замере текстового потока; блочный фоллбек — дорожками.
            if let Some(w) = col_w_px {
                d = d.grid().grid_cols_min(px(w));
            }
        } else {
            // Блочные дети с ИЗВЕСТНЫМИ высотами — честная укладка по
            // колонкам с балансом и монолитами (css-break, фаза 1;
            // план target/scout-multicol.md / scout-fragmentation.md).
            // Коробка ребёнка и его вертикальные поля отдельно:
            // поля схлопываются между соседями и на границах колонок.
            // Флоаты в поддереве ломают известность высоты.
            // Прямые абсолюты многоколоночника — не в стопку: их
            // содержащий блок — весь контейнер, рисуются его детьми
            // рядом со стопкой (`out-of-flow-in-multicolumn-094…097`
            // при нулевой записи в стопке уходили в колонку).
            // Статическая позиция (CSS 2.1 §10.6.4: «where the box
            // would have been if position were static»; §10.3.7 — то
            // же по строчной оси) — правило ПОЗИЦИОНИРОВАННОЙ
            // коробки. У плавающей своё место по §9.5, и щуп ей не
            // положен: нулевая запись в стопке меняет `kids.len()`, с
            // ним `balance_last` и весь план `fill_avoiding`
            // (`multicol-fill-balance-038` — монолитный флоат с
            // полями 40/70 при `margin-bottom: -30px` у соседа:
            // 0.32 -> «красное видно», замерено на v206). Blink
            // перебирает в `LayoutFragmentainerDescendants` только
            // `oof_positioned_candidates`; флоат идёт обычной
            // укладкой (`PositionFloat`).
            let positioned = |s: &Computed| {
                matches!(
                    s.position,
                    Some(crate::style::computed::Position::Absolute)
                        | Some(crate::style::computed::Position::Fixed)
                )
            };
            // Строчный размер колонки для меры строк (`with_lines`):
            // css-multicol-1 §3.4 (11) «W := max(0, (U + column-gap)/N −
            // column-gap)» при U в точках; иначе строки не меряются.
            // Ширина `auto` блока в потоке — ширина содержимого родителя
            // (CSS 2.1 §10.3.3), когда та в точках и у коробки нет боковых
            // полей, рамок и отбивок (`text-box-trim-multicol-011-ref`:
            // многоколоночник без ширины в `.container` 640).
            let line_inline_size = col_inline_size.or_else(|| {
                let zero = |l: &Option<Len>| matches!(l, None | Some(Len::Px(0.0)));
                let b = e.style.borders();
                (!col_vert
                    && matches!(e.style.width, None | Some(Len::Auto))
                    && matches!(e.style.display, None | Some(Display::Block))
                    && e.style.float.unwrap_or(0) == 0
                    && !out_of_flow(&e.style)
                    && [&e.style.margin.left, &e.style.margin.right, &e.style.padding.left, &e.style.padding.right, &b.left, &b.right]
                        .into_iter()
                        .all(zero)
                    // Только простой поток: строки и листья с высотой в точках.
                    // Блок с переполнением своей высоты (`css-break/block-max-
                    // height-001-ref`: 160 с ребёнком 200) стопка рисует иначе,
                    // чем прежний путь рисует тест с `max-height` (0.00 -> 11).
                    && e.children.iter().all(|n| match n {
                        Node::Text(_) => true,
                        Node::Element(k) => {
                            k.inline
                                || inline_content(k)
                                || (k.children.iter().all(is_blank)
                                    && matches!(k.style.height, Some(Len::Px(_))))
                        }
                    }))
                .then_some(inherited.width)
                .flatten()
                .filter(|w| matches!(w, Len::Px(_)))
            });
            let line_col_w = match line_inline_size {
                Some(Len::Px(u)) if merged.border_box != Some(true) && cols > 0 => {
                    Some(((u + used_gap) / cols as f32 - used_gap).max(0.0))
                }
                _ => None,
            };
            // Строчные прогоны среди блоков — анонимными блоками (CSS 2.1
            // §9.2.1.1), только когда строки можно измерить.
            let grouped_e = line_col_w.and_then(|_| group_inline_runs(e));
            let ge: &Element = grouped_e.as_ref().unwrap_or(e);
            // Плавающий прямой ребёнок во всю ширину колонки рядом с
            // собой ничего не терпит: строки и блоки встают под ним,
            // как под блоком, — в стопку колонок он идёт БЛОКОМ и
            // рвётся по колонкам вместе с потоком (css-break-3 §4:
            // флоат — фрагментируемая коробка; `css-break/float-001`:
            // флоат 200px в колонках по 100 — прежний путь рисовал его
            // соседом стопки одним куском).
            let zero_m = |l: &Option<Len>| match l {
                None => true,
                Some(Len::Px(v)) => v.abs() < 0.01,
                _ => false,
            };
            let full_float = |c: &Element| {
                c.style.float.is_some_and(|f| f != 0)
                    && matches!(c.style.width, Some(Len::Pct(k)) if (k - 1.0).abs() < 1e-4)
                    && zero_m(&c.style.margin.left)
                    && zero_m(&c.style.margin.right)
                    // Поля флоата не схлопываются и у края колонки не
                    // усекаются (CSS 2.1 §8.3.1), а у блока стопки —
                    // да: флоат с вертикальными полями — прежним путём
                    // (`multicol-fill-balance-037`: `margin: 40px 0`).
                    && zero_m(&c.style.margin.top)
                    && zero_m(&c.style.margin.bottom)
                    && c.style.shape_outside.is_none()
                    && !positioned(&c.style)
            };
            let floats_blocked = ge
                .children
                .iter()
                .any(|n| matches!(n, Node::Element(c) if full_float(c)))
            .then(|| {
                let mut g = ge.clone();
                for n in g.children.iter_mut() {
                    if let Node::Element(c) = n
                        && full_float(c)
                    {
                        c.style.float = None;
                        c.style.clear = None;
                        c.attrs.push(("kamin-float-block".into(), "1".into()));
                    }
                }
                g
            });
            let ge: &Element = floats_blocked.as_ref().unwrap_or(ge);
            // Обёртка flex/сетки с ЕДИНСТВЕННЫМ элементом-`clone`
            // (`clone_wrapper_item`): по блочной оси такая обёртка
            // раскладывается ровно как блок с этим ребёнком, и фрагменты
            // клонированного украшения строятся у самого элемента.
            let unwrapped = ge
                .children
                .iter()
                .any(|n| matches!(n, Node::Element(c) if clone_wrapper_item(c).is_some()))
                .then(|| {
                    let mut g = ge.clone();
                    for n in g.children.iter_mut() {
                        if let Node::Element(c) = n
                            && let Some(item) = clone_wrapper_item(c)
                        {
                            *c = item;
                        }
                    }
                    g
                });
            let ge: &Element = unwrapped.as_ref().unwrap_or(ge);
            // Плавающие прямые дети — как прежде: не в стопку,
            // рисуются её соседями.
            let direct_oof: Vec<Element> = ge
                .children
                .iter()
                .filter_map(|n| match n {
                    Node::Element(c)
                        if out_of_flow(&c.style) && !positioned(&c.style) =>
                    {
                        Some(c.clone())
                    }
                    _ => None,
                })
                .collect();
            // Позиционированные прямые дети несут МЕСТО В ПОТОКЕ —
            // номер среди детей, ушедших в стопку: точка статической
            // позиции лежит В КОЛОНКЕ, а не под стопкой, и взять её
            // больше неоткуда — раскладка под нами про колонки не
            // знает. Счёт идёт по тем же детям, что отбирает
            // `stackable` ниже: непустые и не внепоточные.
            let oof_static: Vec<(usize, Element)> = {
                let mut at = 0usize;
                let mut out: Vec<(usize, Element)> = Vec::new();
                for n in ge.children.iter().filter(|n| !is_blank(n)) {
                    let Node::Element(c) = n else {
                        at += 1;
                        continue;
                    };
                    if positioned(&c.style) {
                        out.push((at, c.clone()));
                    } else if !out_of_flow(&c.style) {
                        at += 1;
                    }
                }
                out
            };
            // Высота внешнего фрагментаинера для вложенного рядами
            // (`nested_rows_shape`): `column-fill: auto` и блочный размер
            // в точках, без своих рядов.
            let outer_frag = (e.style.column_fill_auto == Some(true)
                && rows.is_none()
                && !col_vert)
                .then(|| col_h.or(box_h))
                .flatten();
            let first_flow = ge
                .children
                .iter()
                .find(|n| !is_blank(n) && !matches!(n, Node::Element(c) if out_of_flow(&c.style)))
                .map(|n| n as *const Node);
            // Чью меру дала `nested_rows_shape` — только их копии рядами.
            let nested_auto: std::cell::RefCell<Vec<u64>> = Default::default();
            // Вложенный многоколоночник `height: auto` под БАЛАНСОМ внешнего
            // (`nested_whole`): одна сбалансированная строка колонок целиком.
            // css-multicol-1 §2: вложенный многоколоночник — сам мультиколонный
            // контейнер, его колонки и линейки рисуются внутри внешней колонки
            // (Blink `ColumnLayoutAlgorithm` для внутреннего — та же раскладка).
            // Копия стопки шла узкой веткой `styled_div_with` и рисовала его
            // плоско — без колонок и линеек (`multicol-rule-color-inherit-001`).
            let nested_whole: std::cell::RefCell<Vec<u64>> = Default::default();
            let whole_ok = e.style.column_fill_auto != Some(true)
                && rows.is_none_or(|r| r.cap)
                && nest_rows.is_none()
                && !col_vert;
            // Дети, чью высоту меряет раскладка копии (`StackChild::measure`).
            let measured_kids: std::cell::RefCell<Vec<(u64, f32)>> = Default::default();
            let measure_ok = e.style.column_fill_auto == Some(true)
                && rows.is_none()
                && nest_rows.is_none()
                && !col_vert
                && (col_h.is_some() || matches!(e.style.height, Some(Len::Px(_))));
            let last_flow = ge
                .children
                .iter()
                .rev()
                .find(|n| !is_blank(n) && !matches!(n, Node::Element(c) if out_of_flow(&c.style)))
                .map(|n| n as *const Node);
            let stackable: Option<Vec<(Element, Shape)>> = with_lines(&merged, line_col_w, opts, || ge
                .children
                .iter()
                .filter(|n| !is_blank(n))
                .filter(|n| !matches!(n, Node::Element(c) if out_of_flow(&c.style)))
                .map(|n| match n {
                    // Вложенный многоколоночник `height: auto` с верха
                    // внешней колонки — рядами (`nested_rows_shape`).
                    Node::Element(c)
                        if first_flow == Some(n as *const Node)
                            && matches!(c.style.height, None | Some(Len::Auto))
                            && zero_len(c.style.margin.top)
                            && outer_frag.is_some()
                            && line_col_w.is_some()
                            && nested_rows_box(c) =>
                    {
                        let c = &resolved_lengths(c, &merged);
                        let (Some(hh), Some(cw)) = (outer_frag, line_col_w) else {
                            return None;
                        };
                        match nested_rows_shape(c, &merged, hh, cw, opts) {
                            Some(h) => {
                                nested_auto.borrow_mut().push(c.node_id);
                                Some(((*c).clone(), h))
                            }
                            None => shape_full(c, 4, ShapeCx::COLUMNS).map(|h| ((*c).clone(), h)),
                        }
                    }
                    Node::Element(c)
                        if whole_ok
                            && matches!(c.style.height, None | Some(Len::Auto))
                            && line_col_w.is_some()
                            && nested_rows_box(c) =>
                    {
                        // Высоту даёт раскладка самой копии (`StackChild::measure`):
                        // внутренний многоколоночник балансирует себя сам, и мера
                        // `shape_full` его колонок не видит (сумма детей).
                        let c = resolved_lengths(c, &merged);
                        let w = line_col_w?;
                        let m = |l: &Option<Len>| match l {
                            Some(Len::Px(v)) => Some(*v),
                            None | Some(Len::Auto) => Some(0.0),
                            _ => None,
                        };
                        let (mt, mb) = (m(&c.style.margin.top)?, m(&c.style.margin.bottom)?);
                        nested_whole.borrow_mut().push(c.node_id);
                        measured_kids.borrow_mut().push((c.node_id, w));
                        Some((c, (0.0, mt, mb, Vec::new(), Vec::new(), Vec::new())))
                    }
                    // `position: relative` укладке не мешает — сдвиг
                    // накладывается на месте (корень A1).
                    Node::Element(c)
                        if !c.inline
                            && (c.style.position.is_none()
                                || c.style.position
                                    == Some(crate::style::computed::Position::Relative))
                            && (c.style.float.unwrap_or(0) == 0
                                || block_like_float(&c.style)) =>
                    {
                        // В вертикальном письме мера — по БЛОЧНОЙ оси
                        // (css-multicol-1 §2: «The column height is the
                        // length of the column box in the block
                        // direction»): поддерево меряется ПОВЁРНУТЫМ
                        // клоном (`transpose_tree`), рисуется исходным.
                        // Длины коробки — в точках (`resolved_lengths`):
                        // и мере, и копиям, и распоркам роста — одно дерево.
                        let mut c = resolved_lengths(c, &merged);
                        // `text-box-trim` многоколоночника режет его ПЕРВУЮ и
                        // ПОСЛЕДНЮЮ отформатированную строку (css-inline-3
                        // §4.2) — у строчного блока-ребёнка с края потока они
                        // его же. Метка, а не флаг стиля: срез на разрывах
                        // решает ближайшая коробка со своим флагом
                        // (`brk_trim`), и флаг ребёнка отнял бы его у хоста
                        // (`text-box-trim-multicol-005`).
                        if inline_content(&c) {
                            if merged.text_box_trim_start && first_flow == Some(n as *const Node) {
                                c.attrs.push(("kamin-host-trim-start".into(), "1".into()));
                            }
                            if merged.text_box_trim_end && last_flow == Some(n as *const Node) {
                                c.attrs.push(("kamin-host-trim-end".into(), "1".into()));
                            }
                        }
                        let c = &c;
                        if col_vert {
                            let t = transpose_tree(c, col_rl)?;
                            shape_full(&t, 4, ShapeCx::COLUMNS).map(|h| ((*c).clone(), h))
                        } else {
                            shape_full(c, 4, ShapeCx::COLUMNS)
                                .map(|h| ((*c).clone(), h))
                                .or_else(|| {
                                    // Мера `shape_full` не выразила ребёнка (флоаты,
                                    // внепоточные потомки, таблица со сросшимися
                                    // рамками …). Прежде отказ ОДНОГО ребёнка
                                    // отправлял весь многоколоночник в сетку без
                                    // фрагментации — содержимое вовсе не переходило
                                    // в следующую колонку. css-break-3 §4: любая
                                    // блочная коробка фрагментируема; её высоту даёт
                                    // сама раскладка копии в колонку (`StackChild::
                                    // measure`, Blink меряет ребёнка тем же
                                    // алгоритмом, что и кладёт), а разрез — по краю
                                    // колонки (`slice`, без точек класса A).
                                    // Только колонки с заданной высотой
                                    // (`column-fill: auto`): у баланса высота
                                    // коробки сама зависит от меры, а раскладка
                                    // копии флоаты в высоту не берёт (а коробка
                                    // многоколоночника — корень контекста — берёт).
                                    // Принудительного разрыва внутри мера без
                                    // точек тоже не видит.
                                    if !measure_ok || forced_inside(c, 6) {
                                        return None;
                                    }
                                    let w = line_col_w?;
                                    let m = |l: &Option<Len>| match l {
                                        Some(Len::Px(v)) => Some(*v),
                                        None | Some(Len::Auto) => Some(0.0),
                                        _ => None,
                                    };
                                    let (mt, mb) = (m(&c.style.margin.top)?, m(&c.style.margin.bottom)?);
                                    measured_kids.borrow_mut().push((c.node_id, w));
                                    Some(((*c).clone(), (0.0, mt, mb, Vec::new(), Vec::new(), Vec::new())))
                                })
                        }
                    }
                    _ => None,
                })
                .collect());
            if let Some(kids) = stackable.filter(|k| !k.is_empty()) {
                return multicol_column_stack(d, e, inherited, merged, opts, kids, cols, column_width, used_gap, row_gap, col_axis, col_vert, col_rl, col_h, line_col_w, rows, nest_rows, nest_phase, direct_oof, oof_static, nested_auto, nested_whole, measured_kids);
            }
            let count = e.children.iter().filter(|n| !is_blank(n)).count().max(1);
            let rows = count.div_ceil(cols as usize).max(1) as u16;
            let gap = used_gap;
            d = d
                .grid()
                .grid_template_cols(
                    (0..cols).map(|_| gpui::GridTrack::Fraction(1.0)).collect(),
                )
                .grid_template_rows((0..rows).map(|_| gpui::GridTrack::Auto).collect())
                .gap_x(px(gap));
            d.style().grid_auto_flow = Some(gpui::GridAutoFlow::Column);
        }
    } else if let Some(Len::Px(w)) = column_width {
        // Ширина колонки без их числа — это «сколько влезет»: ровно
        // то, что умеет короткая форма дорожек в GPUI.
        d = d.grid().grid_cols_min(px(w));
    // Ось блочного потока не зависит от `display` (css-writing-modes-4
    // §3.1): inline-block, ячейка, list-item с вертикальным письмом
    // раскладывают детей той же горизонтальной осью, что и голый блок
    // (`block-flow-direction-*`, `line-box-direction-*`).
    // ★ ЗАМЕРЕНО И ОТКАЧЕНО: явный `Some(Block)` — он же стоит у
    // блокифицированных (абсолют в сетке), и
    // `grid-positioned-children-writing-modes-001` 0.39 -> 1.32 даже с
    // гейтом «родитель не сетка».
    // `list-item` сюда НЕ пускать (★ ЗАМЕРЕНО: `li` с одним текстом
    // становился рядом — `line-box-direction-vrl-019/vlr-020`
    // 6.58 -> 12.55).
    } else if merged.vertical == Some(true)
        // `display: table-caption` — это `Display::Block` С МЕТКОЙ
        // (`computed.rs:3125`), и голый `Some(Block)` сюда пускать
        // нельзя (замеренный откат выше). Метку же ставит ТОЛЬКО
        // само объявление `display: table-caption`, тег `<caption>`
        // её не несёт, а подпись ВНУТРИ стола сюда не приходит вовсе
        // — её строит `table()`. Письмо применяется к подписи
        // (css-writing-modes-4 §3.1, «Applies to: all elements
        // except table row groups, column groups, rows, columns»),
        // значит ось её блочного потока задаёт письмо, а не `display`.
        // Blink: `table_layout_algorithm.cc:67`
        // `ConstraintSpaceBuilder(space, caption.Style().GetWritingDirection(), true)`.
        && (matches!(
            e.style.display,
            None | Some(Display::InlineBlock) | Some(Display::TableCell)
        ) || e.style.is_caption == Some(true)
            // Объявленный `display: flow-root` — тот же блок со своим
            // контекстом (`computed.rs` ставит ему `Some(Block)` с
            // меткой `flow_root`); блокифицированный абсолют метки не
            // несёт, откат выше его не касается. Без этого
            // `flow-root` в `vertical-rl` раскладывал детей
            // горизонтальным блоком: хост полос мерился по
            // min-content, флоаты-колонки эталона
            // `css-break/background-image-001` вставали поперёк строки
            // и пропадали.
            || (e.style.flow_root == Some(true)
                && e.style.display == Some(Display::Block)))
    {
        // Вертикальное письмо: ось блочного потока — горизонтальная.
        // Дети идут слева направо (`vertical-lr`) или справа налево
        // (`vertical-rl`).
        d = d.flex();
        d = if merged.vertical_rl == Some(true) {
            d.flex_row_reverse()
        } else {
            d.flex_row()
        };
        // `sideways-lr`: строка идёт снизу вверх — начало строчной
        // оси у НИЖНЕГО края (css-writing-modes-4 §block-flow).
        // Прижим коробки к низу — ЗАПЛАТКА того времени, когда абзац
        // вертелся по часовой и его содержимое росло от верха.
        // С поворотом против часовой (`VerticalText::ccw`) строка сама
        // начинается у нижнего края, и второй прижим снова уводит
        // рисунок. Снимать ВМЕСТЕ с патчем поворота и мерить
        // `wm-propagation-body-047`, `abs-pos-border-offset-002` —
        // ровно те две пары, ради которых заплатка ставилась.
        if e.style.width.is_none() {
            d = d.flex_shrink_0();
        }
    } else if e.style.display.is_none() {
        // Блок без явного `display` — блочная раскладка taffy, а не
        // гибкая колонка. Колонка навязывала детям сжатие: ребёнок
        // выше родителя ужимался, тогда как браузер даёт ему вылезти.
        // Схлопывание вертикальных отступов при этом делает сама
        // раскладка — включая протекание через пустой блок.
        d = d.flex().flex_col();
        // rtl-прижим переполняющих блоков — ТОЧЕЧНЫЙ align_self End
        // детям с заданной шириной (в blocks): items_end на контейнере
        // снимал stretch у всех, и абзац в rtl ужимался до текста —
        // text-align внутри пустел (text-align-end-001: bw=124.8
        // вместо 300). ЗАМЕРЕНО: +18 css-text при −2..3 wm и −4 mix
        // (spot-механика abs-pos-border-offset полагалась на
        // items_end — новый след) — нетто +10.
    }
    // Ряд по умолчанию — но не тогда, когда письмо справа налево:
    // там ряд обязан идти в обратную сторону, и общая ветка его
    // разворот отменяла. Письмо — СЛИТОЕ: `direction` наследуется
    // (css-writing-modes-4 §2.1), и ряд под `body { direction: rtl }`
    // без своего `direction` шёл слева направо — поля
    // `margin-inline-start` эталонов вставали не между элементами
    // (`gap-001-rtl-ref`, `gap-003-rtl-ref`).
    if e.style.display == Some(Display::Flex)
        && e.style.flex_dir.is_none()
        && merged.rtl != Some(true)
    {
        // При вертикальном письме умолчание `row` — это ось строки, а
        // она идёт сверху вниз.
        d = if merged.vertical == Some(true) {
            d.flex_col()
        } else {
            d.flex_row()
        };
    }
    // ★ ЗАМЕРЕНО И ОТКАЧЕНО: отдавать субсетке обычной сетки
    // РАЗРЕШЁННЫЙ кусок родительских дорожек — тем же кодом, что и
    // проход лунок (`if item.style.subgrid` в `lanes`), с вычетом
    // своих краёв и правкой зазора. Кусок брался из ЯВНЫХ линий
    // ребёнка (в обычной сетке размещение делает вендор, и другого
    // источника `at`/`span` до раскладки нет). Срез css-grid (1133
    // пары, 646 зелёных): 645, приобретено НОЛЬ, и
    // `row-auto-placed-subgrid-nested-subgrid-inherited-tracks-001`
    // ушла 0.00 → HUNG (вложенная субсетка зацикливает раскладку).
    // Возвращать только вместе с размещением субсетки НА НАШЕЙ
    // стороне: пока `at`/`span` берутся из линий, вложенный случай
    // получает кусок от куска и сходится не всегда.
    // Имена областей разворачиваются здесь: контейнер и его дети
    // видны одновременно только на этом уровне.
    let children = match &e.style.grid_areas {
        Some(areas) => place_named_areas(areas, e.children.clone()),
        None => e.children.clone(),
    };
    // Resolve physical margins before preparing the vertical formatting context.
    let children = if merged.vertical == Some(true) {
        // Поле самого контейнера по ведущей стороне оси потока —
        // для схлопывания с первым ребёнком (§8.3.1). Ведущая
        // сторона: левая у `vertical-lr`/`sideways-*`, правая у
        // `vertical-rl`. Открыта, если там нет ни рамки, ни
        // внутреннего отступа; у корня поля не схлопываются вовсе.
        let reverse = merged.vertical_rl == Some(true);
        let lead_margin = if e.tag == "html" {
            None
        } else {
            let b = e.style.borders();
            let (border, pad, own) = if reverse {
                (b.right, e.style.padding.right, e.style.margin.right)
            } else {
                (b.left, e.style.padding.left, e.style.margin.left)
            };
            // Независимый контекст форматирования (overflow не
            // `visible`, флоат, `display: flow-root`, `contain`) не
            // схлопывает своё поле с детьми (CSS2 §8.3.1, css-writing-
            // modes-4 §7.4): `margin-collapse-vlr-017`/`vrl-016`
            // (`overflow: hidden`, v100: 0.00 → «красное видно»).
            let bfc = !matches!(
                e.style.overflow_x,
                None | Some(crate::style::computed::Overflow::Visible)
            ) || !matches!(
                e.style.overflow_y,
                None | Some(crate::style::computed::Overflow::Visible)
            ) || e.style.float.is_some()
                || e.style.display.is_some()
                || e.style.flow_root == Some(true)
                // css-align-3 §align-block — тот же список, что и в
                // `own_context`: своё поле такая коробка с полем
                // первого ребёнка не схлопывает.
                || e.style.align_content_block
                || e.style.contain_layout == Some(true)
                || e.style.contain_paint == Some(true);
            let sealed = bfc
                || margin_px(border, &e.style).unwrap_or(0.0) > 0.0
                || margin_px(pad, &e.style).unwrap_or(0.0) > 0.0;
            if sealed {
                None
            } else {
                Some(margin_px(own, &e.style).unwrap_or(0.0))
            }
        };
        // Доли полей/отступов — в точки от высоты контейнера ДО
        // схлопывания (см. `resolve_inline_pct`).
        vertical_hug::children(
            orthogonal_children(
                vertical_flow_margins::children(
                    resolve_inline_pct(children, &merged, true),
                    &merged,
                    reverse,
                    lead_margin,
                ),
                &merged,
                opts.viewport.0,
            ),
            &e.style,
            &merged,
        )
    } else {
        orthogonal_vertical_children(resolve_inline_pct(children, &merged, false), &merged)
    };
    let mut kids: Vec<AnyElement> = Vec::new();
    kids.extend(clip_layer(&merged, opts));
    // Бюджет строк обрезки: сторожа контекста живут, пока строится
    // поддерево — пробы детей пишут строки в буфер контейнера.
    // Многоколонник клэмпом не режется (`continue: collapse` там как
    // `auto`, §5.2; `styled_div_with` срез не ставит): без этого гейта
    // бюджет абзаца счётного режима поставил бы «…» (`line-clamp-039`).
    let is_clamp = (e.style.clamp_lines().is_some()
        || (e.style.clamp_auto == Some(true) && auto_clamp_limit(&merged).is_some()))
        && !multicol_container(&e.style);
    let _clamp_guard = is_clamp.then(|| crate::text::clamp::ClampGuard::enter(e.node_id));
    let makes_bfc = matches!(
        merged.overflow_x,
        Some(crate::style::computed::Overflow::Hidden) | Some(crate::style::computed::Overflow::Scroll)
    ) || matches!(
        merged.overflow_y,
        Some(crate::style::computed::Overflow::Hidden) | Some(crate::style::computed::Overflow::Scroll)
    ) || merged.float.is_some()
        || merged.flow_root == Some(true)
        // Независимый контекст форматирования и без overflow/float/
        // flow-root: гибкий контейнер, сетка, таблица — своя
        // раскладка по определению, и строки внутри в бюджет
        // `line-clamp` не входят (css-overflow-4: «skip lines in
        // independent formatting contexts»; `webkit-line-clamp-012/013`).
        || matches!(
            merged.display,
            Some(Display::Flex)
                | Some(Display::InlineFlex)
                | Some(Display::Grid)
                | Some(Display::InlineGrid)
                | Some(Display::GridLanes)
                | Some(Display::Table)
                | Some(Display::InlineTable)
                | Some(Display::TableCell)
        )
        // `<fieldset>` — тоже отдельная раскладка
        // (`webkit-line-clamp-027`).
        || e.tag == "fieldset";
    let _bfc_guard = (!is_clamp && makes_bfc && crate::text::clamp::clamp_context().is_some())
        .then(crate::text::clamp::ClampGuard::enter_bfc);
    // Проба элемента сетки/гибкого контейнера: пишет свои разложенные
    // границы в буфер родителя. Ставится ДО clamp-пробы, чтобы её
    // ранний `return` не съел запись. Абсолютные дети дорожек не
    // занимают (css-grid-1 §9), а пустой анонимный блок — это
    // распорка лент (`spacer()`), не элемент.
    if let Some(key) = crate::paint::gap_rules::gap_context()
        && !matches!(
            e.style.position,
            Some(crate::style::computed::Position::Absolute)
                | Some(crate::style::computed::Position::Fixed)
        )
        && !(e.node_id == 0 && e.children.is_empty())
    {
        // Проба ложится на паддинг-бокс; линейкам нужен рамочный.
        let fs = match merged.font_size {
            Some(Len::Px(v)) => v,
            _ => opts.base_size(),
        };
        let bw = |l: Option<Len>| match l {
            Some(Len::Px(v)) => v,
            Some(Len::Em(k)) => k * fs,
            _ => 0.0,
        };
        let b = e.style.borders();
        kids.push(crate::paint::gap_rules::gap_item_probe(
            crate::paint::gap_rules::gap_items_for(key),
            [bw(b.top), bw(b.right), bw(b.bottom), bw(b.left)],
        ));
    }
    if let Some((key, skip)) = crate::text::clamp::clamp_context() {
        // Строки дают пробы абзацев (paragraph_probed); здесь — только
        // коробка с краской: блок прячется целиком, если срез внутри.
        // Поточная коробка со СВОИМ контекстом форматирования точек
        // среза внутри не имеет (css-overflow-4 §5.3: строки
        // независимых контекстов не считаются, точка — только между
        // блоками): пересечённая потолком, она уходит целиком, как
        // коробка с заданной высотой (`line-clamp-auto-033`:
        // `flow-root` под «Line 4»). Флоат и абсолют — не поточные,
        // строчный атом — внутри строки.
        let monolithic = makes_bfc
            && !merged.float.is_some_and(|f| f != 0)
            && !matches!(
                merged.position,
                Some(crate::style::computed::Position::Absolute)
                    | Some(crate::style::computed::Position::Fixed)
            )
            && !matches!(
                merged.display,
                Some(Display::InlineBlock)
                    | Some(Display::InlineFlex)
                    | Some(Display::InlineGrid)
                    | Some(Display::InlineTable)
            );
        if !is_clamp && (has_box_style_probe(&e.style) || monolithic) {
            // Нижние рамка и паддинг фрагментированной коробки
            // остаются в потоке (css-overflow-4 §5.3): проба несёт
            // их вместе с границами паддинг-бокса.
            let side = |l: Option<Len>| match l {
                Some(Len::Px(v)) => v,
                _ => 0.0,
            };
            let bp_after = side(e.style.borders().bottom) + side(e.style.padding.bottom);
            kids.push(crate::text::clamp::clamp_probe(
                crate::text::clamp::clamp_lines_for(key),
                0.0,
                skip,
                e.style.height.is_some() || e.style.min_height.is_some() || monolithic,
                bp_after,
                // Коробка — не абзац: знак обрыва на неё не садится
                // (он всегда в конце строки, css-overflow-4 §5.3).
                None,
                None,
            ));
        } else if !is_clamp
            && !skip
            && e.children.iter().all(is_blank)
            && !merged.float.is_some_and(|f| f != 0)
            && !matches!(
                merged.position,
                Some(crate::style::computed::Position::Absolute)
                    | Some(crate::style::computed::Position::Fixed)
            )
            && matches!(merged.display, None | Some(Display::Block))
        {
            kids.push(crate::text::clamp::clamp_empty_probe(
                crate::text::clamp::clamp_lines_for(key),
            ));
        }
    }
    // Абсолютный потомок ищет ближайшего позиционированного предка
    // (§10.1), а раскладка под нами знает только непосредственного
    // родителя. Пока строятся дети, открыт слой: коробка, чей родитель
    // содержащим блоком не является, переезжает сюда.
    let cb_layer = crate::text::inline::establishes_cb(&merged);
    if cb_layer {
        crate::layout::positioned::containing_block::cb_open_with(fixed_cb_layer_box(&merged));
    }
    // Линейки промежутков (css-gaps-1). Слой заводится ТОЛЬКО когда
    // задан стиль хотя бы одной линейки: начальное `none` означает,
    // что рисовать нечего, и ни одна старая пара сюда не попадает
    // (см. `gap_rule_spec`).
    let gap_rules = gap_rule_spec(e, &merged, opts);
    let gap_buf = gap_rules
        .is_some()
        .then(|| crate::paint::gap_rules::gap_items_for(e.node_id ^ opts.doc_salt));
    let _gap_guard = gap_buf
        .as_ref()
        .map(|_| crate::paint::gap_rules::GapGuard::enter(e.node_id ^ opts.doc_salt));
    // css-gaps-1 §gap-decorations: «Gap decorations are painted just
    // above the border of the container» — ПОД детьми. Слой идёт до
    // них: буфер проб он всё равно читает в `paint`, а prepaint всего
    // дерева у gpui проходит раньше (эталоны `grid-gap-decorations-042`
    // и `flex-033` кладут линейки `z-index: -1`, `008/023` — элементы
    // `z-index: 2`).
    if let (Some(buf), Some(spec)) = (gap_buf, gap_rules) {
        kids.push(crate::paint::gap_rules::painter::GapRulePainter::new(buf, spec).into_any_element());
    }
    kids.extend(blocks(&children, &merged, opts));
    if cb_layer {
        kids.extend(crate::layout::positioned::containing_block::cb_close());
    }
    if is_clamp {
        // Бюджет среза — высота ПОЛЯ СОДЕРЖИМОГО: при `box-sizing:
        // border-box` из `max-height` вычитаются вертикальные рамки и
        // паддинги (`line-clamp-auto-003`: 138 − 2×(1+4) = 128).
        let bb_y = if merged.border_box == Some(true) {
            let side = |l: Option<Len>| match l {
                Some(Len::Px(v)) => v,
                _ => 0.0,
            };
            let bw = merged.borders();
            side(bw.top)
                + side(bw.bottom)
                + side(merged.padding.top)
                + side(merged.padding.bottom)
        } else {
            0.0
        };
        // Потолок высоты участвует в выборе точки среза только в
        // авто-режиме (`line-clamp: auto` / `4 auto`); счётный
        // `line-clamp: 4` и `-webkit-line-clamp` режут ТОЛЬКО по числу
        // строк, а не влезшее в `max-height` переполняет коробку
        // (`line-clamp-035`, `webkit-line-clamp-with-max-height`).
        let max_h = match auto_clamp_limit(&merged) {
            Some(v) if e.style.clamp_auto == Some(true) => Some((v - bb_y).max(0.0)),
            _ => None,
        };
        // `text-box-trim: trim-end` клампа: последняя строка перед
        // обрывом срезается (css-inline-3 §4.2; Blink триммит строку у
        // точки клампа). Метрика — по стилю контейнера.
        let clamp_trim = if merged.text_box_trim_end {
            text_box_trim_px(&merged, false, opts)
        } else {
            0.0
        };
        kids.push(
            crate::text::clamp::ClampCut::new(
                e.node_id,
                crate::text::clamp::clamp_lines_for(e.node_id),
                e.style.clamp_lines(),
                max_h,
            )
            .trim_end(clamp_trim)
            .into_any_element(),
        );
    }
    // Абсолют с `anchor()`-вставками: раскладка поставила его к краю
    // содержащего блока (нулевая вставка), сдвиг до края якоря
    // считает заместитель на подготовке кадра. Без якорных вставок
    // коробка возвращается как есть.
    let child = d.children(kids).into_any_element();
    let child = match overflow_plan { Some(plan) => plan.wrap(child), None => child };
    crate::anchor::place(child, &merged, inherited)
}
