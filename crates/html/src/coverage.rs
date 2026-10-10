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

use crate::style::computed::Computed;
use crate::style::css::parse_decls;

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
    include_str!("style/apply/mod.rs"),
    include_str!("style/apply/grid.rs"),
    include_str!("style/apply/layout.rs"),
    include_str!("style/apply/box_model.rs"),
    include_str!("style/apply/paint.rs"),
    include_str!("style/apply/text.rs"),
    include_str!("render/mod.rs"),
    include_str!("text/inline/mod.rs"),
    include_str!("text/inline/collect.rs"),
    include_str!("style/cascade/inherit.rs"),
    include_str!("text/inline/hyphenate.rs"),
    include_str!("text/inline/spans.rs"),
    include_str!("text/inline/spacers.rs"),
    include_str!("text/inline/whitespace.rs"),
    include_str!("text/inline/case.rs"),
    include_str!("text/inline/runs.rs"),
    include_str!("interactive/forms.rs"),
    include_str!("paint/background/mod.rs"),
    include_str!("paint/background/shape_path.rs"),
    include_str!("paint/background/shape_raster.rs"),
    include_str!("paint/background/shape_profile.rs"),
    include_str!("paint/background/raster.rs"),
    include_str!("paint/background/image_decode.rs"),
    include_str!("paint/background/tiles.rs"),
    include_str!("paint/border_image/mod.rs"),
    include_str!("svg.rs"),
    include_str!("interactive/scroll.rs"),
    include_str!("document/mod.rs"),
    include_str!("dom/mod.rs"),
    include_str!("dom/walk.rs"),
    include_str!("dom/element_style.rs"),
    include_str!("dom/fixup_tree.rs"),
    include_str!("dom/fixup_grid.rs"),
    include_str!("dom/shadow.rs"),
    include_str!("dom/pseudo.rs"),
    include_str!("dom/scroll_markers.rs"),
    include_str!("dom/xhtml.rs"),
    include_str!("style/select/mod.rs"),
    include_str!("style/select/has.rs"),
    include_str!("style/select/matching.rs"),
    include_str!("animation/transition.rs"),
    include_str!("layout/positioned/anchor/mod.rs"),
    include_str!("animation/motion/mod.rs"),
    include_str!("style/zoom.rs"),
    include_str!("render/blocks/flow.rs"),
    include_str!("render/blocks/positioned.rs"),
    include_str!("render/blocks/canvas.rs"),
    include_str!("layout/table/rows.rs"),
    include_str!("layout/table/cell_borders.rs"),
    include_str!("layout/table/finish.rs"),
    include_str!("layout/multicol/container.rs"),
    include_str!("layout/multicol/stack_child.rs"),
    include_str!("layout/table/paint.rs"),
    include_str!("paint/gap_rules/mod.rs"),
    include_str!("paint/gap_rules/geometry.rs"),
    include_str!("paint/gap_rules/painter.rs"),
    include_str!("text/clamp.rs"),
    include_str!("text/vertical/mod.rs"),
    include_str!("layout/positioned/containing_block.rs"),
    include_str!("interactive/resizable.rs"),
    include_str!("interactive/sticky/element.rs"),
    include_str!("paint/effects/grouped_element.rs"),
    include_str!("paint/effects/transformed_element.rs"),
    include_str!("paint/effects/underlay.rs"),
    include_str!("paint/effects/mask/element.rs"),
    include_str!("paint/effects/filter.rs"),
    include_str!("interactive/scroll_area.rs"),
    include_str!("interactive/frame.rs"),
    include_str!("render/paragraph/mod.rs"),
    include_str!("render/paragraph/pieces.rs"),
    include_str!("render/paragraph/atom_piece.rs"),
    include_str!("render/blocks/mod.rs"),
    include_str!("render/element.rs"),
    include_str!("render/generic_box.rs"),
    include_str!("text/ruby/mod.rs"),
    include_str!("text/text_box.rs"),
    include_str!("animation/frames.rs"),
    include_str!("interactive/scroll_box.rs"),
    include_str!("layout/grid.rs"),
    include_str!("layout/block/containing.rs"),
    include_str!("layout/block/reorder.rs"),
    include_str!("layout/block/margins.rs"),
    include_str!("layout/block/struts.rs"),
    include_str!("layout/writing_mode/mod.rs"),
    include_str!("paint/effects/mask/mod.rs"),
    include_str!("paint/effects/grouped.rs"),
    include_str!("paint/effects/transform.rs"),
    include_str!("layout/page/paged.rs"),
    include_str!("layout/page/names.rs"),
    include_str!("layout/fragment/mod.rs"),
    include_str!("layout/fragment/probe.rs"),
    include_str!("layout/fragment/grid_bands.rs"),
    include_str!("layout/fragment/clone.rs"),
    include_str!("layout/fragment/line_shape.rs"),
    include_str!("layout/fragment/shape_contents.rs"),
    include_str!("layout/fragment/shape_kids.rs"),
    include_str!("layout/fragment/push.rs"),
    include_str!("layout/fragment/flex_lines.rs"),
    include_str!("layout/fragment/table_bands.rs"),
    include_str!("layout/fragment/breaks.rs"),
    include_str!("layout/multicol/column_flow.rs"),
    include_str!("layout/multicol/gap_rules.rs"),
    include_str!("layout/multicol/spanner.rs"),
    include_str!("layout/float/mod.rs"),
    include_str!("layout/float/clear.rs"),
    include_str!("layout/float/initial_letter.rs"),
    include_str!("layout/float/wrap.rs"),
    include_str!("layout/float/band_host.rs"),
    include_str!("layout/float/band_measured.rs"),
    include_str!("layout/float/band_flow_host.rs"),
    include_str!("layout/float/band_nest.rs"),
    include_str!("layout/float/float_flow.rs"),
    include_str!("layout/float/shape_flow.rs"),
    include_str!("layout/positioned/static_position.rs"),
    include_str!("layout/positioned/predicates.rs"),
    include_str!("layout/positioned/relative.rs"),
    include_str!("interactive/sticky/mod.rs"),
    include_str!("layout/replaced/image.rs"),
    include_str!("layout/replaced/limits.rs"),
    include_str!("layout/replaced/iframe.rs"),
    include_str!("layout/atom/mod.rs"),
    include_str!("layout/atom/own_box.rs"),
    include_str!("layout/atom/positioned.rs"),
    include_str!("text/ruby/container.rs"),
    include_str!("layout/table/mod.rs"),
    include_str!("layout/table/columns.rs"),
    include_str!("layout/table/anon.rs"),
    include_str!("paint/stacking.rs"),
    include_str!("paint/decorations/mod.rs"),
    include_str!("paint/decorations/gradient_stripes.rs"),
    include_str!("paint/decorations/backdrop.rs"),
    include_str!("paint/decorations/border_shape.rs"),
    include_str!("paint/decorations/shadows.rs"),
    include_str!("render/box_div.rs"),
    include_str!("render/classify.rs"),
    include_str!("render/util.rs"),
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
    ("logical", ".resolve_logical("),
    ("side_seq", ".resolve_logical("),
    // Каскад читает sequence при `all` reset; затем resolve_logical
    // использует сохранённый порядок физических и логических declarations.
    ("decl_seq", ".apply_decls("),
    // Font-relative исходники разрешаются до создания элементов.
    ("gradient_em", ".resolve_em("),
    ("text_shadow_raw", ".resolve_em("),
    ("transform_raw", ".resolve_em("),
    ("shadow_raw", ".resolve_em("),
    ("transform_origin_raw", ".resolve_em("),
    // Обособление осей читается предикатами: физическая ось зависит ещё и
    // от направления письма.
    ("contain_inline_size", "contains_width()"),
];

/// Поля разрешённого стиля, которые никто не читает.
pub fn dead_fields() -> Vec<String> {
    let source = include_str!("style/computed/fields.rs");
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
            !ACCESSORS
                .iter()
                .any(|(field, call)| field == f && CONSUMERS.iter().any(|src| src.contains(call)))
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
