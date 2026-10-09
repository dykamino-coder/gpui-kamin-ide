//! Сборка строчных кусков абзаца (paragraph_pieces_routed).

use crate::render::*;

pub(crate) fn paragraph_pieces_routed(
    nodes: &[Node],
    inherited: &Computed,
    opts: &RenderOpts,
    first_line_at: usize,
    first_line: &Computed,
    native_request: Option<&native_paragraph_route::Request<'_>>,
) -> AnyElement {
    // Бюджет строк знака обрыва (авто-режим) забирается РАЗОМ, до сборки
    // кусков: куски строят вложенные абзацы (`inline-block`, `<svg>`), и
    // чужой бюджет им доставаться не должен.
    let clamp_budget = crate::interact::take_para_budget();
    let clamp_tag = crate::interact::take_para_tag();
    // ★ ЗАМЕРЕНО И ОТКАЧЕНО: брать поперечное выравнивание ряда с самих
    // кусков, когда абзац своего не задал (`vertical-align: bottom` у
    // картинки). Ни это, ни `align-self` на самой картинке высоту строки не
    // меняют — строка всё равно выходит около 90 точек вместо 60, и картинка
    // просто прижимается к её низу. Дело не в выравнивании.
    // Текстовый путь абзаца — единственное место, где место куска вне потока
    // считается ДВУНАПРАВЛЕННО: строку режет и переставляет разбор UAX#9
    // внутри `lines.rs`, а `point_of` берёт продвижение от начала СВОЕЙ
    // строки. Ряд из слов (`inline.rs: as_wrapped_row`) о направлении не
    // знает вовсе — куски идут в ЛОГИЧЕСКОМ порядке, строка прижимается
    // целиком, и щуп садится в НАЧАЛО rtl-строки (замер: x = 328 при верных
    // 168, снимки `target/scoutrtlv/target/wpt-shots/`).
    //
    // Открыть текстовый путь при `direction: rtl` можно только абзацу, у
    // которого этот путь ДОСТУПЕН: на пустом тексте `inline::text_and_runs`
    // отдаёт `None` (`inline.rs:2543`), абзац всё равно сваливается в ряд, а
    // там кусок вне потока заворачивается в `inline.rs: overlay_in_row` со
    // СВОИМ, пустым `Spot` и теряет и `rtl`, и `next_line`. Ровно так устроена
    // семья `css-position/static-position/inline-level-absolute-in-block-
    // level-context-007..012` (rtl, абсолют `display: inline`, текста в абзаце
    // НЕТ, все шесть 0.00) — гейт оставляет её на прежнем пути.
    let flow_text = has_flow_text(nodes);
    let mut atom = |e: &Element| -> Option<inline::Piece> {
        let in_inline_cb = crate::inline::take_atom_cb();
        let svg_sized = svg_percentage_size::resolve(e, inherited);
        let e = svg_sized.as_ref().unwrap_or(e);
        // CSS 2.1 sections 10.3.8/10.6.5 use the replaced default size
        // before solving absolute insets; an empty frame is still replaced.
        let iframe_sized = (replaced_content::default_iframe(e)
            && matches!(
                e.style.position,
                Some(crate::computed::Position::Absolute | crate::computed::Position::Fixed)
            ))
            .then(|| replaced_content::empty_iframe_size(e, inherited, opts.viewport));
        let e = iframe_sized.as_ref().unwrap_or(e);
        // Абсолютный элемент на статической позиции ВНУТРИ строки — кусок вне
        // потока: место в строке он не занимает, поэтому абзац остаётся
        // текстовым и не теряет пробелы (`line-breaking-018`).
        // Вертикальное письмо сюда по-прежнему не пускается: там строчная ось
        // вертикальна, `point_of` отдаёт ДО-поворотные координаты, и класс
        // берётся отдельно (подкорень A3, три отката на `render.rs` «отдать
        // ось строки самому абзацу»).
        // ★ ЗАМЕРЕНО: без оговорки про вертикаль текстовый путь открывался
        // и ГОРИЗОНТАЛЬНОМУ rtl, и две зелёные пары уходили в «красное
        // видно» (`htb-rtl-ltr.tentative`, `htb-rtl-rtl` 0.16 -> 99.00) при
        // 24 приобретениях. Все приобретения — вертикальные
        // (`abs-pos-non-replaced-v{lr,rl}-*`), поэтому послабление
        // ограничено абзацем, чей содержащий блок пишет вертикально —
        // признак этого у нас `ortho_limit`, он ставится только внутри
        // вертикального содержащего блока (`element()`, `inline.rs:1012`).
        let plain_flow = inherited.vertical != Some(true)
            && (inherited.rtl != Some(true) || (flow_text && inherited.ortho_limit.is_some()));
        // Статическая позиция считается для ГИПОТЕТИЧЕСКОГО статического
        // элемента (§10.3.7: «if position had been static») — блокификация
        // `display: inline` под absolute на неё не влияет, метка
        // inline_display возвращает такой элемент в строчный путь.
        // БЛОЧНЫЙ абсолют в ПОВЁРНУТОМ абзаце вертикального письма — тоже
        // кусок вне потока, но с местом в начале СЛЕДУЮЩЕЙ строки
        // (`lines.rs: next_line_point`). Прежде он шёл атомом со щупом
        // `Spot`: атом выводил абзац из текстового пути в ряд слов, щуп
        // мерил до-поворотные координаты, а заместитель верхнего слоя
        // ложился горизонтальной полосой поперёк вертикального контейнера —
        // красное проступало у всех `css-position/static-position/v{lr,rl}-*`.
        // В повёрнутом абзаце коробка поворачивается вместе со строками, и её
        // до-поворотное место — ровно гипотетическая коробка §10.6.4.
        let rot_block = inherited.rotated_line == Some(true)
            && !inline_level(e)
            && e.style.inline_display != Some(true);
        if plain_flow
            && at_static_position(&e.style)
            && (inline_level(e) || e.style.inline_display == Some(true) || rot_block)
        {
            let mut merged = inline::inherit(inherited, &e.style);
            merged.position = None;
            merged.abs_static = true;
            // Замещаемый элемент строит своя ветка: дети `<svg>` — не блоки,
            // путь блоков давал пустую коробку (clip-path-ellipse-2-ref).
            // Картинка — тем же порядком: у неё детей нет вовсе, и путь
            // блоков давал пустую коробку, то есть абсолютная картинка без
            // краёв не рисовалась ВООБЩЕ (`clip-rect-v*`, проба
            // `probe/absimg2.html`).
            let inner = if e.tag == "svg" {
                let mut copy = e.clone();
                copy.style.image_orient_none = merged.image_orient_none;
                copy.style.position = None;
                crate::svg::element(&copy).unwrap_or_else(|| image(&copy))
            } else if e.tag == "img" {
                let mut copy = e.clone();
                copy.style.image_orient_none = merged.image_orient_none;
                copy.style.position = None;
                // `clip: rect(...)` и маска у картинки живут в буфере группы:
                // путь наложения идёт мимо `grouped`, и без обёртки картинка
                // рисовалась бы целиком (`clip-rect-v*`).
                grouped(image(&copy), &e.style)
            } else {
                // Тот же буфер группы, что и у картинки: маска, обрезка и
                // фильтр иначе не доходят до коробки на статической позиции.
                grouped(
                    styled_div_with(e, &merged)
                        .children(blocks(&e.children, &merged, opts))
                        .into_any_element(),
                    &e.style,
                )
            };
            // Сторона, которой коробка вешается на статическую точку. При
            // `direction: ltr` — левый край (текстовый путь так и кладёт,
            // `lines.rs: prepaint_at(origin)`), при `rtl` — ПРАВЫЙ:
            // css-position-3 §abs-non-replaced-width (строки 1035-1043,
            // перепись CSS 2.1 §10.3.7) — «…if the 'direction' property of the
            // element establishing the static-position containing block is
            // 'ltr' set 'left' to the static position …; otherwise, set
            // 'right' to the static-position». Точка у обеих сторон ОДНА И ТА
            // ЖЕ: замер по снимкам — ltr-двойники `-v{lr,rl}-{004,005,028,029,
            // 104,105,136,137}` ставят левый край ровно на 168.0 и все восемь
            // 0.00, а эталон rtl-пар требует 88.0..168.0, то есть ту же 168.0
            // правым краем.
            //
            // Blink разводит это на два шага: `geometry/static_position.h:86`
            // даёт `kInlineEnd` при `!IsLtr()`, а `absolute_utils.cc:27-37`
            // `GetStaticPositionInsetBias` переводит его в `InsetBias::kEnd`.
            // Сторону задаёт направление СОДЕРЖАЩЕГО блока, а не собственное
            // письмо коробки (css-writing-modes-4 §7.1, строки 1926-1931).
            let rotated_rtl = inherited.rtl == Some(true) && inherited.rotated_line == Some(true);
            let inner = if inherited.rtl == Some(true) && !rot_block && !rotated_rtl {
                crate::interact::InlineStartHang::new(inner).into_any_element()
            } else {
                inner
            };
            // Кусок кладётся КОРНЕМ в статическую точку (`lines.rs`), а корень
            // своих полей не кладёт: от статической позиции коробку отодвигает
            // её поле (CSS 2.1 §10.3.7, `margin-left` в уравнении ширины). Под
            // обёрткой коробка — обычный ребёнок, и поле на месте
            // (`CSS2/text/text-indent-013-ref`: `margin-left: -10em` — чёрная
            // полоса на 328 вместо 168 закрывала PASS).
            let nz = |l: Option<Len>| matches!(l, Some(Len::Px(v)) if v.abs() > 0.001);
            let inner = if nz(merged.margin.left) || nz(merged.margin.top) {
                div().flex().flex_row().items_start().child(inner).into_any_element()
            } else {
                inner
            };
            return Some(inline::Piece::Overlay(
                inner,
                inline::OverlayAt {
                    next_line: rot_block,
                    bidi_hang: rotated_rtl && !rot_block,
                    ..Default::default()
                },
            ));
        }
        // ★ ЗАМЕРЕНО И ОТКАЧЕНО: контр-поворот физических четвёрок краёв
        // (`margin`/`padding`/`border-width` и четвёрки цвета, стиля,
        // видимости) у КАЖДОГО куска повёрнутого абзаца — поворот уносит их
        // с собой, а стороны письмом не переставляются (§3.2). Направление
        // проверено обоими: по часовой (верх←право, право←низ, низ←лево,
        // лево←верх) заметно лучше обратного — девять пар `grid-self-
        // baseline-*` шли 8.38/5.83/16.62/21.74/5.13/3.98/1.52/4.22/6.88, по
        // часовой стало 7.98/5.83/13.78/12.65/5.02/4.39/1.52/6.11/6.88,
        // против часовой 8.31/5.83/16.61/12.99/5.26/6.09/1.52/7.81/6.88.
        // Зелёной не стала НИ ОДНА: их держит отсутствие физической высоты
        // (см. откат про `max_w` выше), а `wm-propagation-body-049` ушла
        // 0.00 → 2.24. Итог на срезе 571 пары: 423 → 422.
        // Возвращать вместе с высотой повёрнутого блока по содержимому.
        // Стоячая коробка в повёрнутом абзаце: `inline-block` с явным
        // ГОРИЗОНТАЛЬНЫМ письмом контр-поворачивается — его содержимое
        // обязано стоять прямо (эмуляция tcy в эталонах compression-*).
        if inherited.rotated_line == Some(true)
            && e.style.display == Some(Display::InlineBlock)
            && e.style.vertical == Some(false)
        {
            let mut merged = inline::inherit(inherited, &e.style);
            merged.rotated_line = None;
            let em = match merged.width {
                Some(Len::Px(v)) => v,
                _ => match merged.font_size {
                    Some(Len::Px(v)) => v,
                    _ => opts.base_size(),
                },
            };
            let inner = styled_div_with(e, &merged)
                .children(blocks(&e.children, &merged, opts))
                .into_any_element();
            return Some(inline::Piece::Atom(
                crate::interact::CombinedUpright::upright_box(inner, em).into_any_element(),
            ));
        }
        if let Some(piece) = combined_text::piece(e, inherited, opts) {
            return Some(piece);
        }
        // Остановленная анимация атома — запечённым кадром, как у блока в
        // `animated()`, но ВМЕСТЕ с `rotate`/`scale`/`transform`: матрицу
        // атома строит `transformed()` ниже от `e.style`. В `animated()` атомы
        // не заходят вовсе, и кадр `individual-transform-combine` (шесть
        // `inline-block` под `animation-delay: -500000s`) не доходил ни до
        // сдвига, ни до матрицы (css-transforms-1 §transformable-element).
        let frozen_atom;
        let e = match bake_frozen(e, true) {
            Some(b) => {
                frozen_atom = b;
                &frozen_atom
            }
            None => e,
        };
        let ruby_atom;
        let e = match ruby_transform::used(e) {
            Some(used) => {
                ruby_atom = used;
                &ruby_atom
            }
            None => e,
        };
        // Боковые поля атома с собственным прижимом несёт ОБЁРТКА: внутри
        // неё они сдвигают коробку, но в продвижение строки не входят —
        // следующий кусок наезжал на предыдущий ровно на его поле
        // (эталоны `fixed-table-layout-021..023`: `img{vertical-align:top}`
        // плюс `margin-left`).
        let wrapped = match e.style.vertical_align {
            Some(crate::computed::Align::Start)
            | Some(crate::computed::Align::End)
            | Some(crate::computed::Align::Center) => true,
            _ => false,
        };
        let original_margin = rotated_atom::margin(&e.style, inherited);
        let bare;
        let e = if wrapped {
            let mut copy = e.clone();
            copy.style.margin = Default::default();
            bare = copy;
            &bare
        } else {
            e
        };
        // An absolutely positioned box inside a rotated vertical paragraph is
        // blockified (CSS 2.1 §9.7) and inherits the vertical writing mode
        // (css-writing-modes-4 §2.1): it is its own vertical block, not a
        // piece of the horizontal pre-rotation paragraph, whose clone has
        // `vertical` cleared. Carry the writing mode on the box itself.
        let vertical_abs;
        let e = if (inherited.rotated_line == Some(true) || inherited.upright_stack)
            && e.style.vertical.is_none()
            && matches!(
                e.style.position,
                Some(crate::computed::Position::Absolute) | Some(crate::computed::Position::Fixed)
            )
            && !at_static_position(&e.style)
            && !replaced_tag(e)
        {
            let mut copy = e.clone();
            copy.style.vertical = Some(true);
            copy.style.vertical_rl = Some(inherited.vertical_rl == Some(true));
            copy.style.sideways = Some(inherited.sideways == Some(true));
            vertical_abs = copy;
            &vertical_abs
        } else {
            e
        };
        crate::inline::set_atom_cb(in_inline_cb);
        let built = atom_element(e, inherited, opts);
        crate::inline::set_atom_cb(false);
        let abs_cb = crate::inline::take_abs_cb();
        built.map(|el| {
            // `mix-blend-mode` на ЗАМЕЩАЕМОМ атоме строки: блочный путь,
            // атом с коробкой и флоат смешивают через `grouped`, а `<svg>`,
            // `<iframe>`, `<img>` в строке шли мимо, и режим пропадал
            // (`mix-blend-mode-svg`, `-iframe-parent`, `-iframe-sibling`:
            // красный квадрат вместо зелёного). Носитель несёт ТОЛЬКО режим:
            // маска и обрезка у атомов живут своими путями, полный стиль
            // применил бы их второй раз.
            let el = if replaced_tag(e) && e.style.blend.is_some_and(|b| b != 0) {
                let mut only = Computed::default();
                only.blend = e.style.blend;
                grouped(el, &only)
            } else {
                el
            };
            // Атомарный строчный — transformable element (css-transforms-1
            // §transformable-element: всё по блочной модели, «except for
            // non-replaced inline boxes»): `img`, `iframe`, `inline-block`,
            // `inline-table`, строчный `<svg>`. Обёртку получали только поля
            // форм — они и сейчас заворачиваются в `atom_element`, здесь их
            // пропускаем. Абсолюты через пустышку статической позиции не
            // трогаем: обёртка на нулевой пустышке взяла бы origin от нуля.
            let el = if matches!(
                e.tag.as_str(),
                "input" | "textarea" | "select" | "progress" | "meter"
            ) || matches!(
                e.style.position,
                Some(crate::computed::Position::Absolute) | Some(crate::computed::Position::Fixed)
            ) {
                el
            } else {
                transformed(el, &e.style, inherited)
            };
            // `vertical-align` НА САМОМ куске (`img { vertical-align: top }`):
            // ряд строит базовую линию, а кускам с top/middle/bottom нужен
            // собственный прижим (wm-propagation-body-033-ref: полоса-картинка
            // в строке с квадратом прижата к верху, у нас висела на базовой).
            // ПРОБОВАЛИ И ОТКАТИЛИ: выражать `text-top`/`text-bottom` у
            // атомарного куска прижимом к краю строки. Замерено по семьям
            // linebox/*, css1/*, *vertical*: 0 и 0 — этим парам нужен сдвиг
            // относительно ТЕКСТОВОЙ области родителя, а не край строки.
            use crate::computed::Align;
            let self_align = match e.style.vertical_align {
                Some(Align::Start) => Some(gpui::AlignItems::FlexStart),
                Some(Align::End) => Some(gpui::AlignItems::FlexEnd),
                Some(Align::Center) => Some(gpui::AlignItems::Center),
                _ => None,
            };
            let el = match self_align {
                Some(a) => {
                    let mut w = crate::apply::margins(div().flex_shrink_0(), &original_margin);
                    w.style().align_self = Some(a);
                    // Доля куска считается от его КОНТЕЙНЕРА, а обёртка встаёт
                    // между ним и рядом: без своей ширины она сжимается по
                    // содержимому, и `width: 100%` внутри разрешался в ноль —
                    // картинка пропадала целиком (`background-repeat-002-ref`:
                    // `img{vertical-align:top}` + `width="100%"`). Долю
                    // повторяем на обёртке, чтобы отсчёт остался прежним.
                    if let Some(Len::Pct(k)) = e.style.width {
                        w = w.w(gpui::relative(k));
                    }
                    if let Some(Len::Pct(k)) = e.style.height {
                        w = w.h(gpui::relative(k));
                    }
                    w.child(el).into_any_element()
                }
                None => el,
            };
            // Отрицательный `z-index` строчного замещаемого: краска уходит
            // ПОД содержимое до него (CSS 2.1 §9.9 шаг 3) — как у блочного
            // (background-size-document-root-vrl-*: красный маркер обязан
            // лечь под зелёный фон iframe).
            let el = if e.style.z_index.is_some_and(|z| z < 0)
                && e.style.position == Some(crate::computed::Position::Relative)
            {
                crate::interact::Underlay::new(el).into_any_element()
            } else {
                el
            };
            // Абсолютная коробка с заданными краями места в строке не
            // занимает — `atom_element` вернул пустышку нулевого размера.
            // Атомом её отдавать нельзя: атом уводит абзац с текстового пути
            // в ряд, и содержащим блоком абсолюта становится коробка ВСЕГО
            // абзаца, а §10.1 п.4 требует прямоугольник фрагментов строчного
            // предка. `Overlay` абзац с текстового пути не уводит.
            if matches!(
                e.style.position,
                Some(crate::computed::Position::Absolute) | Some(crate::computed::Position::Fixed)
            ) && !at_static_position(&e.style)
                && !matches!(
                    e.tag.as_str(),
                    "svg" | "img" | "canvas" | "video" | "embed" | "object" | "iframe"
                )
            {
                return inline::Piece::Overlay(
                    el,
                    inline::OverlayAt {
                        edges: abs_cb,
                        ..Default::default()
                    },
                );
            }
            inline::Piece::Atom(el)
        })
    };
    // Каждому атому — признак, можно ли поставить его В СТРОКУ абзаца
    // (`atom_line_align`): порядок записей совпадает с порядком `Piece::Atom`.
    // Руби несёт ещё и узлы своих аннотаций: по ним строка растёт
    // (`lines::ruby_extent`). Чужие узлы (руби вне строки внутри атома) атому
    // не достаются.
    let mut atom_aligns: Vec<Option<(crate::lines::AtomAlign, crate::lines::RubyExtents)>> =
        Vec::new();
    let mut atom_noted = |e: &Element| -> Option<inline::Piece> {
        let (piece, extents) = crate::lines::collect_ruby_extents(|| atom(e));
        if matches!(piece, Some(inline::Piece::Atom(_))) {
            let extents = if ruby_role(e) == Some(crate::computed::RubyRole::Container) {
                extents
            } else {
                crate::lines::RubyExtents::default()
            };
            // Абсолютная замещаемая — атом строки только РЯДОМ с текстом в
            // потоке: строка из одних внепоточных коробок нулевая (CSS 2.1
            // §9.4.2), а атом завёл бы ей струт (`clear-applies-to-001-ref`:
            // `<div>` с одной абсолютной картинкой вырастал на строку).
            let lone_abs = !flow_text
                && matches!(
                    e.style.position,
                    Some(crate::computed::Position::Absolute)
                        | Some(crate::computed::Position::Fixed)
                );
            atom_aligns.push(
                (!lone_abs)
                    .then(|| atom_line_align(e, inherited, opts))
                    .flatten()
                    .map(|a| (a, extents)),
            );
        }
        piece
    };
    let mut pieces = inline::collect(nodes, inherited, &mut atom_noted);
    if pieces.is_empty() {
        return div().into_any_element();
    }
    // Ряд пробелов через границу строчной коробки — один пробел (§16.6.1).
    // Проход идёт ПЕРВЫМ: всё дальше считает байтовые смещения по готовому
    // тексту кусков.
    inline::collapse_across_pieces(&mut pieces);
    // Точка переноса показывается знаком по СОСЕДЯМ, а они лежат в других
    // кусках — проход идёт по всему абзацу сразу.
    // Слогораздел идёт ПЕРВЫМ: он меняет сам текст кусков, а всё дальше
    // считает по готовому тексту байтовые смещения.
    inline::hyphenate_pieces(&mut pieces);
    inline::space_transform_pieces(&mut pieces);
    let mut pieces = pieces;
    // Пойдут ли атомы В СТРОКУ (решение то же, что ниже у распорок атомов):
    // тогда атом — содержимое строки, и край для среза пробелов он обрывает.
    let atoms_in_line = {
        let count = pieces
            .iter()
            .filter(|p| matches!(p, inline::Piece::Atom(_)))
            .count();
        count > 0
            && count == atom_aligns.len()
            && atom_aligns.iter().all(Option::is_some)
            && atoms_fit_line(
                inherited,
                atom_aligns
                    .iter()
                    .any(|a| a.as_ref().is_some_and(|(_, ex)| !ex.is_empty())),
            )
    };
    if atoms_in_line {
        inline::trim_edge_spaces_solid_atoms(&mut pieces);
    } else {
        inline::trim_edge_spaces(&mut pieces);
    }
    // Свой `unicode-bidi` у самого абзаца знаками не обрамлялся: их ставит
    // сборка КУСКОВ, а корень абзаца куском не бывает. Из-за этого
    // `bidi-override` на блоке не действовал вовсе (`pre-wrap-align-*-003`:
    // строки шли в исходном порядке вместо перевёрнутого).
    // Только ОТМЕНА и ИЗОЛЯЦИЯ: своё направление письма абзац и так знает —
    // оно уходит в основной уровень разбора двунаправленности.
    // Абзац без текста, из одних атомов (U+FFFC — нейтральные, UAX #9 N1/N2), от знаков
    // изоляции порядка не меняет, а знаки — текстовые куски — уводили её с
    // пути атомов: inline-block'и `dir=rtl`-блока (HTML `[dir] { unicode-bidi:
    // isolate }`) шли слева направо (`anchor-position-005`).
    let own_bidi = inherited.bidi_override == Some(true)
        || (inherited.bidi_isolate == Some(true)
            && pieces.iter().any(|p| {
                matches!(p, inline::Piece::Text { text, .. } if !text.trim().is_empty())
            }));
    let marks = if own_bidi {
        inline::bidi_marks(inherited, inherited)
    } else {
        (None, None)
    };
    if let (Some(open), Some(close)) = marks {
        pieces.insert(
            0,
            inline::Piece::Text {
                text: open.to_string(),
                style: inherited.clone(),
            },
        );
        pieces.push(inline::Piece::Text {
            text: close.to_string(),
            style: inherited.clone(),
        });
        // Жёсткий разрыв ЗАКАНЧИВАЕТ абзац разбора двунаправленности, и знак
        // отмены за ним уже не действует: его приходится ставить заново на
        // каждой строке (`pre-wrap-align-*-003`: перевёрнутой выходила только
        // первая строка).
        let again = format!("{close}\n{open}");
        for piece in pieces.iter_mut() {
            if let inline::Piece::Text { text, .. } = piece
                && text.contains('\n')
            {
                *text = text.replace('\n', &again);
            }
        }
    }
    // Буквица: первая буква абзаца — свой кусок со своим стилем. Кегль куска
    // доезжает до прогона (патч GPUI), поэтому она может быть крупнее строки.
    // Слой с `initial-letter` сюда не доходит: такая буквица уже ушла
    // плавающим узлом (`initial_letter_float`), и следующая буква абзаца не
    // должна стать второй буквицей с кеглем слоя.
    if let Some(first) = inherited
        .first_letter
        .as_deref()
        .filter(|f| f.initial_letter.is_none())
    {
        // Слой — копия стиля блока плюс объявления `::first-letter`
        // (`dom.rs` `layer()`): фон блока в нём чужой, букве его не красить —
        // тот же отсев, что в `initial_letter_float`.
        let mut first = first.clone();
        if first.background == inherited.background {
            first.background = None;
        }
        // Inherited values of the letter come from the element that holds
        // the letter, not from the block (Blink `FirstLetterPseudoElement::
        // StyleForFirstLetter`: parent style = the first letter text's
        // parent; css-pseudo-4 §first-letter-styling, the fictional tag sequence
        // sits inside the innermost element). Values the layer only copied
        // from the block must not override a nested element's own
        // (`display-contents-first-letter-002`: `<span>` color green).
        if first.color == inherited.color {
            first.color = None;
        }
        if first.font_family == inherited.font_family {
            first.font_family = None;
        }
        if first.font_weight == inherited.font_weight {
            first.font_weight = None;
        }
        if first.italic == inherited.italic {
            first.italic = None;
        }
        pieces = inline::split_first_letter(pieces, &first);
    }
    if first_line_at > 0 {
        pieces = inline::style_first_line(pieces, first_line_at, first_line);
    }
    // Межсловный интервал и отступ первой строки требуют строки из слов, а
    // единый текстовый блок их не умеет — поэтому решение принимается ДО
    // сборки блока, иначе оба свойства молча пропадали.
    let word = match inherited.word_spacing {
        Some(Len::Px(v)) => v,
        _ => 0.0,
    };
    // Отступ первой строки. Абсолютную часть разбор уже свёл к точкам, доля
    // же берётся от ширины содержащего блока и здесь ещё неизвестна — её
    // считает раскладка строк, когда ширина решена.
    // `calc(50% - 3px)` несёт обе части разом: `Paragraph` складывает
    // `px + pct × ширина строки` сам (`lines.rs`, css-text-3 §2.1).
    let mixed = match inherited.text_indent {
        Some(Len::Calc(i)) => crate::value::calc_get(i).pct_px(),
        _ => None,
    };
    let indent = crate::lines::Indent {
        px: match inherited.text_indent {
            Some(Len::Px(v)) => v,
            _ => mixed.map_or(0.0, |(_, px)| px),
        },
        pct: match inherited.text_indent {
            Some(Len::Pct(k)) => k,
            _ => mixed.map_or(0.0, |(pct, _)| pct),
        },
        each_line: inherited.text_indent_each_line == Some(true),
        hanging: inherited.text_indent_hanging == Some(true),
    };
    // Межсловный интервал считает своя раскладка строк: ряд из слов ломает
    // выключку, висящие пробелы и перенос. Ряд остаётся только под отступ
    // первой строки, который своей раскладке пока неизвестен.
    // Отступ первой строки умеет своя раскладка строк: она одна знает, где
    // строка кончается, и отрицательный отступ ей не помеха. Ряд из слов
    // остаётся запасным путём — на нём отступ становится распоркой, а она
    // отрицательной ширины не бывает.
    // Ряда из слов под отступ первой строки больше нет: `lines::rules` отдаёт
    // правила переноса ВСЕГДА, и своя раскладка строк умеет и отступ, и
    // отрицательный отступ.
    // Атомы — В СТРОКУ абзаца (CSS 2.1 §9.2.2, §10.8): каждый становится
    // распоркой (U+FEFF), её продвижение — ширина атома, а сам элемент
    // раскладывает и ставит на базовую линию своей строки `Paragraph`. Для
    // переноса распорка атома читается как U+FFFC (`Paragraph::linebreaks`):
    // так атом кладёт и Blink (`inline_items_builder.cc`, знак-заместитель
    // объекта), а класс CB даёт разрыв до и после (UAX #14 LB20).
    // Прежде абзац с атомом уходил в гибкий ряд слов (`as_wrapped_row`): одна
    // высота строки на ряд, базовая линия текста taffy не видна, `top`/
    // `bottom`/`text-top` не выражались.
    let mut line_atoms: Vec<(
        usize,
        AnyElement,
        crate::lines::AtomAlign,
        crate::lines::RubyExtents,
    )> = Vec::new();
    let atom_count = pieces
        .iter()
        .filter(|p| matches!(p, inline::Piece::Atom(_)))
        .count();
    if atoms_in_line && atom_count == atom_aligns.len() {
        let mut aligns = atom_aligns.into_iter().flatten();
        let mut at = 0usize;
        let mut out = Vec::with_capacity(pieces.len() + 2 * atom_count);
        let mut mark = inherited.clone();
        mark.word_space_char = None;
        mark.letter_spacing = Some(Len::Px(0.0));
        mark.inline_bg = None;
        mark.inline_border = None;
        for p in pieces {
            match p {
                inline::Piece::Atom(el) => {
                    let (align, extents) = aligns
                        .next()
                        .unwrap_or((crate::lines::AtomAlign::Shift(0.0), crate::lines::RubyExtents::default()));
                    line_atoms.push((at, el, align, extents));
                    out.push(inline::Piece::Text {
                        text: inline::SPACER.to_string(),
                        style: mark.clone(),
                    });
                    at += inline::SPACER.len();
                }
                inline::Piece::Text { text, style } => {
                    at += text.len();
                    out.push(inline::Piece::Text { text, style });
                }
                other => out.push(other),
            }
        }
        pieces = out;
    }
    let edges = edge_pieces(&pieces, inherited, opts);
    if inline::single_block(&pieces, opts.base_size())
        && let Some((text, runs)) = inline::text_and_runs(&pieces, &opts.text)
    {
        // `word-space-transform` смотрит на СОСЕДЕЙ точки переноса, а они
        // сплошь и рядом лежат в других кусках (`あ<wbr>い` — это три куска:
        // текст, точка, текст). По кускам преобразование их не видит, поэтому
        // идёт по собранному тексту абзаца. Замена знак-в-знак: нулевой
        // пробел и идеографический занимают в UTF-8 одинаково, поэтому
        // прогоны не съезжают.
        // Строка растёт под самый крупный кусок — как коробка строки в CSS.
        // Иначе крупный `<span>` вылезал бы на соседние строки.
        let biggest = inline::max_font_size(&pieces, own_size(inherited, opts), opts.base_size());
        // Прогон без своего кегля набирается кеглем АБЗАЦА, а им здесь стоит
        // самый крупный кусок: текст блока без объявленного `font-size` рядом
        // с крупным `<span>` вырастал до его кегля (`c43-rpl-ibx-000`: вся
        // строка в 3.75em). Свой кегль такого куска — кегль блока.
        let boxes = line_box_spans(&pieces, &edges, inherited, opts);
        let mut runs = runs;
        if (!line_atoms.is_empty() || !edges.is_empty() || boxes.is_some())
            && inherited.text_fit.is_none()
        {
            let own = own_size(inherited, opts);
            if biggest != own {
                for run in runs.iter_mut().filter(|r| r.font_size.is_none()) {
                    run.font_size = Some(gpui::px(own));
                }
            }
        }
        let mut opts = opts.clone();
        if biggest != opts.base_size() {
            opts.text.line_height = gpui::px(biggest * normal_fraction(inherited, &opts)).into();
        }
        let opts = &opts;
        // Selection controls handlers, while native text retains its layout and paint.
        let wrap_rules = crate::lines::rules(inherited);
        let native = wrap_rules.is_some() && native_request.is_some_and(|request| request.accepts(&pieces, !line_atoms.is_empty()));
        let selectable = inherited.no_select != Some(true) && inherited.pointer_events_none != Some(true);
        if !native && !selectable {
            return gpui::StyledText::new(SharedString::from(text))
                .with_runs(runs)
                .into_any_element();
        }
        // Native construction remains available without installing selection handlers.
        if let Some(wrap) = wrap_rules {
            let inherited = if native { native_request.unwrap().style } else { inherited };
            // Кегль абзаца — самый крупный кусок в нём: строка растёт под него,
            // и от него же считается высота строки в долях.
            //
            // ПРОБОВАЛИ И ОТКАТИЛИ: считать долю от кегля САМОГО блока
            // (струт §10.8), раз крупный кусок теперь растит строку каналом
            // `lh_spans`. Замерено: приобретено 4, потеряно 4 — три пары
            // `*-applies-to-008` уходят с 0.02 на 0.67. Возвращать вместе с
            // разбором `vertical-align: top/bottom` на тексте.
            // Куски у края строки в её струт не входят (§10.8.1): у
            // `vertical-align-121` строка из 30px текста и прижатого вверх
            // 60px куска — это 30px струта плюс вылет куска вниз, а не 60px
            // с текстом посередине.
            let (flow_biggest, flow_lh) = if edges.is_empty() {
                (biggest, 0.0)
            } else {
                flow_metrics(&pieces, &edges, inherited, opts)
            };
            let line = match inherited.line_height {
                Some(Len::Px(v)) => gpui::px(v),
                Some(Len::Pct(k)) => gpui::px(k * flow_biggest),
                Some(Len::Em(k)) => gpui::px(k * flow_biggest),
                _ if !edges.is_empty() => gpui::px(flow_lh),
                // Своей `line-height` у блока нет — её задают КУСКИ: у куска
                // со своей высотой строки она и берётся, у остальных доля от
                // кегля (§10.8.1). Канал `lh_spans` умеет строку только
                // растить, и объявленная `font: 100px/1` терялась.
                _ => gpui::px(inline::max_line_height(
                    &pieces,
                    own_size(inherited, opts),
                    opts.base_size(),
                    normal_fraction(inherited, opts),
                )),
            };
            // Построчные коробки (`line_box_spans`): высота строки абзаца — СТРУТ
            // блока, крупные куски растят только свои строки.
            let line = match &boxes {
                Some((_, strut)) => gpui::px(*strut),
                None => line,
            };
            let id = gpui::ElementId::Integer(text_id(&text));
            let family = inherited.font_family.clone().unwrap_or_default();
            let para = crate::lines::Paragraph::new(
                SharedString::from(text),
                runs,
                gpui::px(biggest),
                line,
                crate::lines::align_for(inherited),
                wrap,
            )
            // Preserve wrapping, direction and the sideways alphabetic baseline.
            .opaque_background(inherited)
            .reversed_lines(inherited.lines_reversed == Some(true))
            .vertical(inherited.para_vertical.is_some(), inherited.para_vertical == Some(true))
            .ortho_limit(inherited.ortho_limit.map(px))
            .vertical_central_baseline(inherited.sideways != Some(true) && inherited.text_sideways != Some(true))
            .rotated_central(
                inherited.rotated_line == Some(true)
                    && inherited.sideways != Some(true)
                    && inherited.text_sideways != Some(true),
            )
            .vertical_counter_clockwise(inherited.para_vertical == Some(false) && inherited.sideways == Some(true))
            .vertical_inline_constraint(inherited.orthogonal_inline, native_vertical::keyword(inherited))
            .plaintext(
                inherited
                    .bidi_plaintext
                    .unwrap_or(false)
                    .then(|| {
                        inherited
                            .text_align
                            .unwrap_or(crate::computed::TextAlign::Start)
                    })
                    .filter(|a| {
                        matches!(
                            a,
                            crate::computed::TextAlign::Start | crate::computed::TextAlign::End
                        )
                    }),
            )
            .spans(inline::wrap_spans(&pieces, inherited))
            .word_spans(inline::word_spans(&pieces, biggest))
            // Автозазоры идут ПЕРВЫМИ: поиск диапазона берёт первое
            // попадание, и зазор обязан перебить трекинг всего куска.
            .letter_spans(
                [
                    inline::autospace_spans(&pieces, biggest),
                    inline::letter_spans(&pieces, biggest),
                ]
                .concat(),
            )
            .shift_spans({
                let mut v = inline::shift_spans(&pieces, biggest, f32::from(line));
                v.retain(|(r, _)| !in_edge(&edges, r));
                v
            })
            .lh_spans({
                let mut v = inline::line_height_spans(
                    &pieces,
                    inherited,
                    biggest,
                    crate::metrics::normal_line(&inherited.font_family.clone().unwrap_or_default()),
                );
                v.retain(|(r, _)| !in_edge(&edges, r));
                v
            })
            // В повёрнутом абзаце руби в строку не идёт и строку не растит
            // (`atoms_fit_line`), а эталоны акцента сделаны из руби: рост
            // только у горизонтального (`text-emphasis-line-height-003*/004*`).
            .emph_spans(
                if inherited.rotated_line == Some(true) || inherited.vertical == Some(true) {
                    Vec::new()
                } else {
                    inline::emphasis_spans(
                        &pieces,
                        biggest,
                        crate::metrics::normal_line(&inherited.font_family.clone().unwrap_or_default()),
                    )
                },
            )
            // Линии украшений рисует сам абзац (css-text-decor-3 §2); у
            // повёрнутого — прежний путь набора.
            .decor_spans(inline::decor_spans(&pieces, &opts.text))
            .edge_spans(edges)
            .line_boxes(
                boxes.as_ref().map(|b| b.0.clone()).unwrap_or_default(),
                boxes.as_ref().map(|_| {
                    (
                        inline::strut_font(inherited, &opts.text),
                        gpui::px(own_size(inherited, opts)),
                    )
                }),
            )
            .rel_spans(inline::rel_spans(&pieces))
            .ruby_justify(inherited.ruby_justify == Some(true), inherited.ruby_unit)
            .justify_chars(inherited.justify_chars.unwrap_or(1))
            .align_last(
                inherited
                    .text_align_last
                    .map(|a| a.physical(inherited.rtl == Some(true)))
                    .map(crate::lines::align_of_value)
                    // `text-justify: none` forbids justification of the last
                    // line too: it aligns as `start` (css-text-3 §7.3,
                    // `text-justify-none-001` with `text-align-last: justify`).
                    .map(|a| match a {
                        crate::lines::Align::Justify if inherited.no_justify == Some(true) => {
                            if inherited.rtl == Some(true) {
                                crate::lines::Align::Right
                            } else {
                                crate::lines::Align::Left
                            }
                        }
                        other => other,
                    }),
            )
            .letter_spacing(gpui::px(crate::metrics::spacing_px(
                inherited.letter_spacing,
                &family,
                biggest,
            )))
            .word_spacing(gpui::px(crate::metrics::spacing_px(
                inherited.word_spacing,
                &family,
                biggest,
            )))
            .hanging(inherited.hanging)
            .indent(indent)
            .spacers(inline::spacers(&pieces))
            .spacer_edges(inline::spacer_edges(&pieces))
            .box_extents(inline::box_extents(&pieces))
            .flow_shapes(
                inherited
                    .flow_shapes
                    .clone()
                    .unwrap_or_else(|| std::sync::Arc::new((Vec::new(), Vec::new()))),
            )
            // Счётный режим (`line-clamp: <N>`) берёт предел из стиля;
            // авто-режим — из бюджета, посчитанного по точке среза.
            .line_clamp(clamp_budget.or_else(|| inherited.clamp_lines().map(|n| n as usize)))
            .clamp_marked(clamp_budget.is_some())
            .text_ellipsis(
                inherited.ellipsis == Some(true)
                    && inherited
                        .overflow_x
                        .is_some_and(|o| o != crate::computed::Overflow::Visible),
            )
            .overflow_marker(
                inherited.overflow_marker.clone(),
                Some(measure_font(inherited, opts)),
                Some(gpui::px(own_size(inherited, opts))),
            )
            .clamp_mark(inherited.clamp_mark.clone())
            .clamp_tag(clamp_tag)
            // Знак обрыва — анонимный строчный ребёнок блока: и
            // `visibility` у него блочная (css-overflow-3 §text-overflow,
            // css-overflow-4 §block-ellipsis): у скрытого блока знака не
            // видно, даже если кусок у среза `visible`
            // (`text-overflow-ellipsis-002`, `webkit-line-clamp-035`).
            .marker_color(Some(if inherited.hidden == Some(true) {
                gpui::transparent_black()
            } else {
                inherited
                    .color
                    .map(crate::value::Color::to_hsla)
                    .unwrap_or_else(gpui::black)
            }))
            .text_fit(inherited.text_fit)
            .fit_parts(
                // Масштабируемы только интервалы в ДОЛЯХ кегля; `px` и `em`
                // (от вычисленного кегля) подбор не трогает.
                [inherited.letter_spacing, inherited.word_spacing]
                    .iter()
                    .all(|l| matches!(l, None | Some(Len::Pct(_)))),
                matches!(
                    inherited.line_height,
                    Some(Len::Px(_)) | Some(Len::Em(_)) | Some(Len::Ex(_)) | Some(Len::Ch(_))
                ),
            )
            .hyphen_char(inherited.hyphen_char.clone())
            .tab_stops(inline::tab_stops(&pieces, inherited, &opts.text))
            .overlays(inline::overlays(pieces))
            .atoms(line_atoms)
            .ruby_trim(inherited.text_box_trim_start, inherited.text_box_trim_end);
            let para = if selectable {
                para.selectable(id, opts.selection_color())
            } else {
                para
            };
            if native {
                native_request.unwrap().built.set(true);
            }
            return para.into_any_element();
        }
    }
    // Ряд из слов и атом сходятся по базовой линии по РАЗНЫМ правилам: GPUI
    // базовой линии текста в taffy не отдаёт вовсе (`vendor/gpui`), и для
    // куска текста taffy берёт НИЖНИЙ край коробки
    // (`vendor/taffy/src/compute/flexbox.rs:1704`, `height + margin.bottom`),
    // а вложенный атом свою первую базовую линию пропагирует
    // (`flexbox.rs:405`). Кусок заявляет 1.0em, атом — 0.8em: атом тонет на
    // спуск шрифта, а короб строки растёт до 1.2em вместо `line-height`
    // (§10.8). Замерено зондом `target/probe/atom-probe2.html`: при
    // `font: 100px/1 Ahem` короб 120, атом на +20.
    let has_atom = pieces.iter().any(|p| matches!(p, inline::Piece::Atom(_)));
    let em_base = opts.base_size();
    let mut render_text = |t: String, style: &Computed| -> AnyElement {
        // Стоячие знаки в вертикальном письме (`text-orientation: mixed`,
        // CJK): набор идёт вертикальными формами шрифта — возможность `vert`
        // подставляет глиф, а продвижение берётся из его вертикальных метрик
        // (css-writing-modes-3 §7.3, реализация в DirectWrite-слое). Пока
        // только для кусков, стоячих ЦЕЛИКОМ: смешанный кусок потребовал бы
        // резки на прогоны по ориентации.
        let vert_style;
        let style = if style.rotated_line == Some(true)
            && style.sideways != Some(true)
            && t.chars().any(upright_in_mixed)
            && t.chars().all(|c| c.is_whitespace() || upright_in_mixed(c))
        {
            let mut s = style.clone();
            s.font_features.push(("vert".into(), 1));
            vert_style = s;
            &vert_style
        } else {
            style
        };
        // На кусок текста идут ТОЛЬКО текстовые свойства: фон, отступы и
        // рамка принадлежат абзацу целиком, а не каждому его слову.
        let d = apply(div(), &style.text_only())
            .max_w_full()
            .child(SharedString::from(t.clone()));
        d.into_any_element()
    };
    // Начальное значение `text-align` — `start`, а он при письме справа налево
    // означает ПРАВЫЙ край. Без этого ряд из слов оставался слева, и строка
    // расходилась с абзацем-соседом.
    let align = inherited
        .text_align
        .unwrap_or(crate::computed::TextAlign::Start)
        .physical(inherited.rtl == Some(true));
    inline::as_wrapped_row(
        pieces,
        inherited.vertical_align,
        Some(align),
        inherited.rtl == Some(true),
        match inherited.text_indent {
            Some(Len::Px(v)) => v,
            // Ряд из слов долю и прежде не применял (`Pct` идёт нулём); у
            // смеси берутся хотя бы точки — как у чистых точек.
            Some(Len::Calc(i)) => crate::value::calc_get(i).pct_px().map_or(0.0, |(_, px)| px),
            _ => 0.0,
        },
        inherited.nowrap == Some(true),
        &mut render_text,
        inherited.vertical != Some(true) && inherited.rotated_line != Some(true),
    )
}
