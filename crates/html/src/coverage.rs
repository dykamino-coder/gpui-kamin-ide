//! Реестр покрытия CSS: одна таблица, по которой считается процент.
//!
//! Зачем отдельный файл, а не список в документации: список в документации
//! устаревает молча. Здесь каждое свойство обязано быть классифицировано, а
//! тест проверяет, что помеченное `Mapped` действительно меняет разрешённый
//! стиль. Приписать свойству «перенесено» и не написать разбор — не выйдет.
//!
//! Классификация:
//! * `Mapped` — свойство доезжает до GPUI;
//! * `NoOp` — разбирается, но рисовать нечего, и это не искажает картинку
//!   (подсказки движку, вроде `will-change`);
//! * `Impossible` — примитива в GPUI нет, перенести невозможно. Причина
//!   обязательна и попадает в документацию.

use crate::computed::Computed;
use crate::css::parse_decls;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Support {
    /// Значение из образца доезжает до стиля.
    Mapped,
    /// Разбирается и сознательно ничего не делает.
    NoOp(&'static str),
    /// Доезжает, но не во всех значениях или не во всех случаях.
    ///
    /// Отдельный класс появился после третьего аудита: без него «перенесено»
    /// скрывало разницу между «работает как в браузере» и «узнаваемо, но
    /// иначе». Причина обязательна и попадает в документацию.
    Partial(&'static str),
    /// Перенести нечем.
    Impossible(&'static str),
}

/// Свойство, образец значения и вердикт.
pub struct Prop {
    pub name: &'static str,
    pub sample: &'static str,
    pub support: Support,
}

const fn m(name: &'static str, sample: &'static str) -> Prop {
    Prop {
        name,
        sample,
        support: Support::Mapped,
    }
}

const fn no(name: &'static str, why: &'static str) -> Prop {
    Prop {
        name,
        sample: "",
        support: Support::NoOp(why),
    }
}

const fn part(name: &'static str, sample: &'static str, why: &'static str) -> Prop {
    Prop {
        name,
        sample,
        support: Support::Partial(why),
    }
}

const fn imp(name: &'static str, why: &'static str) -> Prop {
    Prop {
        name,
        sample: "",
        support: Support::Impossible(why),
    }
}

/// Полный список свойств, встречающихся в разметке интерфейсов.
///
/// Порядок — по разделам, как в документации.
pub const PROPERTIES: &[Prop] = &[
    // --- Бокс-модель -----------------------------------------------------
    m("box-sizing", "border-box"),
    m("width", "100px"),
    m("height", "100px"),
    m("min-width", "10px"),
    m("min-height", "10px"),
    m("max-width", "500px"),
    m("max-height", "500px"),
    m("inline-size", "100px"),
    m("block-size", "100px"),
    m("min-inline-size", "10px"),
    m("min-block-size", "10px"),
    m("max-inline-size", "500px"),
    m("max-block-size", "500px"),
    m("aspect-ratio", "16 / 9"),
    m("padding", "4px"),
    m("padding-top", "4px"),
    m("padding-right", "4px"),
    m("padding-bottom", "4px"),
    m("padding-left", "4px"),
    m("padding-inline", "4px"),
    m("padding-block", "4px"),
    m("padding-inline-start", "4px"),
    m("padding-inline-end", "4px"),
    m("padding-block-start", "4px"),
    m("padding-block-end", "4px"),
    m("margin", "4px"),
    m("margin-top", "4px"),
    m("margin-right", "4px"),
    m("margin-bottom", "4px"),
    m("margin-left", "4px"),
    m("margin-inline", "4px"),
    m("margin-block", "4px"),
    m("margin-inline-start", "4px"),
    m("margin-inline-end", "4px"),
    m("margin-block-start", "4px"),
    m("margin-block-end", "4px"),
    // --- Раскладка -------------------------------------------------------
    part(
        "display",
        "table",
        "`list-item` и `flow-root` сводятся к блоку: своего маркера-псевдоэлемента у них нет",
    ),
    m("flex-direction", "column"),
    m("flex-wrap", "wrap"),
    m("flex-flow", "column wrap"),
    m("flex", "1 1 auto"),
    m("flex-grow", "1"),
    m("flex-shrink", "0"),
    m("flex-basis", "50px"),
    m("order", "2"),
    m("align-items", "center"),
    m("align-self", "flex-end"),
    m("align-content", "space-between"),
    m("justify-content", "space-between"),
    m("justify-items", "center"),
    m("justify-self", "end"),
    m("place-items", "center start"),
    m("place-content", "center space-between"),
    m("place-self", "center end"),
    m("gap", "8px"),
    m("row-gap", "8px"),
    m("column-gap", "8px"),
    m("grid-gap", "8px"),
    m("grid-row-gap", "8px"),
    m("grid-column-gap", "8px"),
    m("grid-template-columns", "120px 1fr"),
    m("grid-template-rows", "40px auto"),
    m("grid-template", "40px auto / 120px 1fr"),
    m("grid", "auto / repeat(4, auto)"),
    m("grid-auto-columns", "1fr"),
    m("grid-auto-rows", "40px"),
    m("grid-auto-flow", "column"),
    m("grid-column", "1 / 3"),
    m("grid-row", "2 / 4"),
    m("grid-column-start", "1"),
    m("grid-column-end", "3"),
    m("grid-row-start", "2"),
    m("grid-row-end", "4"),
    m("grid-area", "2 / 1 / 4 / 3"),
    m("grid-template-areas", "a b"),
    m("position", "fixed"),
    m("top", "4px"),
    m("right", "4px"),
    m("bottom", "4px"),
    m("left", "4px"),
    m("inset", "4px"),
    m("inset-inline", "4px"),
    m("inset-block", "4px"),
    m("inset-inline-start", "4px"),
    m("inset-inline-end", "4px"),
    m("inset-block-start", "4px"),
    m("inset-block-end", "4px"),
    m("z-index", "10"),
    m("overflow", "auto"),
    m("overflow-x", "scroll"),
    m("overflow-y", "auto"),
    m("visibility", "hidden"),
    m("opacity", "0.5"),
    // --- Фон, рамки, тени ------------------------------------------------
    part(
        "background",
        "#123456",
        "из сокращения читаются цвет, картинка, повтор и размер; позиция и слои — нет",
    ),
    m("background-color", "#123456"),
    m("background-image", "url(logo.png)"),
    m("border", "1px solid #333"),
    m("border-top", "1px solid #333"),
    m("border-right", "1px solid #333"),
    m("border-bottom", "1px solid #333"),
    m("border-left", "1px solid #333"),
    m("border-width", "2px"),
    m("border-top-width", "2px"),
    m("border-right-width", "2px"),
    m("border-bottom-width", "2px"),
    m("border-left-width", "2px"),
    m("border-inline", "1px solid #333"),
    m("border-block", "1px solid #333"),
    m("border-color", "#333"),
    m("border-top-color", "#333"),
    m("border-right-color", "#333"),
    m("border-bottom-color", "#333"),
    m("border-left-color", "#333"),
    part(
        "border-style",
        "dotted",
        "`double`, `groove` и `ridge` рисуются сплошной: рельефа в конвейере нет",
    ),
    m("border-radius", "6px"),
    m("border-top-left-radius", "6px"),
    m("border-top-right-radius", "6px"),
    m("border-bottom-right-radius", "6px"),
    m("border-bottom-left-radius", "6px"),
    m("border-collapse", "collapse"),
    m("border-spacing", "4px"),
    m("outline", "solid #333"),
    m("outline-width", "2px"),
    m("outline-color", "#333"),
    m("outline-offset", "2px"),
    m("box-shadow", "inset 0 2px 4px #0004"),
    m("backdrop-filter", "blur(8px)"),
    // --- Текст -----------------------------------------------------------
    m("color", "#123456"),
    m("font", "italic 700 14px/1.4 monospace"),
    m("font-size", "14px"),
    m("font-weight", "700"),
    m("font-style", "italic"),
    m("font-family", "Segoe UI, sans-serif"),
    m("line-height", "1.4"),
    m("letter-spacing", "0.5px"),
    m("word-spacing", "2px"),
    m("text-align", "justify"),
    m("text-decoration", "underline"),
    m("text-decoration-line", "line-through"),
    m("text-transform", "uppercase"),
    m("text-indent", "12px"),
    m("text-overflow", "ellipsis"),
    m("white-space", "pre-line"),
    m("word-break", "break-all"),
    m("line-break", "anywhere"),
    imp(
        "overflow-wrap",
        "переносчик GPUI рвёт слово, которое иначе не влезает, ВСЕГДА — отличить `normal` от `break-word` нечем, и значение ни на что не влияет",
    ),
    imp(
        "word-wrap",
        "старое имя `overflow-wrap` — то же ограничение",
    ),
    m("text-wrap", "nowrap"),
    m("vertical-align", "middle"),
    m("-webkit-line-clamp", "2"),
    m("list-style", "square"),
    m("list-style-type", "lower-roman"),
    // --- Прочее ----------------------------------------------------------
    m("cursor", "pointer"),
    m("object-fit", "cover"),
    part(
        "pointer-events",
        "none",
        "снимает наведение, курсор и выделение; сквозного клика к элементу под ним нет",
    ),
    m("table-layout", "fixed"),
    // --- Разбирается и ничего не делает ----------------------------------
    no(
        "will-change",
        "подсказка движку о будущих правках — рисовать нечего",
    ),
    part(
        "contain",
        "paint",
        "`paint` и `strict` обрезают содержимое; `size` и `layout` на пересчёт          не влияют — он и так по узлу",
    ),
    m("isolation", "isolate"),
    no(
        "appearance",
        "системного вида у элементов и так нет — рисуем сами",
    ),
    no(
        "box-decoration-break",
        "элемент не разрывается между страницами",
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

/// Доля покрытия.
///
/// Полное совпадение с браузером и честная пустышка идут за единицу,
/// частичное — за половину, невозможное — за ноль. Половина за частичное не
/// научна, но не даёт спрятать «узнаваемо, но иначе» в графу «сделано»:
/// прошлая формула не могла опуститься ниже ста, потому что складывала
/// собственные пометки.
pub fn mapped_pct() -> f32 {
    let score: f32 = PROPERTIES
        .iter()
        .map(|p| match p.support {
            Support::Mapped | Support::NoOp(_) => 1.0,
            Support::Partial(_) => 0.5,
            Support::Impossible(_) => 0.0,
        })
        .sum();
    score * 100.0 / PROPERTIES.len() as f32
}

/// Свойства, которые обещаны как перенесённые, но стиль не меняют.
pub fn broken_promises() -> Vec<&'static str> {
    PROPERTIES
        .iter()
        .filter(|p| matches!(p.support, Support::Mapped | Support::Partial(_)))
        .filter(|p| {
            let decls = parse_decls(&format!("{}: {}", p.name, p.sample));
            let mut c = Computed::default();
            c.apply_decls(&decls);
            format!("{c:?}") == format!("{:?}", Computed::default())
        })
        .map(|p| p.name)
        .collect()
}

/// Исходники, которые ЧИТАЮТ разрешённый стиль.
///
/// Реестр доказывает, что свойство разобрано. Этого мало: поле могло быть
/// заполнено и не прочитано никем — свойство тогда числится поддержанным, а
/// на картинке его нет. Аудит нашёл пять таких. Список ниже — все места, где
/// стиль превращается в элементы; тест требует, чтобы каждое поле
/// разрешённого стиля было прочитано хотя бы в одном из них.
const CONSUMERS: &[&str] = &[
    include_str!("apply.rs"),
    include_str!("render.rs"),
    include_str!("inline.rs"),
    include_str!("forms.rs"),
    include_str!("background.rs"),
    include_str!("border_image.rs"),
    include_str!("svg.rs"),
    include_str!("scroll.rs"),
    include_str!("doc.rs"),
    include_str!("dom.rs"),
    include_str!("transition.rs"),
    include_str!("interact.rs"),
];

/// Читается ли поле в исходнике.
///
/// Простой поиск подстроки `.имя` обманывался вызовами методов: поле `filter`
/// «читал» любой `.filter(` итератора. Поэтому обращение к полю отличается от
/// вызова метода по следующему символу: у поля дальше не круглая скобка.
fn reads_field(source: &str, field: &str) -> bool {
    let needle = format!(".{field}");
    let mut from = 0;
    while let Some(at) = source[from..].find(&needle) {
        let end = from + at + needle.len();
        from = end;
        let next = source[end..].chars().next();
        // Соседняя буква — это другое, более длинное имя.
        if next.is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '(') {
            continue;
        }
        return true;
    }
    false
}

/// Поля, которые потребители читают не напрямую, а через метод стиля:
/// `borders()` гасит толщину рамки без рисунка. Поиск по имени поля такой
/// вызов не видит, поэтому пара «поле — метод» перечислена явно.
const ACCESSORS: &[(&str, &str)] = &[
    ("border_width", "borders()"),
    ("border_visible", "borders()"),
    // Логические свойства ложатся на физические поля отдельным проходом
    // сборщика документа (`doc::resolve_logical`).
    ("logical", "resolve_logical()"),
];

/// Поля разрешённого стиля, которые никто не читает.
pub fn dead_fields() -> Vec<String> {
    let source = include_str!("computed.rs");
    let start = match source.find("pub struct Computed {") {
        Some(i) => i,
        None => return vec![],
    };
    let body = &source[start
        ..source[start..]
            .find(
                "
}",
            )
            .map(|e| start + e)
            .unwrap_or(source.len())];
    body.lines()
        .filter_map(|l| l.trim().strip_prefix("pub "))
        .filter_map(|l| l.split(':').next())
        .map(|f| f.trim().to_string())
        .filter(|f| !f.is_empty() && f.chars().all(|c| c.is_ascii_lowercase() || c == '_'))
        .filter(|f| !CONSUMERS.iter().any(|src| reads_field(src, f)))
        .filter(|f| {
            !ACCESSORS.iter().any(|(field, call)| {
                field == f && CONSUMERS.iter().any(|src| src.contains(call))
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_field_is_written_and_never_read() {
        let dead = dead_fields();
        assert!(
            dead.is_empty(),
            "поля разрешённого стиля никто не читает — свойство числится              поддержанным, но на картинке его нет: {dead:?}"
        );
    }

    #[test]
    fn every_mapped_property_reaches_the_style() {
        let broken = broken_promises();
        assert!(
            broken.is_empty(),
            "помечены перенесёнными, но разбор ничего не меняет: {broken:?}"
        );
    }

    #[test]
    fn the_registry_has_no_duplicates() {
        let mut names: Vec<&str> = PROPERTIES.iter().map(|p| p.name).collect();
        names.sort_unstable();
        let before = names.len();
        names.dedup();
        assert_eq!(before, names.len(), "свойство перечислено дважды");
    }

    #[test]
    fn coverage_does_not_regress() {
        let pct = mapped_pct();
        assert!(pct >= 80.0, "покрытие упало до {pct:.1}%");
    }
}

#[cfg(test)]
mod report {
    use super::*;

    #[test]
    fn print_coverage() {
        let total = PROPERTIES.len();
        let mapped = PROPERTIES
            .iter()
            .filter(|p| p.support == Support::Mapped)
            .count();
        let noop = PROPERTIES
            .iter()
            .filter(|p| matches!(p.support, Support::NoOp(_)))
            .count();
        let partial = PROPERTIES
            .iter()
            .filter(|p| matches!(p.support, Support::Partial(_)))
            .count();
        println!(
            "ПОКРЫТИЕ: всего {total}, полностью {mapped}, частично {partial}, пустышек {noop}, \
             невозможно {}, итого {:.1}%",
            total - mapped - noop - partial,
            mapped_pct()
        );
    }
}
