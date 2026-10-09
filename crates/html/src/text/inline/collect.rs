//! Сборка кусков абзаца из узлов (collect_with_empty_metrics) и сдвиги наложений.

use crate::dom::{Element, Node};
use crate::style::cascade::inherit::inherit;
use crate::style::computed::Computed;
use crate::style::values::value::{Color, Len};
use crate::text::inline::*;
use gpui::{AnyElement, ParentElement, Styled};

pub(super) fn collect_with_empty_metrics(
    children: &[Node],
    inherited: &Computed,
    atom: &mut dyn FnMut(&Element) -> Option<Piece>,
    has_text: bool,
    case: &mut text_case::Context,
) -> Vec<Piece> {
    let mut out = vec![];
    // Место последней распорки зазора за коробкой (см. ниже).
    let mut gap_at: Option<usize> = None;
    for child in children {
        // Куски ЭТОГО ребёнка: по ним ставится зазор на границе с тем, что
        // идёт следом (см. `set_boundary_spacing` ниже).
        let from = out.len();
        match child {
            Node::Text(t) => {
                // Возврат каретки (U+000D) — ПРОБЕЛ при любом `white-space`
                // (css-text-3 §4.1: «carriage returns … are treated
                // identically to spaces»): разбор HTML сводит CR к LF только в
                // разметке, а `&#x0D;` доезжает знаком и при `pre*` рвал
                // строку (`control-chars-00D`).
                let cr_free;
                let t: &str = if t.contains('\r') {
                    cr_free = t.replace('\r', " ");
                    &cr_free
                } else {
                    t
                };
                // `white-space: pre*` сохраняет пробелы как есть: отступы кода
                // иначе схлопывались в один пробел и текст терял форму.
                let raw = if inherited.preserve_newlines == Some(true)
                    && inherited.keep_spaces != Some(false)
                {
                    // Табуляция остаётся СВОИМ знаком: её ширину считает
                    // раскладка строк по позициям табуляции. Разворот в
                    // пробелы менял и число точек переноса, и вид хвоста
                    // строки (`break-spaces-tab`).
                    t.to_string()
                } else if inherited.preserve_newlines == Some(true) {
                    // `pre-line`: пробелы схлопываются, переводы строк живут.
                    t.split('\n')
                        .map(normalize_spaces)
                        .collect::<Vec<_>>()
                        .join("\n")
                } else {
                    normalize_spaces(t)
                };
                // Замена нулевого пробеля идеографическим идёт ПО КУСКАМ
                // абзаца (`space_transform_pieces`): соседи точки переноса
                // сплошь и рядом лежат в других кусках, и проход по одному
                // узлу их не видит.
                let mut text = breakable(&text_case::transform(&raw, inherited, case), inherited);
                // То же правило нулевого пробела СКВОЗЬ границу строчной
                // коробки (css-text-4 §4.1.3; границ коробок для него нет —
                // `seg-break-transformation-018`): перевод строки в начале
                // узла, а нулевой пробел — последний знак предыдущего куска.
                if inherited.preserve_newlines != Some(true)
                    && leading_segment_break(t)
                    && ends_with_zwsp(&out)
                    && text.starts_with(' ')
                {
                    text.remove(0);
                }
                if !text.is_empty() {
                    out.push(Piece::Text {
                        text,
                        style: inherited.clone(),
                    });
                }
            }
            Node::Element(e) => {
                // `display: contents` on `<br>`/`<wbr>` behaves as
                // `display: none` (css-display-3 §B «Unusual Elements»): no
                // line break, no break opportunity
                // (`display-contents-sharing-001`).
                if matches!(e.tag.as_str(), "br" | "wbr")
                    && e.style.display == Some(crate::style::computed::Display::Contents)
                {
                    continue;
                }
                if e.tag == "br" {
                    case.boundary();
                    out.push(Piece::Text {
                        text: "\n".into(),
                        style: inherited.clone(),
                    });
                    continue;
                }
                // `<wbr>` — точка переноса без знака, то есть ровно нулевой
                // пробел (HTML §4.5.28). Раньше тег не давал НИЧЕГО, и
                // разрешённого переноса в этом месте не было.
                if e.tag == "wbr" {
                    // Замена точки переноса пробелом — по стилю САМОГО `<wbr>`
                    // (css-text-4 §word-space-transform: свойство наследуемое,
                    // и у элемента своё значение).
                    let mut style = inherited.clone();
                    style.word_space_char = e.style.word_space_char.or(style.word_space_char);
                    out.push(Piece::Text {
                        text: "\u{200b}".into(),
                        style,
                    });
                    continue;
                }
                // `display: contents` — коробки нет (css-display-3 §2.5 «does
                // not generate any boxes, but its children … still generate
                // boxes and text runs as normal»): ни рамки, ни полей, ни
                // отступов, ни знаков направления — детям уходят только
                // текстовые свойства наследованием. Блочный путь это знает
                // (`render.rs:4358`), а строчный сбор вёл такой элемент обычным
                // `<span>`, и рамка рисовалась прогоном
                // (`display-contents-inline-001` «красное видно»).
                if e.style.display == Some(crate::style::computed::Display::Contents) {
                    let merged = inherit(inherited, &e.style);
                    out.extend(collect_with_empty_metrics(
                        &e.children,
                        &merged,
                        atom,
                        has_text,
                        case,
                    ));
                    continue;
                }
                // Атому сообщают, лежит ли он в позиционированном строчном
                // (`take_atom_cb`); его собственное содержимое — уже вне его.
                let depth = INLINE_CB_DEPTH.with(|d| d.replace(0));
                ATOM_CB.with(|c| c.set(depth > 0));
                let built = atom(e);
                ATOM_CB.with(|c| c.set(false));
                INLINE_CB_DEPTH.with(|d| d.set(depth));
                if let Some(piece) = built {
                    if matches!(piece, Piece::Atom(_)) {
                        case.boundary();
                    }
                    out.push(piece);
                    // Строчный `<span>`, ушедший в свою коробку (узорный фон,
                    // `has_own_box`), атомом в CSS не является: зазор между его
                    // последним знаком и следующим — ПРЕДКА (css-text-3 §8.2),
                    // а внутренний абзац коробки последний знак не трекует.
                    // Без распорки следующее слово вставало вплотную
                    // (`letter-spacing-nesting-003`). Хвостовая распорка у
                    // конца узла снимается ниже — за ней границы нет.
                    if let Some(gap) = boundary_gap_after_box(e, inherited) {
                        gap_at = Some(out.len());
                        out.push(Piece::Text {
                            text: SPACER.into(),
                            style: spacer_style(inherited, gap),
                        });
                    }
                    continue;
                }
                let mut merged = inherit(inherited, &e.style);
                // Относительный сдвиг строчного КОПИТСЯ вниз: вложенные
                // куски двигаются на сумму сдвигов предков.
                let own_rel = relative_inset(&e.style);
                if own_rel != (0.0, 0.0) {
                    let base = merged.rel_shift.unwrap_or((0.0, 0.0));
                    merged.rel_shift = Some((base.0 + own_rel.0, base.1 + own_rel.1));
                }
                // Фон строчного бокса рисует прогон текста: коробки у него
                // нет, а фон обязан рваться на переносах вместе со строкой.
                // Берётся из СЛИТОГО стиля: отложенный цвет (`currentColor`,
                // относительная функция) решён только там.
                // `background-clip: text`: бокс фоном не красится — заливку
                // несёт цвет глифов (`inherit`), узорную показать нечем.
                if let Some(bg) = merged
                    .background
                    .filter(|_| merged.bg_clip != Some(crate::style::computed::BgClip::Text))
                {
                    merged.inline_bg = Some(bg);
                    // Единицы шрифта разрешаются так же, как в `inline_sides`:
                    // разбор только по точкам ронял `padding: 1em` в ноль, и
                    // подсветка строчной коробки шла впритык к тексту.
                    // Замерено по строчным семьям: приобретено 0, потеряно 0 —
                    // правка держится на своей правоте, а не на счёте.
                    //
                    // ПРОБОВАЛИ И ОТКАТИЛИ вместе с ней: раздувать `line_bounds`
                    // в `vendor/gpui/.../line.rs` на перелив полосы прогона,
                    // чтобы фон строки не попадал в один порядок с глифами
                    // соседней. Замерено там же: 0 и 0.
                    let size = match merged.font_size {
                        Some(crate::style::values::value::Len::Px(v)) => v,
                        _ => 16.0,
                    };
                    let family = merged.font_family.clone().unwrap_or_default();
                    let px_of = |l: Option<crate::style::values::value::Len>| match l {
                        Some(
                            crate::style::values::value::Len::Px(_)
                            | crate::style::values::value::Len::Em(_)
                            | crate::style::values::value::Len::Ch(_)
                            | crate::style::values::value::Len::Ex(_),
                        ) => crate::text::metrics::spacing_px(l, &family, size),
                        _ => 0.0,
                    };
                    merged.inline_pad = Some(physical_sides::project(
                        inherited,
                        [
                            px_of(e.style.padding.top),
                            px_of(e.style.padding.right),
                            px_of(e.style.padding.bottom),
                            px_of(e.style.padding.left),
                        ],
                    ));
                    merged.inline_radius = Some(px_of(e.style.radius.tl));
                }
                // Рамка строчной коробки рисуется прогоном: и ровная, и
                // с РАЗНЫМИ гранями (у прогона теперь пооосевые ширины) —
                // коробка рвала перенос, и span с рамкой уезжал столбиком.
                let font_px = match merged.font_size {
                    Some(Len::Px(v)) => v,
                    _ => 16.0,
                };
                let own_bg = merged
                    .background
                    .filter(|_| merged.bg_clip != Some(crate::style::computed::BgClip::Text))
                    .is_some();
                let mut own_border = true;
                if let Some((color, width)) = uniform_border(&e.style, font_px) {
                    merged.inline_border = Some((color, [width; 4]));
                } else if let Some(sided) = sided_border(&e.style, font_px) {
                    merged.inline_border =
                        Some((sided.0, physical_sides::project(inherited, sided.1)));
                } else {
                    own_border = false;
                }
                // CSS 2.1 §8.6 / css-break-3 §5.4: an inline box with a border
                // and no background still paints its border (and its padding
                // area) on each fragment. The text run band is the painter, and
                // `vendor/gpui` starts a band only from a background colour, so
                // such a box gets a fully transparent band colour that is unique
                // per box: bands of adjacent boxes stay separate, runs of its
                // descendants (which inherit it) continue the same band.
                let mut painted_bg = merged.inline_bg.is_some();
                if own_border && !own_bg && !painted_bg {
                    let id = BORDER_BAND.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    merged.inline_bg = Some(Color {
                        r: (id % 251) as f32 / 251.0,
                        g: ((id / 251) % 251) as f32 / 251.0,
                        b: 0.5,
                        a: 0.0,
                    });
                    let family = merged.font_family.clone().unwrap_or_default();
                    let px_of = |l: Option<crate::style::values::value::Len>| match l {
                        Some(
                            crate::style::values::value::Len::Px(_)
                            | crate::style::values::value::Len::Em(_)
                            | crate::style::values::value::Len::Ch(_)
                            | crate::style::values::value::Len::Ex(_),
                        ) => crate::text::metrics::spacing_px(l, &family, font_px),
                        _ => 0.0,
                    };
                    merged.inline_pad = Some(physical_sides::project(
                        inherited,
                        [
                            px_of(e.style.padding.top),
                            px_of(e.style.padding.right),
                            px_of(e.style.padding.bottom),
                            px_of(e.style.padding.left),
                        ],
                    ));
                    merged.inline_radius = Some(px_of(e.style.radius.tl));
                }
                // Контур строчного куска рисует тот же прогон: коробки у
                // куска нет, а место контур и не занимает. Рисуется только
                // ЗАДАННЫЙ видимый стиль (начальное `outline-style` — `none`),
                // толщина без записи — `medium`, цвет без своего — цвет текста
                // куска С НАСЛЕДОВАНИЕМ (`merged`, а не свой `e.style`), без
                // него — чёрный. Полосу прогона `paint_line_background`
                // заводит ТОЛЬКО от фона (`line.rs`, `current_background`),
                // поэтому контуру без фона даётся прозрачная подложка: иначе
                // кольцо не рисовалось вовсе (`outline-004` «красное видно»,
                // `outline-022` 0.83 = 100²−80²). ★ Не путать с откатом K2
                // (07.09, `line.rs`): там полоса заводилась от ЛЮБОЙ рамки, и
                // обычная рамка рисовалась дважды (`bidi-00*`); здесь — только
                // контур, второго рисовальщика у которого нет.
                if merged.inline_border.is_none()
                    && e.style.display.is_none()
                    && let Some(o) = e.style.outline
                    && o.style.is_some_and(|s| s != 0)
                    && let Some(width) = match o.width {
                        Some(Len::Px(w)) => Some(w),
                        None => Some(3.0),
                        _ => None,
                    }
                    && width > 0.0
                {
                    let color = o.color.or(merged.color).unwrap_or(Color {
                        r: 0.0,
                        g: 0.0,
                        b: 0.0,
                        a: 1.0,
                    });
                    merged.inline_border = Some((color, [width; 4]));
                    if merged.inline_bg.is_none() {
                        merged.inline_bg = Some(Color {
                            r: 0.0,
                            g: 0.0,
                            b: 0.0,
                            a: 0.0,
                        });
                        painted_bg = true;
                    }
                }
                // Атомарная строчная коробка — ГРАНИЦА переноса, даже когда
                // своей коробки в раскладке ей не завели (размер не задан, и
                // она осталась куском текста). По CSS строку можно рвать
                // между текстом и такой коробкой; у нас это выражается
                // нулевым пробелом по краям (`line-breaking-atomic-007`).
                // Настоящий `display: inline` сюда НЕ входит: разбор держит
                // его как `InlineBlock` с пометкой `inline_display`, и без
                // отсечки каждый `<span>` обрамлялся служебными нулевыми
                // пробелами — они рвали ряд схлопываемых пробелов, возвращали
                // краевой проход и добавляли лишнюю точку переноса. Та же
                // отсечка стоит в `has_own_box`.
                let atomic = matches!(
                    e.style.display,
                    Some(crate::style::computed::Display::InlineBlock)
                        | Some(crate::style::computed::Display::InlineFlex)
                        | Some(crate::style::computed::Display::InlineGrid)
                        | Some(crate::style::computed::Display::InlineTable)
                ) && e.style.inline_display != Some(true);
                // Ограничитель атомарной коробки — служебный знак, а не текст
                // документа: замена нулевого пробела идеографическим его
                // касаться не должна, иначе вокруг `inline-block` появляется
                // полноширинный пробел, которого в разметке нет.
                let mut marker = merged.clone();
                marker.word_space_char = None;
                if atomic {
                    out.push(Piece::Text {
                        text: "\u{200b}".into(),
                        style: marker.clone(),
                    });
                }
                // Боковые поля, рамки и отступы СТРОЧНОЙ коробки: своей
                // коробки в раскладке у неё нет, поэтому место занимает
                // невидимый знак-распорка. Соединитель слов (U+FEFF) выбран
                // не случайно: точкой переноса он не является, а нулевой
                // пробел ею был бы — строка рвалась бы по краю `<span>`.
                // CSS Writing Modes 4 §2.4, bidi-fragment-boxes: physical edges
                // follow the parent's direction, not the inline's own embedding.
                // Spacers sit outside the inline's bidi controls, in that parent.
                let ((mut mlead, mut mtrail), (mut lead, mut trail)) =
                    inline_spacing::inline_sides(e, &merged, inherited);
                if inherited.rtl == Some(true) {
                    std::mem::swap(&mut lead, &mut trail);
                    std::mem::swap(&mut mlead, &mut mtrail);
                }
                // CSS 2.1 sections 8.4 and 14.2: padding belongs to this
                // inline, even when a descendant supplies a different background.
                // Its advance is already carried by the edge spacers; paint those
                // advances instead of extending a descendant's text band into them.
                let painted_padding = merged.inline_bg.is_some() && merged.inline_border.is_none();
                if painted_padding && let Some(pad) = &mut merged.inline_pad {
                    pad[1] = 0.0;
                    pad[3] = 0.0;
                }
                // Box identity of the edge spacers: the line painter moves them
                // to the outermost visual fragments after bidi reordering.
                let parent_rtl = inherited.rtl == Some(true);
                let box_id = if mlead != 0.0 || lead != 0.0 || trail != 0.0 || mtrail != 0.0 {
                    SPACER_BOX.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
                } else {
                    0
                };
                let edge = |mut style: Computed, leading: bool| {
                    style.spacer_edge = Some((box_id, leading != parent_rtl, parent_rtl));
                    style
                };
                if mlead != 0.0 {
                    out.push(Piece::Text {
                        text: SPACER.into(),
                        style: edge(margin_spacer_style(&merged, inherited, mlead), true),
                    });
                }
                // ПУСТАЯ строчная коробка: прогона текста у неё нет, а
                // отступ, рамку и фон рисовать надо (§8.4). Распорка их не
                // красит (см. запись у `spacer_style`), поэтому коробка идёт
                // отдельным слоем — он места в строке не занимает (место
                // держат распорки) и высоту строки не меняет (§10.8: поля,
                // отступы и рамки строчного в неё не входят).
                let mut inner_text = String::new();
                crate::render::gather_text_public(&e.children, &mut inner_text);
                // Из одних СХЛОПЫВАЕМЫХ пробелов — тоже пустая: пробелы
                // схлопнутся в соседний или срежутся у края строки (§4.1.1,
                // §4.1.3), а рамка без фона прогоном не рисуется
                // (`line-edge-white-space-collapse-001/002`: зелёная рамка
                // `<span>  </span>` пропадала).
                // Коробка с элементами внутри (атомы, `<br>`) не пустая, даже
                // без текста: слой нарисовал бы её одной строкой поверх
                // настоящих фрагментов (`border-radius-012`).
                let only_text = e.children.iter().all(|n| matches!(n, Node::Text(_)));
                let blank = only_text
                    && (inner_text.is_empty()
                        || (merged.keep_spaces != Some(true)
                            && !painted_bg
                            && inner_text
                                .chars()
                                .all(|c| matches!(c, ' ' | '\t' | '\n' | '\r'))));
                if blank && (lead != 0.0 || trail != 0.0) {
                    let px_of = |l: Option<Len>| match l {
                        Some(Len::Px(v)) => v,
                        _ => 0.0,
                    };
                    let size = match merged.font_size {
                        Some(Len::Px(v)) => v,
                        _ => 16.0,
                    };
                    let bs = e.style.borders();
                    let padding = e.style.padding;
                    let flat = physical_sides::project(
                        inherited,
                        [
                            px_of(padding.top) + px_of(bs.top),
                            px_of(padding.right) + px_of(bs.right),
                            px_of(padding.bottom) + px_of(bs.bottom),
                            px_of(padding.left) + px_of(bs.left),
                        ],
                    );
                    let top = flat[0];
                    // Высота области содержимого — подъём плюс спуск шрифта, как
                    // у полосы непустого куска (`run_background_quad`); кегль
                    // вместо неё оставлял под пустой коробкой светлую черту
                    // рядом с полосой соседа (`word-spacing-characters-001`).
                    let family = merged.font_family.clone().unwrap_or_else(|| {
                        if merged.monospace == Some(true) {
                            crate::text::metrics::mono_family_for(merged.lang.as_deref())
                                .to_string()
                        } else {
                            String::new()
                        }
                    });
                    let (asc, desc, _) = crate::text::metrics::vmetrics_px(&family, size);
                    let content = if asc + desc > 0.0 { asc + desc } else { size };
                    let line = match merged.line_height {
                        Some(Len::Px(v)) => v,
                        Some(Len::Em(k)) => k * size,
                        _ => size * crate::text::metrics::normal_line(&family),
                    };
                    // Коробка стоит на области содержимого: она в середине
                    // строки, а полулидинг делит остаток поровну (§10.8).
                    let dy = ((line - content) / 2.0 - top).max(-line);
                    let mut copy = e.clone();
                    copy.style.width = Some(Len::Px(0.0));
                    copy.style.height = Some(Len::Px(content));
                    copy.style.margin = Default::default();
                    copy.style.position = None;
                    copy.style.display = None;
                    // Размер коробки читается из СЛИТОГО стиля: без этого
                    // заданные выше ширина и высота терялись, и коробка
                    // выходила нулевой высоты — рамка `border-left` и фон
                    // под отступом не рисовались вовсе.
                    let mut sized = merged.clone();
                    sized.width = copy.style.width;
                    sized.height = copy.style.height;
                    sized.margin = Default::default();
                    sized.position = None;
                    physical_sides::project_box(inherited, &mut copy.style);
                    physical_sides::project_box(inherited, &mut sized);
                    let boxel = crate::render::styled_div_with(&copy, &sized)
                        .absolute()
                        .top(gpui::px(dy));
                    out.push(Piece::Overlay(
                        boxel.into_any_element(),
                        OverlayAt::default(),
                    ));
                }
                if lead != 0.0 {
                    out.push(Piece::Text {
                        text: SPACER.into(),
                        style: edge(padding_spacer_style(&merged, lead, painted_padding), true),
                    });
                }
                if blank
                    && has_text
                    && !atomic
                    && lead == 0.0
                    && trail == 0.0
                    && empty_inline::different_metrics(&merged, inherited)
                {
                    // CSS 2.1 section 10.8: an empty inline box contributes
                    // its line height and font metrics even without glyphs.
                    out.push(Piece::Text {
                        text: SPACER.into(),
                        style: spacer_style(&merged, 0.0),
                    });
                }
                // Своя сторона письма у куска — это знаки управления по
                // Юникоду: разбор двунаправленности их и ждёт, а рисовать их
                // не надо, ширины у них нет.
                let (open, close) = bidi_marks(&e.style, &merged);
                // Zero-length markers bound the box content between its edge
                // spacers (`box_extents`); they add no text.
                let box_marker = |start: bool| Piece::Text {
                    text: String::new(),
                    style: Computed {
                        spacer_edge: Some((box_id, start, parent_rtl)),
                        ..merged.clone()
                    },
                };
                if box_id != 0 {
                    out.push(box_marker(true));
                }
                if let Some(mark) = open {
                    out.push(Piece::Text {
                        text: mark.to_string(),
                        style: merged.clone(),
                    });
                }
                // Относительный сдвиг строчного куска несёт и его потомков вне
                // потока: абсолютный элемент внутри `position: relative`
                // спана стоит от СДВИНУТОГО места (`static-position/htb-*`).
                let cb_here = establishes_cb(&e.style);
                if cb_here {
                    INLINE_CB_DEPTH.with(|d| d.set(d.get() + 1));
                }
                let kids = collect_with_empty_metrics(&e.children, &merged, atom, has_text, case);
                let kids = if cb_here {
                    INLINE_CB_DEPTH.with(|d| d.set(d.get().saturating_sub(1)));
                    mark_inline_cb(kids, e)
                } else {
                    kids
                };
                out.extend(shift_overlays(
                    kids,
                    &e.style,
                    merged.rotated_line == Some(true),
                ));
                if let Some(mark) = close {
                    out.push(Piece::Text {
                        text: mark.to_string(),
                        style: merged.clone(),
                    });
                }
                if box_id != 0 {
                    out.push(box_marker(false));
                }
                if trail != 0.0 {
                    out.push(Piece::Text {
                        text: SPACER.into(),
                        style: edge(padding_spacer_style(&merged, trail, painted_padding), false),
                    });
                }
                if mtrail != 0.0 {
                    out.push(Piece::Text {
                        text: SPACER.into(),
                        style: edge(margin_spacer_style(&merged, inherited, mtrail), false),
                    });
                }
                if atomic {
                    out.push(Piece::Text {
                        text: "\u{200b}".into(),
                        style: marker,
                    });
                }
                // Зазор между последним знаком этого элемента и первым знаком
                // того, что идёт следом, лежит ВНУТРИ текущего узла — значит,
                // и величина его отсюда (css-text-3 §8.2). Ставится всегда, а
                // снимается ниже с последнего куска: за ним границы уже нет.
                set_boundary_spacing(&mut out[from..], inherited.letter_spacing);
            }
        }
    }
    // За последним куском границы внутри ЭТОГО узла нет: зазор там задаст тот
    // предок, у которого дальше идёт своё содержимое. Он и поставит его на
    // этот же кусок, когда сборка вернётся к нему.
    set_boundary_spacing(&mut out, None);
    if gap_at.is_some_and(|at| at + 1 == out.len()) {
        out.pop();
    }
    out
}

