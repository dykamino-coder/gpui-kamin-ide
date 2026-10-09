//! Многоколоночная ветвь `element()`.
// owner: A

use crate::render::*;

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
    col_axis: crate::flow::StackAxis,
    col_vert: bool,
    col_rl: bool,
    col_h: Option<f32>,
    line_col_w: Option<f32>,
    rows: Option<crate::flow::Rows>,
    nest_rows: Option<f32>,
    nest_phase: f32,
    direct_oof: Vec<Element>,
    oof_static: Vec<(usize, Element)>,
    nested_auto: std::cell::RefCell<Vec<u64>>,
    nested_whole: std::cell::RefCell<Vec<u64>>,
    measured_kids: std::cell::RefCell<Vec<(u64, f32)>>,
) -> AnyElement {
    // Копий у ребёнка — сколько колонок он может занять: без
    // рядов ровно `cols` (как прежде), с рядами — по своей
    // высоте против высоты ряда, с запасом на поля и срезы.
    // Спаннер между колонками не режется — копий ему не
    // надо, и в счёт он не входит (`column-height-019`:
    // спаннер 85px при ряде 5px).
    let copies = match rows {
        Some(r) => {
            let per = r.h.unwrap_or(f32::MAX).max(1.0);
            let span = kids
                .iter()
                .filter(|(c, _)| c.style.column_span != Some(true))
                .map(|(_, s)| (s.0 / per).ceil() as usize)
                .max()
                .unwrap_or(0);
            (span + 2).max(cols as usize).min(48)
        }
        None => cols.max(1) as usize,
    };
    // `column-fill: auto`: колонки заполняются подряд до
    // `column-height`, а без него — до высоты коробки
    // (css-multicol-1 §3.3, как прежде).
    // `column-fill: auto` заполняет колонку до БЛОЧНОГО
    // размера коробки — в вертикальном письме это ширина.
    let fixed = if let Some(hh) = nest_rows {
        (e.style.column_fill_auto == Some(true)).then_some(hh)
    } else if e.style.column_fill_auto == Some(true) {
        col_h.or(match if col_vert { e.style.width } else { e.style.height } {
            Some(Len::Px(h)) => Some(h),
            _ => None,
        })
    } else {
        None
    };
    // Рост от вытолкнутых монолитов — распорки в
    // DOM-клонах до сборки копий (`grow_pushed`).
    // Многострочный колоночный flex — строками, параллельными
    // потоками (`split_flex_lines`). Только при заполнении
    // `column-fill: auto` с заданной высотой и без рядов:
    // баланс считал бы содержимое по сумме записей, а строки
    // идут бок о бок.
    // ★ ЗАМЕРЕНО И ОТКАЧЕНО (01.10): то же при балансе
    // (`rows.is_none()` без `fixed`) с оценкой баланса по
    // самой длинной строке группы (`runs_guess`). css-break/
    // flexbox 319: +0/−2 — `multi-line-row-flex-fragmentation-
    // 037/038` 0.07 → «красное видно»; балансные 033-035, 048
    // не взяты. Только колонки (без рядов) при балансе — 0/0.
    // Балансу нужен свой подбор высоты по строкам, а не оценка.
    // Строки flex и распорки роста (`grow_pushed`) меряют
    // ФИЗИЧЕСКОЕ дерево и знают только вертикальную ось —
    // в вертикальном письме их нет (следующий шаг).
    let (kids, kid_par, kid_parent, kid_starts) = if fixed.is_some() && rows.is_none() && !col_vert {
        let col_w = match merged.width {
            Some(Len::Px(w)) if merged.border_box != Some(true) && cols > 0 => {
                Some((w - used_gap * (cols as f32 - 1.0)) / cols as f32)
            }
            _ => None,
        };
        split_flex_lines(kids, col_w, &merged)
    } else {
        let n = kids.len();
        (kids, vec![crate::flow::Par::default(); n], vec![None; n], (0..=n).collect())
    };
    // `break-inside: avoid` без настоящего монолита — у любого
    // ребёнка колонок (`flow::Par::avoid_only`): с верха колонки
    // коробка выше колонки рвётся, а не переполняет её.
    let mut kid_par = kid_par;
    for (p, (c, _)) in kid_par.iter_mut().zip(kids.iter()) {
        if p.group == 0 {
            p.avoid_only = c.style.break_inside_avoid && avoid_only_monolith(c);
            p.float = c.attr("kamin-float-block").is_some();
            p.clears = c.style.clear.is_some();
        }
    }
    let kids = if col_vert {
        kids
    } else {
        grow_pushed(kids, cols as usize, fixed, rows, copies, &kid_par)
    };
    // `box-decoration-break: clone`: геометрия фрагментов —
    // ДО сборки копий: каждая копия такой коробки строится
    // отдельной коробкой своей высоты (`clone_fragment`).
    // Щуп — как у `grow_pushed`, но с НАСТОЯЩИМ параллельным
    // потоком соседей (та же мера, что у `StackChild` ниже):
    // иначе план соседа с потоком разошёлся бы с укладкой.
    // Без `clone` среди детей не считается вовсе.
    let clone_plan: Vec<Vec<(f32, f32)>> =
        if !col_vert && kids.iter().any(|(c, _)| clone_dec(c).is_some()) {
            let probe: Vec<crate::flow::Kid> = kids
                .iter()
                .enumerate()
                .map(|(pi, (c, s))| {
                    let mut m = c.clone();
                    m.style.margin.top = None;
                    m.style.margin.bottom = None;
                    let (over, cuts, forced, solid) = match shape_full(
                        &m,
                        4,
                        ShapeCx {
                            unclamped: true,
                            ..ShapeCx::COLUMNS
                        },
                    )
                    .filter(|_| {
                        fixed.is_some()
                            && plain_block_tree(&m, 4)
                            && visible_overflow(&m.style)
                    })
                    .filter(|u| u.0 > s.0 + 0.01)
                    {
                        Some(u) => (u.0, u.3, u.4, u.5),
                        None => (s.0, s.3.clone(), s.4.clone(), s.5.clone()),
                    };
                    crate::flow::Kid {
                        h: s.0,
                        mt: s.1,
                        mb: s.2,
                        monolith: solid_box(c),
                        cuts,
                        force_before: edge_break(c, false),
                        force_after: edge_break(c, true),
                        avoid_before: edge_avoid(c, false),
                        avoid_after: edge_avoid(c, true),
                        forced,
                        solid,
                        span: c.style.column_span == Some(true) && !c.inline,
                        over,
                        clone_dec: clone_dec(c),
                        // Тот же предикат, что у `StackChild` ниже:
                        // иначе план соседа разошёлся бы с укладкой.
                        overflow_top: fixed.is_some()
                            && rows.is_none()
                            && !parallel_items_inside(c, 4),
                        repeat: repeat_leads(c, fixed, rows),
                        par: kid_par[pi],
                    }
                })
                .collect();
            crate::flow::ColumnStack::frags_of(
                &probe,
                cols as usize,
                fixed,
                rows,
                copies,
            )
        } else {
            Vec::new()
        };
    let rule = if e.style.column_rule_visible == Some(true) {
        Some((
            match e.style.column_rule_width {
                Some(Len::Px(v)) => v,
                Some(Len::Em(k)) => {
                    k * match e.style.font_size {
                        Some(Len::Px(fs)) => fs,
                        _ => opts.base_size(),
                    }
                }
                _ => 3.0,
            },
            e.style
                .column_rule_color
                .or(merged.color)
                .unwrap_or(crate::value::Color {
                    r: 0.0,
                    g: 0.0,
                    b: 0.0,
                    a: 1.0,
                })
                .to_hsla(),
        ))
    } else {
        None
    };
    // С рядами линейки (`column-rule` со втяжкой/разрывом,
    // `row-rule` — css-multicol-2 §rg/§crc → css-gaps-1)
    // красит `GapRulePainter` по границам колонок и
    // спаннеров из стопки (`ColumnStack::gap_items`);
    // простая полоса `rule` тогда не рисуется. Без рядов —
    // как прежде (`multicol-rule-*` не трогаются).
    let gap_spec = rows
        .filter(|r| r.wrap)
        .and_then(|_| multicol_gap_rule_spec(e, &merged, opts, used_gap, row_gap));
    let gap_items = gap_spec
        .as_ref()
        .map(|_| crate::interact::gap_items_for(e.node_id ^ opts.doc_salt ^ 0x4D43_4F4C));
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
    let balanced_frag: Option<f32> = (fixed.is_none()
        && rows.is_none_or(|r| r.cap)
        && cols > 1
        && kids.len() == 1)
        .then(|| {
            let (c, s) = &kids[0];
            (nested_rows_box(c)
                && matches!(c.style.height, Some(Len::Px(_)))
                && s.3.is_empty()
                && s.1.abs() < 0.01)
                .then(|| {
                    let per = s.0 / cols as f32;
                    rows.and_then(|r| r.h).map_or(per, |cap| per.min(cap))
                })
        })
        .flatten()
        .filter(|h| *h > 1.0);
    let fixed_nest = fixed.or(balanced_frag);
    let nest_at: Vec<Option<f32>> = {
        let mut v = Vec::with_capacity(kids.len());
        let (mut y, mut prev_mb, mut ok) = (0.0f32, 0.0f32, true);
        for (i, (c, s)) in kids.iter().enumerate() {
            let lead = if i == 0 { s.1 } else { prev_mb.max(s.1) };
            let hh = fixed_nest.unwrap_or(0.0);
            v.push((ok && fixed_nest.is_some() && y + lead < hh - 0.01).then_some(y + lead));
            if edge_break(c, false)
                || edge_break(c, true)
                || kid_par.get(i).is_some_and(|p| p.group != 0)
                || y + lead + s.0 > hh + 0.01
            {
                ok = false;
            }
            y += lead + s.0;
            prev_mb = s.2;
        }
        v
    };
    let children: Vec<crate::flow::StackChild> = kids
        .into_iter()
        .enumerate()
        .map(|(ix, (c, (h, mt, mb, cuts, forced, solid)))| {
            multicol_stack_child(
                ix,
                c,
                h,
                mt,
                mb,
                cuts,
                forced,
                solid,
                e,
                &merged,
                opts,
                &clone_plan,
                &kid_parent,
                &kid_par,
                col_vert,
                col_rl,
                line_col_w,
                rows,
                copies,
                fixed,
                fixed_nest,
                balanced_frag,
                &nest_at,
                &nested_auto,
                &nested_whole,
                &measured_kids,
            )
        })
        .collect();
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
    let mut children = children;
    let oof_spots: Vec<crate::interact::SpotCell> =
        oof_static.iter().map(|_| Default::default()).collect();
    for (i, (at, oof)) in oof_static.iter().enumerate().rev() {
        // Заданную ось считает раскладка от содержащего
        // блока, щуп правит только ПУСТУЮ (CSS 2.1
        // §10.3.7) — тот же гейт `fixed_axes`, что у слоёв
        // в `blocks()`.
        oof_spots[i].set(crate::interact::Spot {
            fixed_axes: (
                edge_set(oof.style.inset.left)
                    || edge_set(oof.style.inset.right),
                edge_set(oof.style.inset.top)
                    || edge_set(oof.style.inset.bottom),
            ),
            rtl: merged.rtl == Some(true),
            vertical: merged.vertical == Some(true),
            vertical_rl: merged.vertical_rl == Some(true),
            own_vertical: oof.style.vertical == Some(true),
            ..Default::default()
        });
        let probe = crate::flow::StackChild {
            measure: None,
            el: crate::interact::spot_probe(oof_spots[i].clone(), true),
            frags: Vec::new(),
            monolith: false,
            cuts: Vec::new(),
            force_before: false,
            force_after: false,
            avoid_before: false,
            avoid_after: false,
            forced: Vec::new(),
            solid: Vec::new(),
            h: 0.0,
            mt: 0.0,
            mb: 0.0,
            span: false,
            over: 0.0,
            rel: (0.0, 0.0),
            clone_dec: None,
            overflow_top: false,
            nested_cols: false,
            repeat: None,
            par: crate::flow::Par::default(),
            slack: None,
            laid_w: Default::default(),
            positioned: false,
        };
        // Номер — среди ДЕТЕЙ ДО раскрытия строк flex (`split_flex_lines`).
        let at = kid_starts.get(*at).copied().unwrap_or(children.len()).min(children.len());
        children.insert(at, probe);
    }
    // Стопка тянется по СТРОЧНОЙ оси: в вертикальном письме
    // это высота, значит коробка кладёт её гибким рядом
    // (поперечная ось растягивает высоту), у `vertical-rl` —
    // от ПРАВОГО края (`flex_row_reverse`), там начало
    // блочной оси.
    let d = if col_vert {
        let d = d.flex();
        if col_rl { d.flex_row_reverse() } else { d.flex_row() }
    } else {
        d
    };
    let mut d = d.child(
        crate::flow::ColumnStack::new(
            children,
            cols as usize,
            used_gap,
            fixed,
            rule,
            rows,
            gap_items.clone(),
            intrinsic_inline_size(&e.style, inherited).then(|| {
                crate::flow::Intrinsic(match column_width {
                    Some(Len::Px(w)) if w > 0.0 => Some(w),
                    _ => None,
                })
            }),
        )
        .with_axis(col_axis)
        .with_row_phase(if nest_rows.is_some() { nest_phase } else { 0.0 })
        // Линейки последней линии — до низа содержимого коробки
        // заданной высоты (Blink `PaintColumnRules`), без
        // спаннеров и рядов (`multicol-rule-nested-balancing-001`).
        .with_rule_stretch(
            match merged.height {
                Some(Len::Px(h))
                    if h > 0.0
                        && !col_vert
                        && nest_rows.is_none()
                        && e.style.border_box != Some(true)
                        && !e.children.iter().any(|n| matches!(n, Node::Element(c) if spanner_box(c))) =>
                {
                    Some(h)
                }
                _ => None,
            },
        ),
    );
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
    let cb_h: Option<f32> = (e.style.position.is_some()
        && e.style.position != Some(crate::computed::Position::Static)
        && !col_vert)
        .then(|| {
            let px = |l: Option<Len>| match l {
                None => Some(0.0),
                Some(Len::Px(v)) => Some(v),
                _ => None,
            };
            let b = e.style.borders();
            let pad = px(e.style.padding.top)? + px(e.style.padding.bottom)?;
            let bor = px(b.top)? + px(b.bottom)?;
            match e.style.height {
                Some(Len::Px(h)) if e.style.border_box == Some(true) => Some((h - bor).max(pad)),
                Some(Len::Px(h)) => Some(h.max(0.0) + pad),
                _ => None,
            }
        })
        .flatten();
    for (i, (_, oof)) in oof_static.iter().enumerate() {
        let mut oof = oof.clone();
        if let Some(ch) = cb_h
            && oof.style.position == Some(crate::computed::Position::Absolute)
        {
            for l in [&mut oof.style.height, &mut oof.style.min_height, &mut oof.style.max_height] {
                if let Some(Len::Pct(k)) = *l {
                    *l = Some(Len::Px(k * ch));
                }
            }
        }
        d = d.child(crate::interact::spot_place(
            oof_spots[i].clone(),
            element(&oof, &merged, opts),
        ));
    }
    if let (Some(buf), Some(spec)) = (gap_items, gap_spec) {
        d = d.child(crate::interact::GapRulePainter::new(buf, spec).into_any_element());
    }
    return d.into_any_element();
}

