//! Строчные атомы: `atom_element`.
// owner: A

use crate::render::*;

/// Не-текстовые инлайн-элементы, которые в поток встроить нельзя.
/// Строчный атом с размером по ключевому слову (`width: min-content` и
/// родня): дорожка по содержимому ставится той же обёрткой, что у блочного
/// пути (`content_sized`). Без неё атом шёл в ряд строки голым, гибкий ряд
/// брал его основу по max-content, и `inline-grid`/`inline grid-lanes` с
/// `width: min-content` раскладывался по max-content: доли `1fr 2fr 1fr 1fr`
/// при базах по 2ch раздавались, вторая дорожка выходила 4ch
/// (`grid-lanes-intrinsic-sizing-cols-002-fr`; css-sizing-3 §5.1).
pub(crate) fn atom_element(e: &Element, inherited: &Computed, opts: &RenderOpts) -> Option<AnyElement> {
    if let Some(physical) = rotated_atom::physical(e, inherited, opts) {
        return Some(physical);
    }
    let resolved = rotated_atom::resolved(e, inherited);
    let e = resolved.as_ref().unwrap_or(e);
    let el = atom_element_raw(e, inherited, opts)?;
    // CSS Sizing 3 §5.1: intrinsic keywords in the block axis behave as auto.
    let keyword = |l: Option<Len>| matches!(l, Some(Len::MinContent) | Some(Len::MaxContent) | Some(Len::FitContent));
    // Повёрнутый абзац вертикального письма (`rotated_line`) набирается
    // горизонтально, но ось строки там вертикальна.
    let inline_axis_only = inherited.vertical != Some(true)
        && inherited.rotated_line != Some(true)
        && e.style.vertical != Some(true)
        && keyword(e.style.width)
        && !keyword(e.style.height);
    let wraps = inline_axis_only
        && content_sized_wraps(e)
        && !replaced_tag(e)
        && !at_static_position(&e.style)
        && !matches!(e.tag.as_str(), "input" | "select" | "textarea" | "button");
    Some(if wraps { content_sized(el, &e.style, &inline::inherit(inherited, &e.style), (None, None)) } else { el })
}

