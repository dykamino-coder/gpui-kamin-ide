//! Участие узла в inline-прогоне с учётом позиционирования и flex/grid.

use crate::dom::Node;
use crate::layout::positioned::static_position::{at_static_position, cb_padding_shifts_replaced};
use crate::render::*;
use crate::style::computed::{Computed, Display};
use crate::style::values::value::Len;
use crate::text::text_box::blank_text;

#[allow(clippy::too_many_arguments)]
pub(crate) fn inline_node(
    n: &Node,
    inherited: &Computed,
    ordered_context: bool,
    pending: &[Node],
) -> bool {
    match n {
        // Пробельный узел между инлайн-соседями — часть строки, а не
        // разрыв: `<button>A</button> <button>B</button>` в разметке с
        // переносами давал два абзаца, и кнопки вставали столбиком.
        // Под `white-space: pre*` пробельный узел — содержимое: узел из
        // одного перевода строки это ПУСТАЯ СТРОКА перед `</pre>`
        // (block-plaintext-006), отбрасывание съедало её высоту.
        Node::Text(t) => {
            inherited.preserve_newlines == Some(true)
                || !blank_text(t)
                || (!pending.is_empty() && t.contains(' '))
        }
        // Элемент с ЗАДАННЫМИ краями строчным не бывает: края он считает
        // от позиционированного предка, а не от строки. Куском абзаца он
        // получал содержащим блоком сам абзац — и `inset: 0` растягивал
        // его на одну строку вместо всей коробки родителя. На этом стоит
        // приём эталонов WPT: `::after` с `content: ""` и `inset: 0`
        // накрывает красное зелёным (`overflow-wrap-anywhere-001`).
        // Только когда заданы ОБЕ оси: у коробки с одним краем свободная
        // ось остаётся статической, а статическая позиция строчного — в
        // строке, не в блочном потоке. Такую коробку ведёт щуп в
        // `atom_element` (`x_set != y_set`).
        Node::Element(e)
            if matches!(
                e.style.position,
                Some(crate::style::computed::Position::Absolute)
                    | Some(crate::style::computed::Position::Fixed)
            ) && !at_static_position(&e.style)
                && {
                    let edge = |l: Option<Len>| !matches!(l, None | Some(Len::Auto));
                    (edge(e.style.inset.left) || edge(e.style.inset.right))
                        && (edge(e.style.inset.top) || edge(e.style.inset.bottom))
                }
                // Поле формы и заменяемый элемент строит СВОЙ путь
                // (`forms::element`, картинка), и краями он распоряжается
                // сам. Выведенный из строки, он терял свою коробку —
                // `<button>` с четырьмя краями переставал растягиваться
                // (`position-absolute-semi-replaced-stretch-button`).
                // Исключение снимается ровно там, где оно даёт НЕ ТОТ
                // прямоугольник: содержащий блок абсолюта — внутренний
                // край рамки родителя (§10.1 п.4.2), а куском строки
                // замещаемый считает край от содержимого. Признак —
                // `cb_padding_shifts_replaced`.
                && (!matches!(
                    e.tag.as_str(),
                    "input" | "textarea" | "select" | "button" | "img" | "svg" | "canvas"
                ) || cb_padding_shifts_replaced(e, inherited)) =>
        {
            false
        }
        // Абсолют строчного уровня (до блокификации — `inline-block` и
        // родня) с РОВНО ОДНОЙ заданной осью: свободная ось берётся от
        // гипотетической коробки при `position: static` (CSS 2.1 §10.3.7,
        // §10.6.4), а та стоит в строке, не под ней. Блокифицированный, он
        // уходил блочным ребёнком ниже абзаца, и `left: 0; top: auto`
        // вставал на следующую строку (`border-left-width-thin`: белая
        // заплатка под красным вместо поверх). Щуп строки ведёт такую
        // коробку в `atom_element` (`x_set != y_set`). Без строчного
        // содержимого ДО коробки строка пуста, и гипотетическая коробка
        // стоит в её начале — там же, где блочная статическая позиция;
        // такой абсолют остаётся прежним блочным путём
        // (`left-applies-to-012/014`: абсолют — единственный ребёнок).
        Node::Element(e)
            if e.style.abs_inline_level
                && !ordered_context
                && pending.iter().any(|p| match p {
                    Node::Text(t) => !t.trim().is_empty(),
                    Node::Element(x) => !matches!(
                        x.style.position,
                        Some(crate::style::computed::Position::Absolute)
                            | Some(crate::style::computed::Position::Fixed)
                    ),
                })
                && {
                    let edge = |l: Option<Len>| !matches!(l, None | Some(Len::Auto));
                    (edge(e.style.inset.left) || edge(e.style.inset.right))
                        != (edge(e.style.inset.top) || edge(e.style.inset.bottom))
                } =>
        {
            true
        }
        Node::Element(e) => match e.style.display {
            // Явно заявленная инлайновая коробка остаётся в строке даже у
            // блочного по природе тега — но НЕ внутри гибкого контейнера
            // или сетки: там каждый ребёнок сам себе элемент раскладки
            // («блокирование» из CSS). Иначе колонка из таких коробок
            // выкладывалась рядом: они склеивались в один абзац.
            Some(Display::InlineBlock)
            | Some(Display::InlineFlex)
            | Some(Display::InlineGrid)
            | Some(Display::InlineTable) => !ordered_context,
            // `display: inline grid-lanes` — такая же строчная коробка:
            // разбор держит её как `GridLanes` с пометкой `lanes_inline`,
            // и по css-display-3 внешний вид у неё `inline`. Эталоны
            // семьи `grid-lanes-intrinsic-sizing-*` написаны на
            // `display: inline-grid`, и без этой строки девять сеток
            // вставали столбиком вместо ряда.
            Some(Display::GridLanes) if e.style.lanes_inline => !ordered_context,
            // `display: contents` without block-level descendants: its
            // children are inline-level boxes and text runs of THIS
            // container (css-display-3 §2.5 «as if they replaced the
            // element»), so they join the surrounding inline run — the
            // inline collector dissolves the element (`inline.rs`,
            // `Display::Contents`). Flushing the run here split one line
            // `<div contents>abc</div><br>` into an anonymous block plus a
            // run starting with `<br>` — an extra empty line
            // (`text-autospace-elements-002`).
            Some(Display::Contents) => !ordered_context && !contains_block(&e.children),
            // Прежний откат этой строки СНЯТ (03.09). Он мерился, когда
            // строчный атом строил лунки голым `blocks()` и терял их
            // целиком — оттого вся восьмёрка `flow-tolerance-*` и уходила
            // в красное (0.00 -> 5.66 и родня). Теперь `atom_element`
            // отдаёт лунки блочному пути (`element()`), и обе правки
            // вместе дают по всему CSS3 2420 -> 2442: приобретено 27,
            // потеряно 5 (`row-line-names-007/008/010/012`,
            // `row-subgrid-abs-pos-002` — рядные лунки, они ждут обтяжку
            // по РЯДАМ, корень R4 из `target/scout-subgrid-orthogonal-
            // 2026-09.md`).
            // ★ ЗАМЕРЕНО И ОТКАЧЕНО (05.09): строчный путь для абсолюта
            // с объявленным `display: inline` на статической позиции
            // (css-position-3 §staticpos-rect). Срез 12086 пар вместе с
            // патчем барьера `contain`: 9343 -> 9346 (+9/-6), причём вся
            // шестёрка потерь — этого рукава:
            // `inline-level-absolute-in-block-level-context-002`
            // (0.26->0.52), `-007` (0.00->0.54), `-010` (0.00->1.04),
            // `position-absolute-dynamic-static-position-inline`
            // (0.00->2.10), `abs-pos-border-offset-003` (0.46->1.75),
            // `css-flexbox-height-animation-stretch` (0.10->1.90), против
            // всего двух приобретений (`-009`, `-012`). Строчная ветка
            // теряет полосу обтекания и рамочные смещения — рукав нужен
            // не здесь, а в `atom_element`.
            Some(_) => false,
            // Дети гибкого контейнера и сетки блокируются по CSS: каждый
            // сам себе элемент раскладки. Без оговорки `<span>` без
            // объявленного `display` оставался строчным, склеивался с
            // соседями в ОДИН абзац, и четыре элемента раскладки
            // превращались в один.
            //
            // Плавающий кусок строчным не бывает: `float` вынимает элемент
            // из строки и делает блоком (CSS 2.1 §9.7). Пока картинка с
            // `float: right` оставалась куском абзаца, до неё не доходило
            // поле родителя, и она вылезала за край страницы.
            // Перевод строки коробки не создаёт: в гибком контейнере и
            // сетке он остаётся ВНУТРИ безымянного элемента раскладки
            // вместе с соседним текстом, а не становится своим элементом
            // (`position-absolute-root-element-flex`: два предложения,
            // разделённые `<br><br>`, вставали бок о бок и переносились
            // раньше времени).
            None => {
                e.inline
                    && (!ordered_context || e.tag == "br")
                    && !e.style.float.is_some_and(|f| f != 0)
            }
        },
    }
}