#[allow(clippy::too_many_arguments, clippy::needless_return)]
pub(crate) fn multicol_spanner_segments(
    mut d: gpui::Div,
    e: &Element,
    merged: Computed,
    opts: &RenderOpts,
    col_w_px: Option<f32>,
    want: Option<usize>,
    lone_span: bool,
    rest_h: Option<f32>,
    mut span_prev: Option<String>,
    mut seg_open: bool,
    is_span: impl Fn(&Node) -> bool,
) -> AnyElement {
    // `<fieldset>`: отрисованная легенда стоит ВНЕ колонок —
    // многоколоночность получает анонимная коробка содержимого
    // fieldset (HTML §15.3.13 «The fieldset and legend
    // elements», пересказ; Blink `LayoutFieldset` +
    // `FieldsetContentBox`). Прежде легенда падала в первый ряд и
    // занимала колонку (`multicol-span-all-fieldset-001…003`:
    // эталон — `fieldset > legend + div.inner` с колонками).
    // Отрисованная — первая `legend` в потоке.
    let mut kids = e.children.clone();
    if e.tag == "fieldset" {
        if let Some(i) = kids.iter().position(|n| {
            matches!(n, Node::Element(c) if c.tag == "legend" && !out_of_flow(&c.style))
        }) {
            let legend = kids.remove(i);
            d = d.children(blocks(&[legend], &merged, opts));
        }
    }
    for chunk in kids.split_inclusive(&is_span) {
        let (body, span) = match chunk.split_last() {
            Some((last, head)) if is_span(last) => (head, Some(last)),
            _ => (chunk, None),
        };
        if body.iter().any(|n| !is_blank(n)) {
            // Ряд колонок между спаннерами — новый контекст
            // форматирования: поля спаннеров сквозь него не
            // схлопываются (§column-span: «margins on elements
            // inside a column box will not collapse with the margin
            // of a spanner»). Сам ряд для taffy — лист или
            // не-блок, насквозь его поле не проходит.
            span_prev = None;
            seg_open = true;
            let mut seg = e.clone();
            seg.children = body.to_vec();
            seg.style.column_span = None;
            // Одна колонка со спаннером (`lone_span`) — ряд рисуется
            // обычным блоком: `column_flow` спускается в
            // единственного блочного ребёнка и рисует только его
            // текст, теряя коробку (фон, рамку, высоту фрагмента
            // `container` в `multicol-span-all-children-height-005/
            // 008`). Клон `sub` с одной колонкой спаннеров не несёт,
            // и гейт `lone_span` его снова не пускает.
            let flow = if lone_span {
                None
            } else {
                column_flow(&seg, &merged, opts, want, col_w_px)
            };
            if let Some(el) = flow {
                d = d.child(el);
            } else {
                // Блочный сегмент: рекурсия в общий рендер —
                // он сам выберет укладку колонок; коробка
                // (фон/рамки/поля) остаётся на хосте.
                let mut sub = seg.clone();
                sub.style.background = None;
                // Всё, что многоколоночник рисует и сдвигает КОРОБКОЙ,
                // уже стоит на хосте `d` (`styled_div_with(e, ..)`);
                // клон ряда повторял это на каждом ряду: контур и
                // тень вокруг каждого ряда, двойная прозрачность,
                // фильтр и трансформ, двойной сдвиг `relative`, а у
                // `position: absolute` ряды выпадали из потока и
                // ложились друг на друга (`multicol-span-all-
                // fieldset-002/003`, `-button-002/003`). Обрезку
                // переполнения делает хост: по css-multicol-1
                // §overflow режет коробка многоколоночника, а не ряд.
                // Ряд остаётся содержащим блоком (`relative`), как
                // прежде.
                sub.style.gradient = None;
                sub.style.bg_image = None;
                sub.style.border_image = None;
                sub.style.shadows = Vec::new();
                sub.style.inset_shadows = Vec::new();
                sub.style.outline = None;
                sub.style.opacity = None;
                sub.style.transform = None;
                sub.style.filter = None;
                sub.style.mask_image = None;
                sub.style.backdrop_blur = None;
                if matches!(
                    sub.style.position,
                    Some(crate::computed::Position::Absolute)
                        | Some(crate::computed::Position::Fixed)
                ) {
                    sub.style.position = Some(crate::computed::Position::Relative);
                }
                sub.style.inset = Default::default();
                sub.style.z_index = None;
                sub.style.overflow_x = None;
                sub.style.overflow_y = None;
                sub.style.margin = Default::default();
                sub.style.padding = Default::default();
                sub.style.border_width = Default::default();
                sub.style.width = None;
                sub.style.height = None;
                // Хвост после спаннера при `column-fill: auto` —
                // остаток коробки (`rest_h`): укладка заполняет
                // колонки подряд, а не балансирует. Только
                // измеримым блокам: неизмеримый ряд уходит в
                // запасную сетку, где заданная высота растянула бы
                // дорожки `auto`.
                if span.is_none()
                    && body.iter().all(|n| {
                        is_blank(n)
                            || matches!(n, Node::Element(k)
                                if !k.inline
                                    && shape_full(k, 4, ShapeCx::COLUMNS).is_some())
                    })
                {
                    if let Some(rest) = rest_h {
                        sub.style.height = Some(Len::Px(rest));
                    }
                }
                d = d.child(div().children(blocks(
                    &[Node::Element(sub)],
                    &merged,
                    opts,
                )));
            }
        }
        if let Some(Node::Element(sp)) = span {
            // Хост сегментов — блок taffy (`Display::Block`,
            // `vendor/gpui/src/style.rs:862`), и поля соседних детей
            // он схлопывает сам (`vendor/taffy/src/compute/
            // block.rs:186-210`): соседние спаннеры ОДНОГО предка —
            // верно («the margins of two adjacent spanners will
            // collapse with each other»; `multicol-span-all-margin-
            // 003`). Запрещённое схлопывание гасит нулевая гибкая
            // сторожка — её taffy насквозь не проходит
            // (`has_styles_preventing_being_collapsed_through`:
            // `!style.is_block()`): (1) перед ПЕРВЫМ куском-спаннером
            // — многоколоночник сам контекст форматирования, и
            // верхнее поле спаннера сквозь его верх не уходит;
            // (2) между спаннерами РАЗНЫХ исходных предков (метка
            // `kamin-span-parent`, `spanner_parts`).
            let key = sp.attr("kamin-span-parent").unwrap_or("").to_string();
            if !seg_open || span_prev.as_ref().is_some_and(|k| *k != key) {
                d = d.child(div().flex());
            }
            seg_open = true;
            span_prev = Some(key);
            let inner = inline::inherit(&merged, &sp.style);
            // Спаннер — независимый контекст форматирования
            // (§column-span). Голый `styled_div_with` — блок taffy,
            // и тот схлопывал поле первого/последнего ребёнка
            // сквозь край спаннера (`vendor/taffy/src/compute/
            // block.rs:186`): в `multicol-span-all-margin-nested-
            // firstchild-001` `<span style="margin: 2em 0">` уводил
            // `<h6>` вниз, и чёрного фона не было видно вовсе.
            // Оболочка — гибкая колонка, ровно как у блока общего
            // пути (`d.flex().flex_col()` при пустом `display`), где
            // поля детей сводит сам `blocks()` — а он с предикатом
            // `own_context_style` поле наружу больше не отдаёт.
            let shell = styled_div_with(sp, &inner);
            let shell = if matches!(inner.display, None | Some(Display::Block))
                && inner.vertical != Some(true)
            {
                shell.flex().flex_col()
            } else {
                shell
            };
            d = d.child(shell.children(blocks(
                &sp.children,
                &inner,
                opts,
            )));
        }
    }
    // Нижняя сторожка: нижнее поле последнего спаннера остаётся
    // внутри многоколоночника — он независимый контекст
    // форматирования (CSS 2.1 §8.3.1).
    d = d.child(div().flex());
    return d.into_any_element();
}
