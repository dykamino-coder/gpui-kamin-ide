//! Атом по тегу и роли (atom_element_raw): формы, контейнеры, замещаемые, ruby.

use super::replaced::{canvas_atom, container_atom, img_atom, svg_atom};
use crate::dom::Element;
use crate::layout::atom::own_box::own_box_atom;
use crate::layout::atom::positioned::{absolute_atom, static_position_atom};
use crate::layout::build_contracts::Effect;
use crate::layout::options::RenderOpts;
use crate::layout::positioned::predicates::has_own_box;
use crate::layout::positioned::static_position::at_static_position;
use crate::layout::replaced::iframe::{iframe, object_is_document};
use crate::layout::replaced::limits::{atom_base_font, with_inherited_font};
use crate::layout::replaced::{replaced_content, svg_percentage_size};
use crate::style::cascade::materialize::box_style;
use crate::style::computed::{Computed, Display};
use crate::style::values::value::Len;
use crate::text::ruby::container::ruby_container_atom;
use crate::text::ruby::ruby_role;
use gpui::AnyElement;

pub(super) fn atom_element_raw(
    e: &Element,
    inherited: &Computed,
    opts: &RenderOpts,
) -> Option<AnyElement> {
    let vcx = opts.cx.vcx();
    let svg_sized = svg_percentage_size::resolve(e, inherited);
    let e = svg_sized.as_ref().unwrap_or(e);
    let media_sized =
        crate::layout::replaced::media::resolve(e, inherited, opts.viewport, opts.cx.avail_w, vcx);
    let e = media_sized.as_ref().unwrap_or(e);
    // Строчный атом — независимый контекст форматирования: строки внутри
    // `inline-block` в бюджет `line-clamp` не входят (css-overflow-4 §5.3),
    // иначе строка атома считалась ВТОРОЙ поверх строки абзаца
    // (`line-clamp-032`). Содержимое атома строится ниже голым `blocks()`,
    // мимо сторожа общего пути. `display: inline` после каскада — это
    // `InlineBlock` с `inline_display` — атомом не является.
    let clamp_bfc = matches!(
        e.style.display,
        Some(Display::InlineBlock)
            | Some(Display::InlineFlex)
            | Some(Display::InlineGrid)
            | Some(Display::InlineTable)
    ) && e.style.inline_display != Some(true);
    let opts = &*opts.inner(|cx| {
        if clamp_bfc {
            cx.clamp_build = cx.clamp_build.bfc();
        }
    });
    // Элементу формы нужен СЛИТЫЙ стиль: в своём у него единицы шрифта ещё не
    // разрешены (`width: 3ch` считался бы по базовому кеглю, а не по своему),
    // да и наследуемое до поля иначе не доходит.
    if let Some(el) =
        crate::interactive::forms::element(e, &box_style(inherited, &e.style, vcx), opts)
    {
        // Трансформы поля формы шли МИМО обёртки: инпуты стояли ровно, а
        // эталон сдвигал (transform-input-001..019).
        return Some(
            opts.cx
                .build
                .effects(el, Effect::transform(&e.style, inherited, vcx)),
        );
    }
    // Абсолютный элемент без заданных краёв стоит на СТАТИЧЕСКОЙ позиции — там,
    // где он оказался бы в потоке. Внутри строки это место знает только сама
    // строка, поэтому в неё встаёт пустышка нулевого размера, а элемент висит
    // от её угла. Без этого раскладка ставила его в угол ближайшего
    // позиционированного предка, и текст уезжал в начало абзаца.
    // Не пускать сюда коробку с ОДНИМ заданным краем, хотя §10.6.4 решает оси
    // независимо и `left: 0` не должен отменять статический `top: auto`
    // (`abspos-009`): у замещаемых пустышка нулевая, и размер по свободной
    // оси перестаёт считаться от содержащего блока
    // (`absolute-replaced-height-*`, `absolute-replaced-width-*`).
    // Вернуть можно только вместе с ненулевой распоркой по занятой оси.
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
    if let Some(math) = crate::layout::mathml::element(e, inherited, opts) {
        return Some(opts.cx.build.effects(math, Effect::group(&e.style, vcx)));
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
    // Эталоны `column-grid-lanes-container-baseline-*` одной шириной не
    // берутся: их держит ещё и выравнивание по базовой линии.
    // Лунки сетки — та же история: раскладку лунок строит только блочный
    // путь, а строчный отдавал голый `blocks()`, и `display: inline
    // grid-lanes` терял лунки целиком.
    if let Some(value) = container_atom(inherited, opts, e) {
        return value;
    }
    let built = match e.tag.as_str() {
        "img" => img_atom(inherited, opts, e),
        // Замещаемые с адресом в СВОЁМ атрибуте: у блочного пути такие рукава
        // есть, у строчного не было, и `<object data>` в строке терял
        // собственный размер (§10.3.2, §10.6.2) — коробки не заводил и уходил
        // прогоном запасного текста. Отсечка по атрибуту обязательна: объект
        // без `data` и видео без `poster` замещаемыми не являются.
        "embed" if e.attr("src").is_some() => Some(opts.cx.build.replaced(
            &with_inherited_font(e, inherited),
            Some(atom_base_font(inherited, opts)),
            &opts.cx,
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
                copy.style.resolve_em(atom_base_font(inherited, opts), vcx);
                return iframe(&copy, inherited, opts);
            }
            Some(opts.cx.build.replaced(
                &with_inherited_font(&copy, inherited),
                Some(atom_base_font(inherited, opts)),
                &opts.cx,
            ))
        }
        "video" if e.attr("poster").is_some() => {
            let mut copy = e.clone();
            let url = e.attr("poster").unwrap_or_default().to_string();
            copy.attrs.push(("src".to_string(), url));
            Some(opts.cx.build.replaced(
                &with_inherited_font(&copy, inherited),
                Some(atom_base_font(inherited, opts)),
                &opts.cx,
            ))
        }
        "iframe" => Some(
            iframe(e, inherited, opts)
                .unwrap_or_else(|| replaced_content::empty_iframe(e, inherited, opts)),
        ),
        _ if crate::layout::replaced::media::empty(e) => {
            Some(crate::layout::replaced::media::render(e, opts))
        }
        "svg" => svg_atom(inherited, opts, e),
        // Свой бокс (фон, рамка, отступы) означает, что кусок не может быть
        // прогоном текста: прогон не умеет рисовать вокруг себя рамку.
        _ if has_own_box(
            &e.style,
            match inherited.font_size {
                Some(Len::Px(v)) => v,
                _ => 16.0,
            },
         opts.cx.vcx())
            // Инлайн с СОБСТВЕННЫМ письмом, отличным от родителя, — по
            // css-writing-modes-4 §3.1 «its display computes instead to
            // inline-block»: коробка со своими размерами
            // (`different-block-flow-dir-001/002`).
            || (e.style.writing_mode.is_some() && e.style.vertical_decl() != inherited.vertical_decl()) =>
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
        // спуска в детей, так что сюда доходит и `<span>`.
        // Главная коробка `block ruby` сюда не попадает: она блок, а
        // строчный контейнер внутри неё — синтетический `<ruby>` (`dom::fixup_tree`).
        _ if ruby_role(e) == Some(crate::style::computed::RubyRole::Container) => {
            ruby_container_atom(inherited, e, opts)
        }
        // `<canvas>` — замещаемый элемент с собственными размерами 300x150
        // по умолчанию (HTML §4.12.5); рисовать в нём нечего, но место он
        // занимает и фон несёт. Своего рукава у него не было, и голый
        // `<canvas>` с одним лишь фоном пропадал из строки целиком: коробки
        // фон не заводит (`has_own_box`), а строчный путь возвращал `None`.
        // Размеры уже проставлены разбором (`dom`), здесь нужна коробка.
        // Не переводить атрибуты холста из `style.width/height` в природный
        // размер коробки здесь, хотя по HTML §15.3.10 холст не в списке
        // «dimension attributes» и атрибуты дают ПРИРОДНЫЙ размер: на заданный
        // размер холста опираются `flex-basis: content`, `contain: size` и
        // спаннер многоколонника (`flexbox-flex-basis-content-001a/002a`,
        // `contain-size-replaced-003a`, `replaced-content-spanner-auto-width`);
        // менять можно только вместе с ними.
        "canvas" => canvas_atom(inherited, e, &opts.cx),
        _ => None,
    };
    // CSS Borders 4 shapes replaced content together with its CSS box.
    built.map(|el| {
        if crate::layout::box_kind::replaced_tag(e) && e.style.corner_shaped() {
            opts.cx.build.effects(
                el,
                Effect::GroupReplaced {
                    style: &e.style,
                    vcx,
                },
            )
        } else {
            el
        }
    })
}