/// Начинается ли текст узла пробельным рядом с переводом строки.
fn leading_segment_break(raw: &str) -> bool {
    raw.chars()
        .take_while(|c| is_collapsible(*c))
        .any(|c| matches!(c, '\n' | '\r'))
}

/// Последний ЗНАЧАЩИЙ знак собранных кусков — нулевой пробел. Распорки полей
/// и рамок, метки направления и куски вне потока — это границы коробок, а
/// для преобразования перевода строки их нет.
fn ends_with_zwsp(out: &[Piece]) -> bool {
    for p in out.iter().rev() {
        match p {
            Piece::Overlay(..) => continue,
            Piece::Atom(_) => return false,
            Piece::Text { text, .. } => {
                if text.is_empty() || text == SPACER || text.chars().all(bidi_format) {
                    continue;
                }
                return text.ends_with('\u{200b}');
            }
        }
    }
    false
}

/// Зазор предка за строчной коробкой, ушедшей в раскладку (см. `collect`).
/// Только НЕатомарный строчный элемент без замещения: у атома и картинки
/// межбуквенного интервала по краям Blink не ставит (интервал живёт в наборе
/// текста, `shape_result.cc` `ApplySpacing`).
fn boundary_gap_after_box(e: &Element, inherited: &Computed) -> Option<f32> {
    let inline_level = e.style.display.is_none() || e.style.inline_display == Some(true);
    let replaced = matches!(
        e.tag.as_str(),
        "img"
            | "svg"
            | "canvas"
            | "video"
            | "embed"
            | "object"
            | "iframe"
            | "input"
            | "button"
            | "select"
            | "textarea"
    );
    if !inline_level || replaced || e.style.ruby_role.is_some() {
        return None;
    }
    let size = match inherited.font_size {
        Some(Len::Px(v)) => v,
        _ => 16.0,
    };
    let gap = crate::text::metrics::spacing_px(
        inherited.letter_spacing,
        &inherited.font_family.clone().unwrap_or_default(),
        size,
    );
    (gap != 0.0).then_some(gap)
}

