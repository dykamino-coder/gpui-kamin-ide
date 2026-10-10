//! CSS property coverage for unsupported: kept in registry order.

use super::{m, no, part};
use crate::coverage::Prop;

pub(super) const PROPERTIES: &[Prop] = &[
    // --- Перенести нечем -------------------------------------------------
    part(
        "transform",
        "rotate(45deg) scale(1.2) skewX(10deg)",
        "поворот, масштаб, сдвиг и скос точны; области попадания курсора остаются на исходном месте",
    ),
    m("transform-origin", "left top"),
    m("rotate", "45deg"),
    m("scale", "1.5"),
    m("translate", "10px 20px"),
    no(
        "perspective",
        "объёмных преобразований нет, а без них перспектива ничего не меняет",
    ),
    part(
        "transition",
        "0.2s ease",
        "плавно меняются цвета и прозрачность; отступы и размеры переключаются на середине пути",
    ),
    part(
        "animation",
        "pulse 2s infinite",
        "интерполируются цвет, прозрачность, размеры и сдвиг; кривые времени и задержка не разбираются",
    ),
    part(
        "filter",
        "grayscale(1) brightness(0.8) blur(4px)",
        "цветовые функции и размытие поддерева есть; `drop-shadow` и цветовые матрицы — нет",
    ),
    m("mix-blend-mode", "multiply"),
    m("clip-path", "polygon(50% 0%, 100% 100%, 0% 100%)"),
    part(
        "mask",
        "circle(50%)",
        "формы те же, что у `clip-path`; растровой маски из картинки нет",
    ),
    part(
        "float",
        "left",
        "текст обтекает блок сбоку и возвращается под него, когда у блока заданы размеры; без них остаётся ряд из двух колонок",
    ),
    part(
        "clear",
        "both",
        "прерывает ряд обтекания; настоящего сброса строк нет",
    ),
    m("columns", "200px 3"),
    m("column-count", "3"),
    m("column-width", "200px"),
    m("zoom", "2"),
    part(
        "writing-mode",
        "vertical-rl",
        "блок поворачивается на четверть оборота; `vertical-rl` и `vertical-lr` не различаются",
    ),
    part(
        "direction",
        "rtl",
        "разворачивает раскладку и прижим текста; перестановка смешанных прогонов — за движком шрифта",
    ),
    m("counter-reset", "item 0"),
    m("counter-increment", "item"),
    m("content", "\"→\""),
    m("user-select", "none"),
    part(
        "caret-color",
        "#ff0000",
        "каретка рисуется в поле с объявленным фокусом: своего фокуса у документа нет",
    ),
    m("accent-color", "#ff0000"),
    m("resize", "both"),
    m("text-shadow", "1px 1px 2px #000"),
    m("font-variant", "small-caps"),
    m("font-stretch", "condensed"),
    m("font-feature-settings", "\"tnum\" 1"),
    part(
        "hyphens",
        "auto",
        "мягкий перенос становится точкой разрыва, дефис на разрыве не рисуется; словарного переноса нет",
    ),
    m("background-repeat", "no-repeat"),
    m("background-position", "center"),
    m("background-size", "cover"),
    no(
        "background-attachment",
        "`fixed` привязывает фон к окну; ленты с таким фоном у нас нет",
    ),
];
