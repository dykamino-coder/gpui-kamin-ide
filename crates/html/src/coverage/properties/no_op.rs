//! CSS property coverage for no op: kept in registry order.

use super::{imp, m, no, part};
use crate::coverage::Prop;

pub(super) const PROPERTIES: &[Prop] = &[
    // --- Разбирается и ничего не делает ----------------------------------
    part(
        "will-change",
        "transform",
        "обещание даёт то же, что само свойство: содержащий блок для absolute/fixed и контекст наложения (css-will-change-1 §2.1); слоёв композитора нет",
    ),
    part(
        "contain",
        "paint",
        "`paint` и `strict` обрезают содержимое; `size` и `layout` на пересчёт          не влияют — он и так по узлу",
    ),
    part(
        "container-type",
        "size",
        "включает обособление размера и стиля, как велит css-conditional-5          §container-type; самого правила `@container` движок пока не разбирает",
    ),
    m("isolation", "isolate"),
    no(
        "appearance",
        "системного вида у элементов и так нет — рисуем сами",
    ),
    part(
        "box-decoration-break",
        "clone",
        "`clone` — у блочной коробки, прямого ребёнка многоколоночной стопки; у строчных коробок и на страницах — как `slice`",
    ),
    no(
        "font-smooth",
        "сглаживание задаёт растеризатор шрифта, не стиль",
    ),
    no("-webkit-font-smoothing", "то же самое: решает растеризатор"),
    m("tab-size", "4"),
    no(
        "scroll-behavior",
        "плавная прокрутка — свойство ленты, задаётся в коде",
    ),
    no("touch-action", "жестов касания в настольном окне нет"),
    no("overscroll-behavior", "цепочки прокрутки за край нет"),
    imp(
        "unicode-bidi",
        "перестановку внутри строки делает движок шрифта, и это умолчание          (`normal`); ни `isolate`, ни `bidi-override` конвейеру текста задать          нечем",
    ),
];