/// Зазор на границе элементов — на последний ЗНАЧАЩИЙ кусок набора.
///
/// Распорка строчной коробки пропускается: в её трекинге лежит поле коробки, а
/// не межбуквенный интервал, и перебивать его нельзя.
fn set_boundary_spacing(pieces: &mut [Piece], spacing: Option<Len>) {
    let last = pieces.iter_mut().rev().find_map(|p| match p {
        Piece::Text { text, style } if !text.is_empty() && text != SPACER => Some(style),
        _ => None,
    });
    if let Some(style) = last {
        style.letter_spacing_after = spacing;
    }
}

/// Кусок вне потока в РЯДУ: сам абзац его разместить не может (ряд собирает
/// раскладка), поэтому работает прежний обход — нулевая распорка на месте
/// куска и сам элемент в позднем слое, который рисуется от её угла.
pub(super) fn overlay_in_row(el: AnyElement) -> AnyElement {
    let spot: crate::layout::positioned::containing_block::SpotCell = Default::default();
    let probe = crate::layout::positioned::containing_block::spot_probe(spot.clone(), false);
    match crate::layout::positioned::containing_block::late_push(spot, el) {
        None => probe,
        Some(kept) => {
            let mut hole = gpui::div().relative().w_0().h_0().flex_shrink_0();
            hole.style().align_self = Some(gpui::AlignItems::FlexStart);
            hole.child(kept).into_any_element()
        }
    }
}