pub(crate) fn atom_element_raw(e: &Element, inherited: &Computed, opts: &RenderOpts) -> Option<AnyElement> {
    let svg_sized = svg_percentage_size::resolve(e, inherited);
    let e = svg_sized.as_ref().unwrap_or(e);
    // Строчный атом — независимый контекст форматирования: строки внутри
    // `inline-block` в бюджет `line-clamp` не входят (css-overflow-4 §5.3),
    // иначе строка атома считалась ВТОРОЙ поверх строки абзаца
    // (`line-clamp-032`). Содержимое атома строится ниже голым `blocks()`,
    // мимо сторожа общего пути. `display: inline` после каскада — это
    // `InlineBlock` с `inline_display` — атомом не является.
    let _clamp_bfc = (matches!(
        e.style.display,
        Some(Display::InlineBlock)
            | Some(Display::InlineFlex)
            | Some(Display::InlineGrid)
            | Some(Display::InlineTable)
    ) && e.style.inline_display != Some(true)
        && crate::interact::clamp_context().is_some())
    .then(crate::interact::ClampGuard::enter_bfc);
    // Элементу формы нужен СЛИТЫЙ стиль: в своём у него единицы шрифта ещё не
    // разрешены (`width: 3ch` считался бы по базовому кеглю, а не по своему),
    // да и наследуемое до поля иначе не доходит.
    if let Some(el) = crate::forms::element(e, &inline::inherit(inherited, &e.style), opts) {
        // Трансформы поля формы шли МИМО обёртки: инпуты стояли ровно, а
        // эталон сдвигал (transform-input-001..019).
        return Some(transformed(el, &e.style, inherited));
    }
    // Абсолютный элемент без заданных краёв стоит на СТАТИЧЕСКОЙ позиции — там,
    // где он оказался бы в потоке. Внутри строки это место знает только сама
    // строка, поэтому в неё встаёт пустышка нулевого размера, а элемент висит
    // от её угла. Без этого раскладка ставила его в угол ближайшего
    // позиционированного предка, и текст уезжал в начало абзаца.
    // ★ ЗАМЕРЕНО И ОТКАЧЕНО (01.09): пускать сюда коробку с ОДНИМ заданным
    // краем — §10.6.4 решает оси независимо, и `left: 0` не должен отменять
    // статическое разрешение `top: auto` (`abspos-009`: зелёный на 37.6
    // точки выше эталона). Занятую ось при этом сообщал `fixed_axes`.
    // Срез из 1691 пары абсолютов: зелёных 1620 → 1596. Вся потеря —
    // замещаемые (`absolute-replaced-height-010/013/017/020/024/027/031/034`,
    // `-width-022..025` из 0.00-0.18 в 1.9-7.9): у них пустышка нулевая, и
    // размер по свободной оси считается уже не от содержащего блока.
    // Возвращать вместе с ненулевой распоркой по занятой оси.
    if at_static_position(&e.style) {
        let mut merged = inline::inherit(inherited, &e.style);
        // Позиционирование с внутренней коробки СНИМАЕТСЯ. Содержащим блоком
        // ей стала бы нулевая пустышка, а ширина у неё «по содержимому» —
        // в нулевом блоке это ноль, и элемент пропадал вовсе. Без
        // позиционирования она меряется своим содержимым и висит от угла
        // пустышки, то есть ровно от статической позиции.
        merged.position = None;
        merged.abs_static = true;
        // Замещаемый элемент строит своя ветка: голая коробка со стилем
        // теряла содержимое (сломанная картинка с alt-подписью пропадала,
        // `abs-pos-vlr-border-001`). Позиция снимается копией — та же
        // причина, что и у merged ниже.
        let replaced: Option<AnyElement> = match e.tag.as_str() {
            "img" | "svg" => {
                let mut copy = e.clone();
                copy.style.position = None;
                // `image-orientation` НАСЛЕДУЕТСЯ (css-images-3 §5.4), а
                // копия несёт только собственный стиль элемента: без этой
                // строки `image-orientation: none`, заданный на `body`, до
                // картинки не доезжает и EXIF-разворот применяется всё равно.
                copy.style.image_orient_none = merged.image_orient_none;
                Some(match copy.tag.as_str() {
                    // CSS-коробка `<svg>` (рамка, отбивка) — `svg_replaced`;
                    // позиция снята и со слитого стиля, как с копии выше.
                    "svg" => {
                        let mut unpositioned = merged.clone();
                        unpositioned.position = None;
                        svg_replaced(&copy, &copy, &unpositioned).unwrap_or_else(|| image(&copy))
                    }
                    _ => image(&copy),
                })
            }
            _ => None,
        };
        let inner = styled_div_with(e, &merged).children(blocks(&e.children, &merged, opts));
        // Пустышка прижимается к ВЕРХУ строки: иначе она садится на базовую
        // линию, и содержимое уезжает под неё — абсолютный блок оказывался
        // ниже своей строки, а не на её месте (видно на `static-position`:
        // зелёный блок висел под коробкой, красное проступало).
        // Рисуется элемент ПОВЕРХ соседей по строке, поэтому содержимое
        // уходит в верхний слой блока-контейнера, а в строке остаётся щуп: он
        // и держит место, и сообщает, куда потом вернуть содержимое.
        let spot: crate::interact::SpotCell = Default::default();
        let below = e.style.z_index.is_some_and(|z| z < 0);
        // Блочный элемент встал бы на НОВУЮ строку — там его статическая
        // позиция и находится: левый край содержимого родителя, верх — низ
        // текущей строки. Строчный остаётся точкой в самой строке.
        let inline_level = match e.style.display {
            Some(Display::InlineBlock)
            | Some(Display::InlineFlex)
            | Some(Display::InlineGrid)
            | Some(Display::InlineTable) => true,
            Some(Display::GridLanes) => e.style.lanes_inline,
            // Блокифицированный `display: inline` (§9.7) для СТАТИЧЕСКОЙ
            // позиции остаётся строчным: гипотеза §10.3.7 считается без
            // блокификации (htb-rtl-*).
            Some(_) => e.style.inline_display == Some(true),
            None => e.inline,
        };
        // Флаги направления нужны и СТРОЧНОМУ атому: в rtl-строке статическая
        // позиция — правый край, заместитель вешается правым краем на точку
        // распорки. Начало новой строки — только у блочного.
        spot.set(crate::interact::Spot {
            hole: None,
            next_line: (!inline_level).then(|| line_height_px(inherited, opts)),
            rtl: inherited.rtl == Some(true),
            vertical: inherited.vertical == Some(true),
            vertical_rl: inherited.vertical_rl == Some(true),
            ..Default::default()
        });
        let probe = crate::interact::spot_probe(spot.clone(), false);
        let inner = match replaced {
            Some(el) => el,
            None => inner.into_any_element(),
        };
        let taken = if below {
            Some(inner)
        } else {
            crate::interact::late_push(spot, inner)
        };
        return match taken {
            None => Some(probe),
            Some(kept) => {
                let mut hole = div().relative().w_0().h_0().flex_shrink_0();
                hole.style().align_self = Some(gpui::AlignItems::FlexStart);
                Some(hole.child(kept).into_any_element())
            }
        };
    }
    // Абсолютный элемент С заданными краями считается от ближайшего
    // позиционированного предка, а не от строки. Места в строке он не
    // занимает вовсе — потому и пустышка нулевая, и БЕЗ `relative`: иначе
    // содержащим блоком стала бы она сама, и края отсчитывались бы от неё.
    // Пока он был обычной коробкой куска, строка росла под его высоту.
    if matches!(
        e.style.position,
        Some(crate::computed::Position::Absolute) | Some(crate::computed::Position::Fixed)
    ) {
        let merged = inline::inherit(inherited, &e.style);
        // Содержащий блок — позиционированный СТРОЧНЫЙ предок в этом абзаце
        // (CSS 2.1 §10.1 п.4): края по обеим осям считает `lines.rs` от
        // прямоугольника его фрагментов, коробка идёт без пустышки и слоёв.
        let inline_cb = crate::inline::take_atom_cb()
            && e.style.position == Some(crate::computed::Position::Absolute)
            && (edge_set(e.style.inset.left) || edge_set(e.style.inset.right))
            && (edge_set(e.style.inset.top) || edge_set(e.style.inset.bottom))
            && e.style.z_index.unwrap_or(0) >= 0;
        if inline_cb {
            crate::inline::note_abs_cb();
        }
        // ПРОБОВАЛИ И ОТКАТИЛИ: отдавать элемент без пустышки, чтобы `inset: 0`
        // считался от позиционированного ПРЕДКА. В раскладке под нами
        // содержащим блоком служит ЛЮБОЙ родитель, поэтому вынос ничего не
        // меняет: css-position 35 → 34, зелёный прямоугольник
        // `position-absolute-in-inline-005` так и не появился.
        // Пустышка нулевого размера сама становится содержащим блоком, и
        // края, заданные с ОБЕИХ сторон оси, схлопываются в ничто. Такому
        // элементу коробка нужна настоящая, поэтому он идёт без пустышки.
        // Остальным она нужна: без неё сдвигаются соседи (замерено на
        // `position-sticky-contained-by-display-table`).
        // ПРОБОВАЛИ И ОТКАТИЛИ: считать «растянутым» и элемент с ДОЛЕЙ
        // размера, чтобы доля не бралась от нулевой пустышки. Замерено по
        // семьям *replaced*, positioning/*, normal-flow/*, *float*: 0 и 0.
        let stretched = inline_cb
            || (edge_set(e.style.inset.left) && edge_set(e.style.inset.right))
            || (edge_set(e.style.inset.top) && edge_set(e.style.inset.bottom))
            || matches!(e.style.width, Some(Len::Pct(_)));
        // Замещаемый элемент строит своя ветка: дети `<svg>` — не блоки,
        // путь блоков давал пустую коробку (clip-path-ellipse-2-ref: рисунок
        // absolute с left/top не рисовался вовсе).
        // Тот же список, что и у обычного пути: у замещаемых детей-блоков
        // нет, и блочная ветка давала пустую коробку — картинка с краями
        // не рисовалась вовсе.
        if matches!(
            e.tag.as_str(),
            "svg" | "img" | "canvas" | "video" | "embed" | "object" | "iframe"
        ) {
            let mut copy = e.clone();
            copy.style.image_orient_none = merged.image_orient_none;
            copy.style.position = None;
            // Поля несёт ДЕРЖАТЕЛЬ: снимали только позицию, и `margin`
            // прикладывался дважды — раз держателем, раз внутренней коробкой
            // (`absolute-replaced-width-050`: край 48 превращался в 72).
            copy.style.margin = Default::default();
            // Долю размера держатель тоже несёт сам: внутри она считалась ОТ
            // НЕГО и выходила долей от доли — `width: 50%` давало четверть
            // содержащего блока (`absolute-replaced-width-006`). Внутренней
            // коробке остаётся заполнить держателя.
            if matches!(copy.style.width, Some(Len::Pct(_))) {
                copy.style.width = Some(Len::Pct(1.0));
            }
            if matches!(copy.style.height, Some(Len::Pct(_))) {
                copy.style.height = Some(Len::Pct(1.0));
            }
            let built = if e.tag == "svg" {
                crate::svg::element(&copy).unwrap_or_else(|| image(&copy))
            } else if replaced_content::default_iframe(e) {
                // The holder owns the CSS box; empty content paints no second border.
                div().w_0().h_0().flex_shrink_0().into_any_element()
            } else {
                image(&copy)
            };
            let holder = styled_div_with(e, &merged).child(built);
            // Замещаемый атом БЕЗ позиционированного предка считается от
            // начального содержащего блока (§10.1 п.4), а не от строки, где он
            // написан: с обеими заданными осями место в строке ему не нужно
            // вовсе. Тот же приём, что у блочного пути (`to_icb`).
            let x_set = edge_set(e.style.inset.left) || edge_set(e.style.inset.right);
            let y_set = edge_set(e.style.inset.top) || edge_set(e.style.inset.bottom);
            // CSS 2.1 §§10.1, 10.3.7, 10.6.4: explicit insets use the
            // containing block; the auto axis keeps its inline static spot.
            // Route to that block's layer, rather than the paragraph wrapper.
            if x_set != y_set && e.style.z_index.unwrap_or(0) >= 0 {
                let spot: crate::interact::SpotCell = Default::default();
                spot.set(crate::interact::Spot {
                    hole: None,
                    next_line: None,
                    fixed_axes: (x_set, y_set),
                    line_thickness: line_height_px(inherited, opts),
                    rtl: inherited.rtl == Some(true),
                    vertical: inherited.vertical == Some(true),
                    vertical_rl: inherited.vertical_rl == Some(true),
                    own_vertical: e.style.vertical == Some(true),
                    replaced: matches!(
                        e.tag.as_str(),
                        "img" | "iframe" | "video" | "canvas" | "object" | "embed" | "svg"
                    ),
                    ..Default::default()
                });
                let probe = crate::interact::spot_probe(spot.clone(), false);
                return match inline_replaced_position::push(
                    spot,
                    inline_abs_paint_last(e, holder.into_any_element()),
                    &e.style,
                    inherited,
                ) {
                    None => Some(probe),
                    Some(kept) => {
                        let mut hole = div().relative().w_0().h_0().flex_shrink_0();
                        hole.style().align_self = Some(gpui::AlignItems::FlexStart);
                        Some(hole.child(kept).into_any_element())
                    }
                };
            }
            // A negative `z-index` joins the ICB layer too, painted in the
            // bottom layer (CSS 2.1 §9.9 step 3) — in place it painted over
            // its later negative-z siblings (`shape-image-009`).
            let below_icb = e.style.z_index.is_some_and(|z| z < 0) && !stacking_context(inherited);
            if x_set
                && y_set
                && !inline_cb
                && (e.style.z_index.unwrap_or(0) >= 0 || below_icb)
                && !(inherited.cb_ancestor || crate::inline::establishes_cb(inherited))
            {
                let holder: AnyElement = if below_icb {
                    crate::interact::Underlay::new(holder.into_any_element()).into_any_element()
                } else {
                    holder.into_any_element()
                };
                let spot: crate::interact::SpotCell = Default::default();
                spot.set(crate::interact::Spot {
                    fixed_axes: (true, true),
                    rtl: inherited.rtl == Some(true),
                    vertical: inherited.vertical == Some(true),
                    vertical_rl: inherited.vertical_rl == Some(true),
                    own_vertical: e.style.vertical == Some(true),
                    replaced: matches!(
                        e.tag.as_str(),
                        "img" | "iframe" | "video" | "canvas" | "object" | "embed" | "svg"
                    ),
                    ..Default::default()
                });
                // Слоя нет — элемент возвращается назад, и рисуем его на
                // месте прежним путём.
                match crate::interact::icb_push(spot, holder.into_any_element()) {
                    None => {
                        return Some(div().w_0().h_0().flex_shrink_0().into_any_element());
                    }
                    Some(kept) => {
                        return Some(
                            div()
                                .w_0()
                                .h_0()
                                .flex_shrink_0()
                                .child(kept)
                                .into_any_element(),
                        );
                    }
                }
            }
            return Some(if stretched {
                holder.into_any_element()
            } else {
                div()
                    .w_0()
                    .h_0()
                    .flex_shrink_0()
                    .child(holder)
                    .into_any_element()
            });
        }
        let inner = styled_div_with(e, &merged).children(blocks(&e.children, &merged, opts));
        // Незамещаемая коробка с РОВНО ОДНОЙ заданной осью — тем же щупом, что
        // и замещаемая выше: без этого `<span>` с одним краем падал в нулевой
        // держатель, и край считался от него (`abs-pos-non-replaced-vlr-017`:
        // `right: 2em` уводил коробку на свои же 160 влево).
        let x_set = edge_set(e.style.inset.left) || edge_set(e.style.inset.right);
        let y_set = edge_set(e.style.inset.top) || edge_set(e.style.inset.bottom);
        if x_set != y_set && e.style.z_index.unwrap_or(0) >= 0 {
            let spot: crate::interact::SpotCell = Default::default();
            spot.set(crate::interact::Spot {
                hole: None,
                next_line: None,
                fixed_axes: (x_set, y_set),
                line_thickness: line_height_px(inherited, opts),
                rtl: inherited.rtl == Some(true),
                vertical: inherited.vertical == Some(true),
                vertical_rl: inherited.vertical_rl == Some(true),
                own_vertical: e.style.vertical == Some(true),
                ..Default::default()
            });
            let probe = crate::interact::spot_probe(spot.clone(), false);
            return match crate::interact::late_push(spot, inline_abs_paint_last(e, inner.into_any_element())) {
                None => Some(probe),
                Some(kept) => {
                    let mut hole = div().relative().w_0().h_0().flex_shrink_0();
                    hole.style().align_self = Some(gpui::AlignItems::FlexStart);
                    Some(hole.child(kept).into_any_element())
                }
            };
        }
        if stretched {
            return Some(inner.into_any_element());
        }
        return Some(
            div()
                .w_0()
                .h_0()
                .flex_shrink_0()
                .child(inner)
                .into_any_element(),
        );
    }
    // Многоколоночная коробка В СТРОКЕ (`inline-block` с `column-count`) —
    // такой же случай, как строчная таблица ниже: своя раскладка живёт в
    // блочном пути, а сюда приходил голый `blocks()`, и колонок не
    // получалось вовсе — шесть детей вставали одним столбцом. Так красными
    // были ЭТАЛОНЫ семьи `column-grid-lanes-container-baseline-*`: сам тест
    // на лунках рисуется верно, а эталон написан на многоколоннике в
    // `display: inline-block`.
    //
    // Ширину сжатой коробке даём по css-multicol-1 §3.4: когда заданы и
    // число колонок, и их ширина, использованная ширина коробки —
    // `count * width + (count - 1) * gap`. Без этого колонки делили бы
    // ширину содержимого строки. Умолчание `column-gap: normal` — кегль
    // (css-align §8.3), как и в блочном пути.
    //
    // ЗАМЕРЕНО: правка верна по пробе, но своды не двигает — CSS2
    // 5343 -> 5343 и CSS3 2420 -> 2420, ноль приобретено, ноль потеряно:
    // эталоны этой семьи держит ещё и выравнивание по базовой линии.
    // Лунки сетки — та же история: раскладку лунок строит только блочный
    // путь, а строчный отдавал голый `blocks()`, и `display: inline
    // grid-lanes` терял лунки целиком.
    if e.style.display == Some(Display::GridLanes) {
        return Some(element(e, inherited, opts));
    }
    if multicol_container(&e.style) {
        let merged = inline::inherit(inherited, &e.style);
        let gap = match e.style.column_gap {
            Some(Len::Px(v)) => v,
            _ => match merged.font_size {
                Some(Len::Px(size)) => size,
                _ => opts.base_size(),
            },
        };
        let shrink_to_fit = match (e.style.column_count, e.style.column_width) {
            (Some(c), Some(Len::Px(w))) if w > 0.0 => Some(c as f32 * w + (c as f32 - 1.0) * gap),
            _ => None,
        };
        if e.style.width.is_some() || shrink_to_fit.is_some() {
            let mut copy = e.clone();
            if copy.style.width.is_none() {
                copy.style.width = shrink_to_fit.map(Len::Px);
            }
            return Some(element(&copy, inherited, opts));
        }
    }
    // Таблица в строке — атомарная коробка со своей табличной раскладкой:
    // путь блока строил бы детей-ряды как обычные блоки, без решётки.
    if e.style.display == Some(Display::InlineTable) {
        let built = table(e, &inline::inherit(inherited, &e.style), opts);
        // `vertical-align` коробки в строке: низ/верх/середина СТРОКИ, а не
        // базовая линия. Строка — гибкий ряд, и место коробки задаёт её
        // собственный `align-self`.
        let self_align = match e.style.vertical_align {
            Some(Align::End) => Some(gpui::AlignItems::FlexEnd),
            Some(Align::Start) => Some(gpui::AlignItems::FlexStart),
            Some(Align::Center) => Some(gpui::AlignItems::Center),
            _ => None,
        };
        if let Some(a) = self_align {
            let mut wrap = div().flex_shrink_0();
            wrap.style().align_self = Some(a);
            return Some(wrap.child(built).into_any_element());
        }
        return Some(built);
    }
    match e.tag.as_str() {
        "img" => {
            let mut copy = with_inherited_font(&pct_height_to_px(e, inherited), inherited);
            replaced_used_style::inline_percentage_width(&mut copy.style, AVAIL_W.get());
            // Держатель картинки строится из СЫРОГО стиля (`image_with` →
            // `styled_div`), а признак определённого блока (CSS 2.1 §10.5)
            // ставит только `inline::inherit`: без переноса `apply` выбрасывал
            // `height: %` у картинки во flex-элементе, абсолюте, по цепочке
            // долей, и она рисовалась природным размером
            // (`intrinsic-percent-replaced-024/026`: 200×200 вместо 100×100).
            // Ячейка исключена: её высоту считает табличная раскладка движка;
            // лунки — тоже свой путь (`row-auto-repeat-auto-023/024`: 0.32 →
            // 4.93 с переносом признака).
            if !matches!(inherited.display, Some(Display::TableCell) | Some(Display::GridLanes)) {
                copy.style.cb_height_def = inline::inherit(inherited, &e.style).cb_height_def;
            }
            // Единицы окна (`vw`/`vh`, в том числе внутри `calc`) — в точки: держатель
            // строится из СЫРОГО стиля, а сворачивает их только слитый
            // (`Computed::resolve_viewport`), и `height: calc(60vh - 6px)` у
            // картинки падал в природный размер (`intrinsic-percent-replaced-009-ref`).
            copy.style.resolve_viewport(opts.viewport);
            Some(image_with(&copy, Some(atom_base_font(inherited, opts))))
        }
        // Замещаемые с адресом в СВОЁМ атрибуте: у блочного пути такие рукава
        // есть, у строчного не было, и `<object data>` в строке терял
        // собственный размер (§10.3.2, §10.6.2) — коробки не заводил и уходил
        // прогоном запасного текста. Отсечка по атрибуту обязательна: объект
        // без `data` и видео без `poster` замещаемыми не являются.
        "embed" if e.attr("src").is_some() => Some(image_with(
            &with_inherited_font(e, inherited),
            Some(atom_base_font(inherited, opts)),
        )),
        // `<object>` с ДОКУМЕНТОМ в `data` (HTML §4.8.7: `type` text/html
        // или адрес .html/.htm/.xht/.xhtml) — вложенный контекст, как
        // `<iframe>`: та же коробка, тот же разбор, тот же размер (CSS сильнее
        // атрибутов, умолчание 300×150). Картинка в `data` идёт прежним путём.
        // Без файла (или глубже `IFRAME_DEPTH`) — `None`: кусок строится из
        // детей, то есть из запасного содержимого объекта (§4.8.7
        // «represents the element's children»), а не пустой коробкой.
        // Единицы шрифта разрешаются как у картинки (`image_with`): `iframe()`
        // читает только `Len::Px`, и `width: 10em` иначе падал бы в умолчание.
        "object" if e.attr("data").is_some() => {
            let mut copy = e.clone();
            let url = e.attr("data").unwrap_or_default().to_string();
            copy.attrs.push(("src".to_string(), url));
            if object_is_document(e) {
                copy.style.resolve_em(atom_base_font(inherited, opts));
                return iframe(&copy, opts);
            }
            Some(image_with(
                &with_inherited_font(&copy, inherited),
                Some(atom_base_font(inherited, opts)),
            ))
        }
        "video" if e.attr("poster").is_some() => {
            let mut copy = e.clone();
            let url = e.attr("poster").unwrap_or_default().to_string();
            copy.attrs.push(("src".to_string(), url));
            Some(image_with(
                &with_inherited_font(&copy, inherited),
                Some(atom_base_font(inherited, opts)),
            ))
        }
        "iframe" => Some(iframe(e, opts).unwrap_or_else(|| {
            replaced_content::empty_iframe(e, inherited, opts.viewport)
        })),
        "svg" => {
            // Рисунок без собственного размера — stretch-fit от содержащего
            // блока (`svg::stretch_fit`): ширина родителя, когда она в
            // точках, иначе ближайшая известная (`CB_WIDTH` — та же, что у
            // картинки с одним соотношением в `image_with`).
            let cb_w = match inherited.width {
                Some(Len::Px(v)) if v > 0.0 => Some(v),
                _ => CB_WIDTH.get().filter(|v| *v > 0.0),
            };
            // Размер в единицах шрифта (`svg { width: 10ch }`) решается тем же
            // шагом, что у картинки (`image_with` → `resolve_em`): сырой стиль
            // узла несёт `Len::Ch`, а `svg::size_of` понимает только точки —
            // рисунок молча падал в размер по `viewBox` (`ch-unit-001-ref`:
            // 150×150 вместо 10ch).
            let mut sized = with_inherited_font(e, inherited);
            sized.style.resolve_em(atom_base_font(inherited, opts));
            // CSS-коробка `<svg>` (рамка, отбивка) — `svg_replaced`; стиль
            // коробки — свой, с решёнными шрифтовыми единицами.
            let fitted = crate::svg::stretch_fit(&sized, cb_w);
            let shell = sized.style.clone();
            svg_replaced(&sized, &fitted, &shell).or_else(|| {
                Some(image_with(
                    &with_inherited_font(e, inherited),
                    Some(atom_base_font(inherited, opts)),
                ))
            })
        }
        // Свой бокс (фон, рамка, отступы) означает, что кусок не может быть
        // прогоном текста: прогон не умеет рисовать вокруг себя рамку.
        _ if has_own_box(
            &e.style,
            match inherited.font_size {
                Some(Len::Px(v)) => v,
                _ => 16.0,
            },
        )
            // Инлайн с СОБСТВЕННЫМ письмом, отличным от родителя, — по
            // css-writing-modes-4 §3.1 «its display computes instead to
            // inline-block»: коробка со своими размерами
            // (`different-block-flow-dir-001/002`).
            || (e.style.vertical.is_some() && e.style.vertical != inherited.vertical) =>
        {
            let mut merged = inline::inherit(inherited, &e.style);
            // CSS 2 sections 5.12.1-5.12.2 include inline-block containers,
            // but not ordinary inline boxes, in the pseudo-line scope.
            if e.style.display == Some(Display::InlineBlock)
                && e.style.inline_display != Some(true)
            {
                pseudo_line_layers::install(e, &mut merged);
            }
            let mut box_ = styled_div_with(e, &merged);
            // Строчная коробка БЕЗ содержимого всё равно высотой в строку:
            // рамка и фон рисуются по кеглю, а не по тексту. Без этого
            // `<span style="border-left:30px solid green">  </span>` выходил
            // нулевой высоты и не рисовался вовсе
            // (`line-edge-white-space-collapse-001`).
            // Обособленная блочная ось высоту уже задала (пусть нулевую) —
            // подставлять кегль строки поверх неё нельзя.
            // …и только у НАСТОЯЩЕГО строчного: у `inline-block` высота идёт от
            // содержимого, и подставленный кегль строки накрывал детей-блоков
            // (`border-left-width-applies-to-012`: квадрат 96 выходил 19).
            let genuine_inline = e.style.display.is_none() || e.style.inline_display == Some(true);
            if genuine_inline
                && merged.height.is_none()
                && !has_text(&e.children)
                && !merged.contains_height()
            {
                box_ = box_.h(px(line_height_px(&merged, opts)));
            }
            // `vertical-align` коробки в строке: верх/низ/середина СТРОКИ
            // (CSS 2.1 §10.8.1) — как у строчной таблицы выше. Без этого
            // `inline-block` с `vertical-align: top` сидел на базовой линии
            // и в высокой строке уезжал вниз.
            let self_align = match e.style.vertical_align {
                Some(Align::End) => Some(gpui::AlignItems::FlexEnd),
                Some(Align::Start) => Some(gpui::AlignItems::FlexStart),
                Some(Align::Center) => Some(gpui::AlignItems::Center),
                _ => None,
            };
            if let Some(a) = self_align {
                box_.style().align_self = Some(a);
            }
            // ★ ЗАМЕРЕНО, ЭФФЕКТА НЕТ (01.09): открывать здесь слой содержащего
            // блока, как это делает блочный путь (`cb_open`/`cb_close` ниже по
            // файлу), чтобы абсолютный ребёнок строчного `position: relative`
            // не уезжал к внешнему содержащему блоку. Срез из 229 пар:
            // 205 → 205 при гейте «только настоящая строчная коробка», и
            // 205 → 197 без него (`position-relative-table-{tbody,thead,tfoot,
            // tr}-*-absolute-child` уходили 0.00 → «красное видно»).
            // До `position-absolute-in-inline-*` правка НЕ доезжает: там у
            // строчного нет своей коробки, он идёт прогоном текста, и вешать
            // слой не на что — чинить надо в сборке прогонов.
            // `inline-block` с ВЕРТИКАЛЬНЫМ письмом — контейнер блоков со своей
            // осью блочного потока (css-writing-modes-4 §3.1): дети-блоки идут
            // колонками справа налево (`vertical-rl`) или слева направо
            // (`block-flow-direction-vrl-011`). Путь атома минует общий гейт в
            // `element()`, поэтому ось ставится здесь.
            let mut merged = merged;
            if e.style.display == Some(Display::InlineBlock)
                && merged.vertical == Some(true)
                && e.children.iter().any(|n| {
                    matches!(n, Node::Element(k)
                        if !k.inline || matches!(k.style.display, Some(Display::Block)))
                })
            {
                // Предел строчной оси детей — собственная высота коробки (как
                // `element()` сеет `ortho_limit` от высоты предка): без него
                // колонки тянулись за низ (`block-flow-direction-vrl-012`).
                let em_base = match merged.font_size {
                    Some(Len::Px(v)) => v,
                    _ => opts.base_size(),
                };
                let px_of = |l: Option<Len>| match l {
                    Some(Len::Px(v)) => Some(v),
                    Some(Len::Em(k)) => Some(k * em_base),
                    _ => None,
                };
                if let Some(h) = px_of(e.style.height) {
                    let b = e.style.borders();
                    let edges = if e.style.border_box == Some(true) {
                        px_of(b.top).unwrap_or(0.0)
                            + px_of(b.bottom).unwrap_or(0.0)
                            + px_of(e.style.padding.top).unwrap_or(0.0)
                            + px_of(e.style.padding.bottom).unwrap_or(0.0)
                    } else {
                        0.0
                    };
                    merged.ortho_limit = Some((h - edges).max(0.0));
                }
                box_ = box_.flex();
                box_ = if merged.vertical_rl == Some(true) {
                    box_.flex_row_reverse()
                } else {
                    box_.flex_row()
                };
            }
            // Маска, обрезка формой и фильтр действуют и на СТРОЧНУЮ коробку
            // (css-masking §1: `clip-path` применяется ко всем элементам):
            // блочный путь заворачивает её в буфер группы, а атомный шёл
            // мимо, и `clip-path` на `inline-block` не резал ничего
            // (`clip-path-contentBox-1d/1e`). Обёртка сама возвращает
            // элемент как есть, когда группировать нечего.
            Some(grouped(
                box_.children(blocks(&e.children, &merged, opts))
                    .into_any_element(),
                &e.style,
            ))
        }
        // `<ruby>` — контейнер руби (css-ruby-1 §2.1). Дети режутся на
        // сегменты и единицы (`ruby_segments`, §2.2-§2.3); база и её
        // аннотации стоят колонкой, колонки сегмента — рядом по базовой
        // линии, распорная аннотация (`<rtc>` без `<rt>`) накрывает весь
        // сегмент, уровни идут стопкой наружу (§3.1.2).
        //
        // Колонка для `over` собирается гибкой колонкой С ОБРАТНЫМ порядком:
        // первым ребёнком идёт БАЗА. Базовую линию гибкой колонки taffy берёт
        // у первого ребёнка (`flexbox.rs:404`), и атом встаёт на базовую
        // линию строки своей базой, а аннотация визуально ложится над ней.
        // Прежняя колонка «аннотация, база» отдавала строке базовую линию
        // АННОТАЦИИ, и база проваливалась под соседний текст
        // (`rbc-rtc-basic-001`). `ruby-position: under` — прямой порядок.
        //
        // Кегль, `nowrap` и `text-emphasis: none` аннотации даёт лист агента
        // (A.1: `rt, rtc { font-size: 50% }`), поэтому авторский
        // `rt { font-size }` теперь действует (`ruby-base-different-size`).
        // Коробки базы и аннотации — блочные: они тянутся на ширину колонки
        // (align-items: stretch), а содержимое выключается `text-align`
        // из `ruby-align` (§4.3); `items_center` абзац не центрировал.
        // Контейнер узнаётся по РОЛИ: тег `<ruby>` без своего `display` или
        // `display: ruby` на любом строчном (`ruby-box-model-001`:
        // `span.r`). `inline::collect` зовёт `atom` для КАЖДОГО элемента до
        // спуска в детей (`inline.rs:156`), так что сюда доходит и `<span>`.
        // Главная коробка `block ruby` сюда не попадает: она блок, а
        // строчный контейнер внутри неё — синтетический `<ruby>` (`dom.rs`).
        _ if ruby_role(e) == Some(crate::computed::RubyRole::Container) => {
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
        // `<canvas>` — замещаемый элемент с собственными размерами 300x150
        // по умолчанию (HTML §4.12.5); рисовать в нём нечего, но место он
        // занимает и фон несёт. Своего рукава у него не было, и голый
        // `<canvas>` с одним лишь фоном пропадал из строки целиком: коробки
        // фон не заводит (`has_own_box`), а строчный путь возвращал `None`.
        // Размеры уже проставлены разбором (`dom.rs`), здесь нужна коробка.
        // ЗАМЕРЕНО И ОТКАЧЕНО (04.09): не переносить атрибуты холста в
        // `style.width/height` (по HTML §15.3.10 холста в списке
        // «dimension attributes» нет — атрибуты дают ПРИРОДНЫЙ размер), а
        // ставить коробке природный размер и соотношение сторон здесь. Срез
        // из 169 пар с `<canvas>`: 54 -> 52, приобретено 2
        // (`replaced-alignment-with-aspect-ratio-002`,
        // `percent-height-replaced-in-percent-cell-003`), потеряно 4 —
        // `flexbox-flex-basis-content-001a/002a` 0.00 -> 1.9,
        // `contain-size-replaced-003a`, `replaced-content-spanner-auto-width`
        // в «красное видно». На заданный размер холста опираются
        // `flex-basis: content`, `contain: size` и спаннер многоколоночника;
        // возвращать вместе с ними.
        "canvas" => {
            // Доля высоты холста — в точки от содержащего блока, ровно как у
            // строчной картинки (`pct_height_to_px`): держатель стоит в
            // анонимном ряду строки с `height: auto`, и доля от ряда
            // схлопывала холст в ноль — флоат выходил нулевой ширины
            // (`intrinsic-percent-replaced-001`: 2.08). Точки берутся только
            // при высоте блока в `Px` и блочном `display` (оговорки там же).
            let converted = pct_height_to_px(&canvas_limit_keywords(e), inherited);
            let e = &converted;
            let merged = inline::inherit(inherited, &e.style);
            let d = styled_div_with(e, &merged).flex_shrink_0();
            // Перенос размера через соотношение сторон (css-sizing-4 §4.1)
            // у АТОМАРНОЙ строчной коробки срабатывает лишь тогда, когда
            // соотношение несёт ВНУТРЕННЯЯ коробка, заполняющая названную
            // автором ось: ровно так собран `<img>` (`image_with`,
            // `render.rs:13221-13225`), и ровно поэтому
            // `<img style="height:100%">` во флоате определённой высоты
            // выходит квадратом, а холст — полоской. Соотношение на самой
            // коробке этого не даёт (проба `target/probe/r4-canvas-ib.html`:
            // 2.08 против 0.00 у той же коробки с `display: block`).
            // Наполнитель ставится только при ОДНОЙ названной оси: при обеих
            // названных соотношение по спеке не действует вовсе
            // (css-sizing-4 §4.1, замечание про automatic size).
            // Под обособлением размера холст меряется как пустой
            // (css-contain-2 §size containment) — там наполнителя быть не
            // должно, иначе он вернул бы размер, который обособление сняло.
            let ratio = e
                .style
                .aspect_ratio
                .filter(|r| r.is_finite() && *r > 0.0 && !e.style.contains_width() && !e.style.contains_height());
            let auto = |l: Option<Len>| matches!(l, None | Some(Len::Auto));
            let named = |l: Option<Len>| matches!(l, Some(Len::Px(_)) | Some(Len::Pct(_)));
            // Доля ШИРИНЫ во вкладе в размер контейнера цикличная и считается
            // `auto` (css-sizing-3 §5.2.1: «treated for the purpose of
            // calculating the box's max-content contributions only as that
            // property's initial value»): ширину вклада даёт высота через
            // соотношение, а в раскладке держатель берёт свою долю
            // (`intrinsic-percent-replaced-019/031/032`: `width:100%;
            // height:100%` во flex-элементе высотой 100).
            let pct = |l: Option<Len>| matches!(l, Some(Len::Pct(_)));
            let d = match ratio {
                Some(r) if (auto(e.style.width) || pct(e.style.width)) && named(e.style.height) => {
                    let mut fill = div().h(gpui::relative(1.0));
                    fill.style().aspect_ratio = Some(r);
                    d.child(fill)
                }
                Some(r) if auto(e.style.height) && named(e.style.width) => {
                    let mut fill = div().w(gpui::relative(1.0));
                    fill.style().aspect_ratio = Some(r);
                    d.child(fill)
                }
                _ => d,
            };
            Some(d.into_any_element())
        }
        _ => None,
    }
}

/// Доли коробки по содержимому, решённые от содержащего блока (см. вызов в
/// `blocks`). `None` — решать нечего или блок не блочный: у гибкого и
/// сеточного родителя размеры приходят от раскладки.
pub(crate) fn pct_resolved_for_wrapper(e: &Element, inherited: &Computed) -> Option<Element> {
    if !content_sized_wraps(e) {
        return None;
    }
    pct_resolved_against_block(e, inherited)
}

/// Доли высоты и отступов, решённые от блочного содержащего блока с
/// известными сторонами. Нужны там, где между коробкой и её содержащим
/// блоком стоит наша служебная обёртка (сетка `content_sized`, строка
/// абзаца у `inline-block`), и раскладка решала бы долю от неё.
pub(crate) fn pct_resolved_against_block(e: &Element, inherited: &Computed) -> Option<Element> {
    if replaced_tag(e) || e.style.vertical == Some(true) {
        return None;
    }
    if !matches!(inherited.display, None | Some(Display::Block)) || inherited.vertical == Some(true)
    {
        return None;
    }
    let px_of = |l: Option<Len>| match l {
        Some(Len::Px(v)) => v,
        _ => 0.0,
    };
    // Опора высоты — СОДЕРЖИМОЕ родителя: при `box-sizing: border-box`
    // заданная высота включает его отступы и рамку.
    let base_h = match inherited.height {
        Some(Len::Px(h)) if inherited.border_box == Some(true) => {
            let b = inherited.borders();
            Some(
                h - px_of(inherited.padding.top)
                    - px_of(inherited.padding.bottom)
                    - px_of(b.top)
                    - px_of(b.bottom),
            )
        }
        Some(Len::Px(h)) => Some(h),
        // Высота из `aspect-ratio` при ширине в точках — определённая
        // (css-sizing-4 §5.1, тот же признак `ratio_height` в
        // `inline::inherit`): `.outer {width: 200px; aspect-ratio: 2/1}` даёт
        // доле детей 100 (`intrinsic-percent-replaced-015/016`). Только
        // `content-box`: соотношение тогда — у содержимого.
        None | Some(Len::Auto) if inherited.border_box != Some(true) => {
            match (inherited.aspect_ratio, inherited.width) {
                (Some(r), Some(Len::Px(w))) if r.is_finite() && r > 0.0 => Some(w / r),
                _ => None,
            }
        }
        _ => None,
    }
    .filter(|h| *h >= 0.0);
    let base_w = match inherited.width {
        Some(Len::Px(w)) if inherited.border_box == Some(true) => {
            let b = inherited.borders();
            Some(
                w - px_of(inherited.padding.left)
                    - px_of(inherited.padding.right)
                    - px_of(b.left)
                    - px_of(b.right),
            )
        }
        Some(Len::Px(w)) => Some(w),
        _ => None,
    }
    .filter(|w| *w >= 0.0);
    let of = |l: Option<Len>, base: Option<f32>| match (l, base) {
        (Some(Len::Pct(k)), Some(b)) => Some(Some(Len::Px(k * b))),
        _ => None,
    };
    let mut copy = e.clone();
    let mut changed = false;
    if let Some(v) = of(e.style.height, base_h) {
        copy.style.height = v;
        changed = true;
    }
    for (dst, src) in [
        (&mut copy.style.padding.top, e.style.padding.top),
        (&mut copy.style.padding.bottom, e.style.padding.bottom),
        (&mut copy.style.padding.left, e.style.padding.left),
        (&mut copy.style.padding.right, e.style.padding.right),
    ] {
        if let Some(v) = of(src, base_w) {
            *dst = v;
            changed = true;
        }
    }
    changed.then_some(copy)
}
