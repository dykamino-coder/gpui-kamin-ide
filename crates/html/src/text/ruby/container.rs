//! Строчный контейнер руби (`<ruby>` атомом строки): сегменты, базы и аннотации.
// owner: A

use crate::render::*;

pub(crate) fn ruby_container_atom(
    inherited: &Computed,
    e: &Element,
    opts: &RenderOpts,
) -> Option<AnyElement> {
    use crate::computed::{RubyAlign, TextAlign};
    let mut merged = inline::inherit(inherited, &e.style);
    // Внутри руби знак акцента не разворачивается (как прежде).
    merged.text_emphasis = None;
    let segments = ruby_segments(&e.children);
    // Без хотя бы одной непустой аннотации руби — обычный строчный
    // (`ruby-line-breaking-001`: `<rtc><rt>` пустой; `ruby-intrinsic-isize-*`).
    if !segments
        .iter()
        .any(|s| s.levels.iter().any(|l| l.units.iter().any(|u| !ruby_unit_blank(u))))
    {
        return None;
    }
    // `ruby-align`: `space-around` (начальное) и `center` — по центру
    // (у латиницы точек выключки нет, §4.3); `space-between` —
    // выключка обоих краёв; `start` — к началу.
    match merged.ruby_align {
        Some(RubyAlign::Start) => merged.text_align = Some(TextAlign::Start),
        Some(RubyAlign::SpaceBetween) => {
            merged.text_align = Some(TextAlign::Justify);
            merged.text_align_last = Some(TextAlign::Justify);
        }
        _ => merged.text_align = Some(TextAlign::Center),
    }
    // `ruby-position` уровня k (css-ruby-1 §4.1): явное `over`/`under`
    // — для всех уровней; начальное `alternate` (`None`) — первый
    // уровень над базой, следующий под, и так далее. `alternate
    // under` пока не различается (первый уровень над).
    let level_under = |k: usize| match merged.ruby_under {
        Some(under) => under,
        None => k % 2 == 1,
    };
    // Стопка «база, уровни наружу»: под базой — прямая гибкая колонка.
    let under_stack = || div().flex().flex_col().flex_shrink_0();
    // Над базой — сетка 1×1: база и обёртка уровней делят одну
    // ячейку. Первая базовая сетки — у первого по порядку элемента
    // первого ряда (css-grid-2 §10.7 «grid baselines»: `row-major
    // grid order`), то есть у базы. Прежняя `column-reverse` отдавала
    // базовую линию ВИЗУАЛЬНО начального элемента (css-flexbox-1
    // §8.5 «startmost flex item», Taffy 0.14) — обёртки уровней,
    // а не базы (`ruby-align-001`, `empty-ruby-text-container-float`).
    let over_stack = |base: AnyElement| {
        div()
            .grid()
            .flex_shrink_0()
            .grid_template_cols(vec![gpui::GridTrack::Auto])
            .grid_template_rows(vec![gpui::GridTrack::Auto])
            .child(div().row_start(1).col_start(1).child(base))
    };
    // Уровни аннотаций уходят из БЛОЧНОГО потока колонки (css-ruby-1
    // §3.4: «ordinarily, ruby annotation containers and ruby
    // annotation boxes do not contribute to the measured height of a
    // line's inline contents»). Обёртка нулевой ГЛАВНОЙ высоты:
    // `h_0` задаёт основу, `min_h_0` снимает автоминимум гибкого
    // элемента (`vendor/taffy/src/compute/flexbox.rs:824-827`: иначе
    // `min-height: auto` вернёт высоту содержимого). Над базой
    // содержимое прижато к главному концу — свободное место
    // отрицательное, и `FlexEnd` отдаёт его целиком
    // (`compute/common/alignment.rs:61-67`), уровни встают НАД нулём;
    // под базой обычный `flex-start` свисает вниз.
    //
    // Зачем: первую базовую линию гибкой КОЛОНКИ taffy берёт у
    // первого DOM-ребёнка (`flexbox.rs:405-419`), а `child.baseline`
    // колонки (`:2152`) содержит `total_offset_main`, который в
    // `column-reverse` равен ВЫСОТЕ аннотаций. Пока уровни лежали в
    // самой колонке, атом отдавал строке базовую линию на H(ann)
    // ниже: замерено `target/ruby-probe/probe-c.html` (Ahem 64px) —
    // 88 dev вместо 104/144/184 при `line-height` 1/2/3, и
    // `probe-d.html` — 128 вместо 144 при `rt { font-size: 64px }`,
    // 56 вместо 72 при `rb { font-size: 32px }`. На живой паре
    // `text-box-trim-ruby-start-001` (Ahem 40px, H(ann) = 25 dev) это
    // давало строку на 15 dev ниже эталона и базу на 25 dev ниже.
    let level_wrap = |under: bool| {
        let w = div().flex().flex_col().flex_shrink_0().h_0().min_h_0();
        if under {
            w
        } else {
            // Над базой: та же ячейка сетки `over_stack`, прижатая к
            // её верху; уровни свисают вверх от нуля.
            let mut w = w.justify_end().row_start(1).col_start(1);
            w.style().align_self = Some(gpui::AlignItems::Start);
            w
        }
    };
    // Единица из ОДНОГО `<rb>`/`<rt>` (или элемента с ролью базы /
    // аннотации по `display`) рисуется его собственной БЛОЧНОЙ
    // коробкой: распорка строки — от его кегля и `line-height` (UA
    // `rt { font-size: 50%; line-height: 1 }`), а не от контейнера
    // руби. Прежде коробка аннотации носила строку контейнера, и при
    // разном кегле контейнера в тесте и эталоне
    // (`ruby-base-different-size`: 16px против 32px) аннотации
    // вставали на разной высоте; в `nested-ruby-pairing-001` уровень
    // `<rt>` (стиль контейнера) выходил выше уровня `<rtc>`. Поля,
    // рамка и фон единицы действуют (§3.3: базы и аннотации —
    // строчные коробки, все их свойства применяются). Анонимная
    // единица (текст) — по-прежнему абзац со стилем уровня.
    // Содержимое единицы не рвётся: разрыв внутри базы — только
    // вынужденный (§3.5.2), а атом монолитен; без `nowrap` анонимная
    // база `あい` рвалась внутри колонки (эталон `rbc-rtc-basic-001`).
    // Единица из одних пробелов (межбазовая, межаннотационная,
    // межсегментная — css-ruby-1 §2.2 п.6) — это ПРОБЕЛ строки, а не
    // пустота: у нас единица — свой блок, и пробел в нём срезался как
    // краевой, единица выходила нулевой, а колонка без строки ломала
    // общую базовую линию ряда (`ruby-box-generation-*`). Пробел
    // заменяется неразрывным: ширина пробела, строка и базовая на месте.
    // Пробельной считается и единица, где пробел обёрнут строчным
    // элементом: эталоны пишут межбазовый пробел как
    // `<rb><span> </span></rb>`, и пустая колонка без строки рядом с
    // колонкой «e» роняла базовую ряда (`ruby-box-generation-001-ref`).
    // Остаётся ОДИН неразрывный пробел — прочие пробельные тексты
    // единицы схлопнулись бы с ним (css-text-3 §4.1.1).
    fn only_space(nodes: &[Node]) -> bool {
        nodes.iter().all(|n| match n {
            Node::Text(t) => blank_text(t),
            Node::Element(k) => {
                let plain = ruby_role(k).is_some_and(|r| r != crate::computed::RubyRole::Container)
                    || (k.style.display.is_none() && k.style.inline_display != Some(false) && !replaced_tag(k));
                plain && only_space(&k.children)
            }
        })
    }
    fn has_space(nodes: &[Node]) -> bool {
        nodes.iter().any(|n| match n {
            Node::Text(t) => !t.is_empty(),
            Node::Element(k) => has_space(&k.children),
        })
    }
    fn spaced(nodes: &[Node], done: &mut bool) -> Vec<Node> {
        nodes
            .iter()
            .map(|n| match n {
                Node::Text(t) if !t.is_empty() && !*done => {
                    *done = true;
                    Node::Text("\u{a0}".into())
                }
                Node::Text(_) => Node::Text(String::new()),
                Node::Element(k) => {
                    let mut k = k.clone();
                    k.children = spaced(&k.children, done);
                    Node::Element(k)
                }
            })
            .collect()
    }
    let unit_box = |nodes: &[Node], style: &Computed| -> AnyElement {
        let blank_space = !nodes.is_empty() && only_space(nodes) && has_space(nodes);
        let owned;
        let nodes = if blank_space {
            owned = spaced(nodes, &mut false);
            &owned[..]
        } else {
            nodes
        };
        let mut style = style.clone();
        style.nowrap = Some(true);
        style.ruby_unit = true;
        // CSS Ruby 1 §2.1.1: these units share an inline formatting
        // context, rather than starting indented block paragraphs.
        // Blink line_breaker.cc:846-848 excludes ruby sub-line breakers.
        style.text_indent = Some(Len::Px(0.0));
        style.text_indent_each_line = Some(false);
        style.text_indent_hanging = Some(false);
        if let [Node::Element(k)] = nodes
            && matches!(
                ruby_role(k),
                Some(crate::computed::RubyRole::Base) | Some(crate::computed::RubyRole::Text)
            )
        {
            let mut block = k.clone();
            ruby_transform::clear(&mut block.style);
            block.style.display = Some(Display::Block);
            block.style.inline_display = None;
            block.style.ruby_role = None;
            block.style.text_indent = Some(Len::Px(0.0));
            block.style.text_indent_each_line = Some(false);
            block.style.text_indent_hanging = Some(false);
            return div()
                .children(blocks(&[Node::Element(block)], &style, opts))
                .into_any_element();
        }
        div().children(blocks(nodes, &style, opts)).into_any_element()
    };
    let empty: RubyUnit = Vec::new();
    // Стопка уровней одной стороны — узел, чью высоту знает строка
    // (`lines::ruby_extent`, css-ruby-1 §3.4).
    // Полулидинг базы: стопка стоит на краю её коробки строки.
    let base_half = {
        let size = match merged.font_size {
            Some(Len::Px(v)) => v,
            _ => opts.base_size(),
        };
        let family = merged.font_family.clone().unwrap_or_default();
        let (asc, desc, _) = crate::metrics::vmetrics_px(&family, size);
        let line = match merged.line_height {
            Some(Len::Px(v)) => v,
            Some(Len::Pct(k)) | Some(Len::Em(k)) => k * size,
            _ => size * normal_fraction(&merged, opts),
        };
        (line - (asc + desc)) / 2.0
    };
    let extent = |d: gpui::Div, under: bool| {
        crate::lines::ruby_extent(d.into_any_element(), under, base_half)
    };
    let mut row = div().flex().flex_row().items_baseline().flex_shrink_0();
    for seg in &segments {
        // Стиль уровня: аннотации внутри `<rtc>` наследуют от него.
        let level_style: Vec<Computed> = seg
            .levels
            .iter()
            .map(|l| match &l.container {
                Some(c) => inline::inherit(&merged, c),
                None => merged.clone(),
            })
            .collect();
        // Колонок столько, сколько баз или аннотаций самого длинного
        // нераспорного уровня; нехватка — пустые анонимные (§2.3.2).
        let columns = seg.bases.len().max(
            seg.levels
                .iter()
                .filter(|l| !l.spanning)
                .map(|l| l.units.len())
                .max()
                .unwrap_or(0),
        );
        // Колонки не сжимаются: ширина колонки — по самому широкому
        // из базы и аннотаций (§3.1.1), а не по остатку строки.
        let mut cols = div().flex().flex_row().items_baseline().flex_shrink_0();
        for i in 0..columns {
            // Уровни над базой — в обратную стопку вместе с базой,
            // уровни под ней — прямой стопкой снаружи.
            let mut over_anns: Vec<AnyElement> = Vec::new();
            let mut under: Vec<AnyElement> = Vec::new();
            for (k, (l, style)) in seg.levels.iter().zip(&level_style).enumerate() {
                if l.spanning {
                    continue;
                }
                let nodes = l.units.get(i).unwrap_or(&empty);
                let base = ruby_hiding::text(seg.bases.get(i).unwrap_or(&empty));
                let nodes = if ruby_hiding::hidden(nodes, &base, style) { &empty } else { nodes };
                let ann = unit_box(nodes, style);
                if level_under(k) {
                    under.push(ann);
                } else {
                    over_anns.push(ann);
                }
            }
            // База — ПЕРВЫЙ элемент ячейки `over_stack`, обёртка уровней
            // идёт после неё в той же ячейке. Внутри обёртки уровни
            // в обратном порядке: нулевой (ближний к базе) — внизу.
            // css-ruby-1 §4.4 ruby-overhang: a single-column ruby lets
            // its line know the base content width, so an annotation
            // wider than the base may overhang the neighbours
            // (`lines::lay_atoms`, Blink `ruby_utils.cc` GetOverhang).
            let base_nodes = seg.bases.get(i).unwrap_or(&empty);
            let base_el = if segments.len() == 1 && columns == 1 && !seg.levels.is_empty() {
                let base_font = match merged.font_size {
                    Some(Len::Px(v)) => v,
                    _ => opts.base_size(),
                };
                // The annotation's own font size: UA `rt { font-size: 50% }`.
                let ann_font = seg
                    .levels
                    .first()
                    .and_then(|l| l.units.first())
                    .and_then(|u| match u.as_slice() {
                        [Node::Element(k)] => match k.style.font_size {
                            Some(Len::Px(v)) => Some(v),
                            Some(Len::Pct(p)) | Some(Len::Em(p)) => Some(p * base_font),
                            _ => None,
                        },
                        _ => None,
                    })
                    .unwrap_or(base_font * 0.5);
                crate::lines::ruby_base_with_overhang(
                    merged.ruby_overhang.unwrap_or(crate::computed::RubyOverhang::Auto),
                    ann_font / 2.0,
                    merged.ruby_align == Some(crate::computed::RubyAlign::Start),
                    base_font,
                    || unit_box(base_nodes, &merged),
                )
            } else {
                unit_box(base_nodes, &merged)
            };
            let mut over = over_stack(base_el);
            if !over_anns.is_empty() {
                over = over.child(level_wrap(false).child(extent(
                    div()
                        .flex()
                        .flex_col()
                        .flex_shrink_0()
                        .children(over_anns.into_iter().rev()),
                    false,
                )));
            }
            let col = if under.is_empty() {
                over.into_any_element()
            } else {
                under_stack()
                    .child(over)
                    .child(level_wrap(true).child(extent(
                        div().flex().flex_col().flex_shrink_0().children(under),
                        true,
                    )))
                    .into_any_element()
            };
            cols = cols.child(col);
        }
        let mut seg_el = cols.into_any_element();
        for (k, (l, style)) in seg.levels.iter().zip(&level_style).enumerate() {
            if l.spanning {
                let nodes = l.units.first().unwrap_or(&empty);
                let base: String = seg.bases.iter().map(|b| ruby_hiding::text(b)).collect();
                let nodes = if ruby_hiding::hidden(nodes, &base, style) { &empty } else { nodes };
                let host = if level_under(k) {
                    under_stack().child(seg_el)
                } else {
                    over_stack(seg_el)
                };
                seg_el = host
                    .child(level_wrap(level_under(k)).child(extent(
                        div()
                            .flex()
                            .flex_col()
                            .flex_shrink_0()
                            .child(unit_box(nodes, style)),
                        level_under(k),
                    )))
                    .into_any_element();
            }
        }
        row = row.child(seg_el);
    }
    Some(row.into_any_element())
}