/// Относительный сдвиг коробки в точках: `left - right`, `top - bottom`
/// (CSS 2.1 §9.4.3). Нулевой, если элемент не относительный.
fn relative_inset(style: &Computed) -> (f32, f32) {
    if style.position != Some(crate::style::computed::Position::Relative) {
        return (0.0, 0.0);
    }
    let px_of = |l: Option<Len>| match l {
        Some(Len::Px(v)) => v,
        _ => 0.0,
    };
    (
        px_of(style.inset.left) - px_of(style.inset.right),
        px_of(style.inset.top) - px_of(style.inset.bottom),
    )
}

/// Сдвинуть куски вне потока на относительный сдвиг их строчного предка.
///
/// `rotated` — абзац повёрнутого вертикального письма. Там до-поворотная
/// ось y — БЛОЧНАЯ ось экрана, и сдвиг по ней обязан округляться так же, как
/// глифы соседнего текста: `paint_glyph` кладёт глиф `floor` от физической
/// точки, а раскладка ставит край отбивки `round` (`taffy.rs: layout_bounds`).
/// На 2px при масштабе 1.25 (2.5 точки) коробка вставала на точку дальше
/// текста, и столбец красного в одну точку проступал у всех `cb`-случаев
/// `css-position/static-position/v{lr,rl}-*`. Поэтому сдвиг по y не идёт
/// отбивкой, а копится в `OverlayAt::rot_dy` и прикладывается `lines.rs`
/// одним округлением вместе с местом в строке.
fn shift_overlays(pieces: Vec<Piece>, style: &Computed, rotated: bool) -> Vec<Piece> {
    if style.position != Some(crate::style::computed::Position::Relative) {
        return pieces;
    }
    let px_of = |l: Option<Len>| match l {
        Some(Len::Px(v)) => v,
        _ => 0.0,
    };
    let (dx, dy) = (
        px_of(style.inset.left) - px_of(style.inset.right),
        px_of(style.inset.top) - px_of(style.inset.bottom),
    );
    if dx == 0.0 && dy == 0.0 {
        return pieces;
    }
    pieces
        .into_iter()
        .map(|p| match p {
            // Абсолют от строчного содержащего блока: сдвигается сам блок, и
            // обёртка-отбивка встала бы его родителем (`lines.rs`).
            Piece::Overlay(el, at) if at.cb.is_some() => Piece::Overlay(
                el,
                OverlayAt {
                    cb: at.cb.map(|c| InlineCb {
                        shift: (c.shift.0 + dx, c.shift.1 + dy),
                        ..c
                    }),
                    ..at
                },
            ),
            Piece::Overlay(el, at) if rotated => Piece::Overlay(
                el,
                OverlayAt {
                    rot_dx: at.rot_dx + dx,
                    rot_dy: at.rot_dy + dy,
                    ..at
                },
            ),
            Piece::Overlay(el, at) => Piece::Overlay(
                gpui::div()
                    .pl(gpui::px(dx))
                    .pt(gpui::px(dy))
                    .child(el)
                    .into_any_element(),
                at,
            ),
            other => other,
        })
        .collect()
}
