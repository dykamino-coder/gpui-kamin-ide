//! Строчные атомы: `atom_element`.
// owner: A

use crate::dom::Element;
use crate::layout::atom::own_box::own_box_atom;
use crate::layout::atom::positioned::{absolute_atom, static_position_atom};
use crate::layout::block::containing::{AVAIL_W, CB_WIDTH};
use crate::layout::multicol::spanner::multicol_container;
use crate::layout::positioned::predicates::has_own_box;
use crate::layout::positioned::static_position::at_static_position;
use crate::layout::replaced::iframe::{iframe, object_is_document};
use crate::layout::replaced::image::{image_with, pct_height_to_px};
use crate::layout::replaced::limits::{atom_base_font, canvas_limit_keywords, with_inherited_font};
use crate::layout::replaced::replaced_content::svg_replaced;
use crate::layout::replaced::{replaced_content, replaced_used_style, svg_percentage_size};
use crate::layout::table::table;
use crate::layout::writing_mode::rotated_atom;
use crate::paint::effects::transform::transformed;
use crate::render::{
    RenderOpts, content_sized, content_sized_wraps, element, replaced_tag, styled_div_with,
};
use crate::style::cascade::inherit::inherit;
use crate::style::computed::{Align, Computed, Display};
use crate::style::values::value::Len;
use crate::text::ruby::container::ruby_container_atom;
use crate::text::ruby::ruby_role;
use gpui::{AnyElement, IntoElement, ParentElement, Styled, div};

mod own_box;
mod positioned;

/// Не-текстовые инлайн-элементы, которые в поток встроить нельзя.
/// Строчный атом с размером по ключевому слову (`width: min-content` и
/// родня): дорожка по содержимому ставится той же обёрткой, что у блочного
/// пути (`content_sized`). Без неё атом шёл в ряд строки голым, гибкий ряд
/// брал его основу по max-content, и `inline-grid`/`inline grid-lanes` с
/// `width: min-content` раскладывался по max-content: доли `1fr 2fr 1fr 1fr`
/// при базах по 2ch раздавались, вторая дорожка выходила 4ch
/// (`grid-lanes-intrinsic-sizing-cols-002-fr`; css-sizing-3 §5.1).
pub(crate) fn atom_element(
    e: &Element,
    inherited: &Computed,
    opts: &RenderOpts,
) -> Option<AnyElement> {
    if let Some(physical) = rotated_atom::physical(e, inherited, opts) {
        return Some(physical);
    }
    let resolved = rotated_atom::resolved(e, inherited);
    let e = resolved.as_ref().unwrap_or(e);
    let el = atom_element_raw(e, inherited, opts)?;
    // CSS Sizing 3 §5.1: intrinsic keywords in the block axis behave as auto.
    let keyword = |l: Option<Len>| {
        matches!(
            l,
            Some(Len::MinContent) | Some(Len::MaxContent) | Some(Len::FitContent)
        )
    };
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
    Some(if wraps {
        content_sized(el, &e.style, &inherit(inherited, &e.style), (None, None))
    } else {
        el
    })
}

fn atom_element_raw(e: &Element, inherited: &Computed, opts: &RenderOpts) -> Option<AnyElement> {
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
        && crate::text::clamp::clamp_context().is_some())
    .then(crate::text::clamp::ClampGuard::enter_bfc);
    // Элементу формы нужен СЛИТЫЙ стиль: в своём у него единицы шрифта ещё не
    // разрешены (`width: 3ch` считался бы по базовому кеглю, а не по своему),
    // да и наследуемое до поля иначе не доходит.
    if let Some(el) = crate::interactive::forms::element(e, &inherit(inherited, &e.style), opts) {
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
        return static_position_atom(inherited, e, opts);
    }
    // Абсолютный элемент С заданными краями считается от ближайшего
    // позиционированного предка, а не от строки. Места в строке он не
    // занимает вовсе — потому и пустышка нулевая, и БЕЗ `relative`: иначе
    // содержащим блоком стала бы она сама, и края отсчитывались бы от неё.
    // Пока он был обычной коробкой куска, строка росла под его высоту.
    if matches!(
        e.style.position,
        Some(crate::style::computed::Position::Absolute)
            | Some(crate::style::computed::Position::Fixed)
    ) {
        return absolute_atom(inherited, e, opts);
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
        let merged = inherit(inherited, &e.style);
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
        let built = table(e, &inherit(inherited, &e.style), opts);
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
            if !matches!(
                inherited.display,
                Some(Display::TableCell) | Some(Display::GridLanes)
            ) {
                copy.style.cb_height_def = inherit(inherited, &e.style).cb_height_def;
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
        "iframe" => Some(
            iframe(e, opts)
                .unwrap_or_else(|| replaced_content::empty_iframe(e, inherited, opts.viewport)),
        ),
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
            let fitted = crate::svg::size::stretch_fit(&sized, cb_w);
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
            own_box_atom(inherited, e, opts)
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
        _ if ruby_role(e) == Some(crate::style::computed::RubyRole::Container) => {
            ruby_container_atom(inherited, e, opts)
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
            let merged = inherit(inherited, &e.style);
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
            let ratio = e.style.aspect_ratio.filter(|r| {
                r.is_finite() && *r > 0.0 && !e.style.contains_width() && !e.style.contains_height()
            });
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
pub(super) fn pct_resolved_against_block(e: &Element, inherited: &Computed) -> Option<Element> {
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
