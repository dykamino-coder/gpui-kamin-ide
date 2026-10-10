//! Тип Computed: все поля вычисленного стиля (один тип, не делится).

use crate::style::computed::*;
use crate::style::values::value::{Color, Len};

#[derive(Clone, Debug, Default)]
pub struct Computed {
    /// Счётчик объявлений этого узла: порядок каскада между логическими и
    /// физическими сторонами (UA `padding-inline-start: 40px` против
    /// авторского `padding-top: 0` в вертикальном письме —
    /// `line-box-direction-vrl-019`).
    pub decl_seq: u32,
    pub side_seq: SideSeq,
    pub display: Option<Display>,
    pub flex_dir: Option<FlexDir>,
    pub flex_wrap: Option<bool>,
    /// `wrap-reverse` — строки укладываются с противоположного края.
    pub flex_wrap_reverse: Option<bool>,
    /// `flex-wrap: balance` (css-flexbox-2 §5.2) — строки режет
    /// балансировщик; ортогонально `wrap`/`wrap-reverse`.
    pub flex_balance: Option<bool>,
    /// `flex-line-count` (css-flexbox-2 §5.3) — минимум строк у balance;
    /// умолчание 1.
    pub flex_line_count: Option<u16>,
    pub flex_grow: Option<f32>,
    pub flex_shrink: Option<f32>,
    /// `flex-basis: content` — основа берётся ПО СОДЕРЖИМОМУ, и заданный
    /// главный размер при ней игнорируется. Ключевого значения у раскладки
    /// под нами нет, поэтому признак хранится отдельно, а основа ставится в
    /// `auto`: этого достаточно, если снять главный размер.
    /// Свой пиксель ЗАМЕЩАЕМОГО элемента: размер из атрибутов `width`/`height`
    /// (у холста — умолчание 300×150). Хранится отдельно от `width`, потому
    /// что `flex-basis: content` снимает ЗАДАННЫЙ главный размер, но свой
    /// пиксель оставляет.
    pub attr_width: Option<Len>,
    pub attr_height: Option<Len>,
    /// Размер по оси пришёл из АТРИБУТА (`<canvas width>`), а не из CSS:
    /// у холста атрибуты — природный размер (HTML §4.12.5), не `width`
    /// (§15.3.10 их к нему не относит), и в сетке ось может растянуться.
    pub attr_sized: (bool, bool),
    pub basis_content: Option<bool>,
    pub align_items: Option<Align>,
    /// `align-self` — про сам элемент; отдельное поле, иначе он выравнивал
    /// бы своих детей вместо себя.
    pub align_self: Option<Align>,
    /// Авторское `align-self: normal` (css-align-3 §6.2): у элемента гибкого
    /// контейнера оно ведёт себя как `stretch`, а не «взять у родителя».
    pub align_self_normal: bool,
    /// Элемент гибкого РЯДА, поперечный размер которого тянет строка
    /// (`stretch` при `height: auto`). Взводит сборщик детей гибкого
    /// контейнера (`render.rs`, ветка `flex_context`), где виден родитель;
    /// читает обособление размера в `apply::apply_box`.
    pub(crate) cross_stretched: bool,
    pub flex_basis: Option<Len>,
    pub justify_content: Option<Justify>,
    pub gap: Option<(Option<Len>, Option<Len>)>,
    /// `box-sizing`. По умолчанию в CSS — `content-box`: заданная ширина
    /// НЕ включает отступы и рамку. Движок раскладки под нами всегда считает
    /// по `border-box`, поэтому разницу приходится компенсировать вручную.
    pub border_box: Option<bool>,
    pub grid_cols: Option<u16>,
    /// Список дорожек, если он выразим: `auto`, `1fr`, px, `%`, `minmax()`.
    pub grid_tracks: Option<Vec<TrackSize>>,
    /// `grid-template-areas`: сетка имён по строкам.
    ///
    /// Именованных областей нет ни в GPUI, ни в taffy, поэтому имена
    /// разворачиваются в номера линий при сборке дерева — там, где известны и
    /// контейнер, и его дети.
    pub grid_areas: Option<Vec<Vec<String>>>,
    /// `grid-template-areas: inherit` (css-cascade-4 §7.3 «explicit
    /// inheritance»): свойство не наследуемое, запись родителя переносит
    /// `doc::settle_explicit_inherit`.
    pub grid_areas_inherit: bool,
    /// Имя области у ребёнка: `grid-area: header`.
    pub grid_area_name: Option<String>,
    /// Имена линий `grid-template-columns` (логические колонки): у
    /// подсеточной оси — её `<line-name-list>` (`subgrid [a] [b]`). Разрешает
    /// их раскладка (taffy `NamedLineResolver`), в том числе через подсетки
    /// (css-grid-2 §9 (d)); прежде имена выбрасывались при разборе.
    pub grid_col_line_names: Option<gpui::GridAxisLineNames>,
    /// То же для `grid-template-rows`.
    pub grid_row_line_names: Option<gpui::GridAxisLineNames>,
    /// Именованные грани `grid-column-start`/`-end` (css-grid-2 §8.3):
    /// `Placement` у такой грани — `Auto`, имя разрешает раскладка.
    pub grid_col_named: [Option<gpui::GridNamedLine>; 2],
    /// То же для рядов.
    pub grid_row_named: [Option<gpui::GridNamedLine>; 2],
    /// `repeat(auto-fill, minmax(N, 1fr))` — сколько влезет колонок шириной
    /// не меньше N. Число колонок здесь считает раскладка, а не разметка.
    pub grid_auto_fill_min: Option<f32>,
    /// Тело повтора из НЕСКОЛЬКИХ дорожек: `repeat(auto-fill, 50px 50px)`
    /// повторяет пару, а не одну дорожку. Пусто — тело из одной дорожки, её
    /// размер лежит в `grid_auto_fill_min`.
    pub grid_auto_fill_tracks: Vec<f32>,
    /// То же по рядам: `grid-template-rows: repeat(auto-fill, 100px)`.
    pub grid_auto_fill_row: Option<f32>,
    /// Повтор «сколько влезет» по колонкам и по рядам целиком: нужен и вид
    /// повтора (`auto-fit` схлопывает пустые дорожки), и размер дорожки
    /// (`None` — дорожка по содержимому).
    /// `grid-template-*: subgrid` — дорожки берутся у родительской сетки.
    /// Своей раскладки подсетки нет, но знать о ней надо: абсолютных потомков
    /// она размещает по СВОИМ линиям, а не по линиям внешней сетки.
    pub subgrid: bool,
    /// Подсеточность ПООСЕВАЯ: `grid-template-columns: subgrid` и
    /// `grid-template-rows: subgrid` — разные объявления, и правило
    /// css-grid-2 §subgrid-box-alignment («в подсеточной оси свой размер и
    /// self-выравнивание игнорируются») действует ровно в СВОЕЙ оси.
    /// Скалярный `subgrid` этого не выражает: у пяти зелёных
    /// `standalone-axis-size-*` подсеточны РЯДЫ, а размер задан по КОЛОНКАМ,
    /// и гасить его нельзя.
    ///
    /// ВАЖНО: маршрут СРЕЗА дорожек эти поля не меняют — срез по-прежнему на
    /// скалярном `subgrid`. Перевод среза на поосевые признаки замерен и
    /// откачен (шапка `dom.rs: subgrid_takes_parent_tracks`, +2/−3).
    pub subgrid_cols: bool,
    pub subgrid_rows: bool,
    /// `container-type: size | inline-size` — элемент стал контейнером
    /// запросов размера и подсеткой быть не может (css-grid-2
    /// §subgrid-listing). Отдельным полем, а НЕ через `contain_size`: голое
    /// `contain: size` подсетку не отменяет — это отдельно проверяют случаи
    /// 8 и 9 `independent-formatting-context.html`.
    pub container_size_query: bool,
    pub auto_repeat_cols: Option<AutoRepeat>,
    pub auto_repeat_rows: Option<AutoRepeat>,
    /// Тело авто-повтора ДОРОЖКА ЗА ДОРОЖКОЙ: `repeat(auto-fill, max-content
    /// min-content)` — это два РАЗНЫХ размера, а не два одинаковых.
    /// css-grid-3 §7.2.1 («The hypothetical size of each track in the repeat()
    /// listing is given by the largest track corresponding to that entry (by
    /// index)», `csswg-drafts/css-grid-3/Overview.bs:469-471`) требует считать
    /// каждую запись тела по ЕЁ функции; скалярные `track`/`intrinsic_min`
    /// этого не выражают. Пишется РЯДОМ с `AutoRepeat` и `grid_tracks` не
    /// трогает: тот путь замерен и откачен (патч II 07.09, v143).
    pub auto_repeat_body_cols: Option<Vec<TrackSize>>,
    pub auto_repeat_body_rows: Option<Vec<TrackSize>>,

    pub width: Option<Len>,
    pub height: Option<Len>,
    pub min_width: Option<Len>,
    pub min_height: Option<Len>,
    pub max_width: Option<Len>,
    pub max_height: Option<Len>,

    pub padding: Sides,
    pub margin: Sides,
    pub border_width: Sides,
    pub border_color: Option<Color>,
    pub radius: Corners,

    pub position: Option<Position>,
    pub inset: Sides,
    /// `anchor-name` (css-anchor-position-1 §anchor-name): имена якоря,
    /// `none` — отсутствие. Коробка с именем получает пробу
    /// (`anchor::probe_for`), пишущую её рамку в реестр кадра.
    pub anchor_name: Option<Vec<String>>,
    /// `position-anchor`; начальное `normal`.
    pub position_anchor: Option<PositionAnchor>,
    /// Неявный якорь псевдоэлемента — `node_id` порождающего элемента
    /// (§implicit: «The implicit anchor element of a pseudo-element is its
    /// originating element»). Ставит `dom::walk` после обхода детей.
    pub implicit_anchor: Option<u64>,
    /// `position-area` (§position-area): два слова области, разбор и смысл —
    /// `anchor::parse_area`. `none`/не задано — `None`.
    pub position_area: Option<crate::layout::positioned::anchor::parse::PositionArea>,
    /// `position-try-fallbacks` (§position-try-fallbacks): варианты позиции —
    /// имя `@position-try`-правила и/или тактика, либо `<position-area>`.
    /// Перебор — `anchor::AnchorPlace` на подготовке кадра, выбранный
    /// вариант накладывается на стиль следующей сборки (`anchor::apply_chosen`).
    pub position_try_fallbacks: Vec<crate::layout::positioned::anchor::parse::TryFallback>,
    /// `position-try-order` (§position-try-order-property): 0 normal,
    /// 1 most-width, 2 most-height, 3 most-block-size, 4 most-inline-size.
    pub position_try_order: u8,
    /// `position-visibility` (§position-visibility), биты `anchor::VIS_*`:
    /// 1 anchor-valid, 2 anchor-visible, 4 no-overflow; 0 — `always`
    /// (начальное значение Blink; спека просит `anchor-visible`, но без
    /// явного свойства гасить коробки по обрезке якоря слишком дорого).
    pub position_visibility: u8,
    /// Стиль ДО наложения выбранного варианта `position-try` — из него
    /// `anchor::place` строит остальные кандидаты. Ставит `anchor::apply_chosen`.
    pub try_base: Option<std::rc::Rc<Computed>>,
    /// Служебные поля якорного шага, ставит `render::element`: свой `node_id`
    /// (ключ реестра содержащих блоков `anchor::CB`), `node_id` ближайшего
    /// содержащего блока абсолюта (`inline::inherit`; 0 — начальный, окно),
    /// порядковый номер сборки в кадре («последний якорь раньше по дереву» в
    /// реестре прошлого кадра) и ключ коробки в реестре размеров клетки.
    pub self_node: u64,
    pub cb_node: u64,
    pub anchor_seq: u32,
    pub anchor_key: u64,
    pub overflow_x: Option<Overflow>,
    /// Внутренняя копия прокручиваемой коробки (`render` снимает с неё
    /// `overflow`, чтобы обёртка `ScrollArea` резала сама): по спеке она
    /// остаётся scroll container — автоминимум по `aspect-ratio` к ней не
    /// применяется (css-sizing-4 §5.2; `block-aspect-ratio-011/012`).
    pub scroller: bool,
    pub overflow_y: Option<Overflow>,
    pub opacity: Option<f32>,

    pub background: Option<Color>,
    /// Фон ЗАЯВЛЕН автором (`background` или `background-color`), пусть даже
    /// прозрачным. Умолчание UA у полей ввода ставится только тогда, когда
    /// автор не сказал ничего: сокращение `background: linear-gradient(...)`
    /// сбрасывает цвет в прозрачный (§2.1), и подставлять поверх него
    /// служебную заливку нельзя.
    pub bg_explicit: bool,
    /// `border: inherit` / `padding: inherit`: свойства не наследуемые, слово
    /// копирует вычисленное значение родителя — оно известно только при
    /// слиянии стилей.
    pub(crate) border_inherit: bool,
    /// То же по СТОРОНАМ и по частям рамки: `border-width: inherit`,
    /// `border-bottom: inherit`, `border-left-color: inherit`. Порядок сторон
    /// всюду один: верх, право, низ, лево.
    pub(crate) border_inherit_w: [bool; 4],
    pub(crate) border_inherit_s: [bool; 4],
    pub(crate) border_inherit_c: [bool; 4],
    pub(crate) padding_inherit: bool,
    /// `inherit` по СТОРОНАМ у полей и отступов, порядок [верх, право, низ,
    /// лево]. Сокращение ставит все четыре.
    pub(crate) margin_inherit: [bool; 4],
    /// `inherit` у ненаследуемых свойств, которым своей ветки не было:
    /// повтор мостовой (`background-repeat`), слой (`z-index`), обводка,
    /// вид (`display`), плитка и её место (`background-image`,
    /// `background-position`), обрезка (`clip`), сокращение шрифта,
    /// преобразование регистра.
    pub(crate) inherit_bits: u32,
    pub(crate) padding_inherit_side: [bool; 4],
    /// `box-shadow: inherit`.
    pub(crate) shadow_inherit: bool,
    /// Относительный цвет фона (css-color-5): функция с `from currentColor`
    /// не решается при разборе — она наследуется КАК ФУНКЦИЯ и считается от
    /// цвета каждого элемента заново.
    pub background_rcs: Option<String>,
    /// `background-color: inherit`: фон не наследуемый, слово переносит
    /// вычисленное значение родителя (включая нерешённую функцию).
    pub(crate) background_inherit: bool,
    /// `background: inherit` — сокращение, значит наследуется весь фон, а не
    /// только цвет.
    pub(crate) background_all_inherit: bool,
    /// Явное `inherit` на ненаследуемых размерах и краях: значение берётся
    /// от родителя при слиянии (`inline::inherit`), как у padding/border.
    pub(crate) width_inherit: bool,
    pub(crate) height_inherit: bool,
    /// То же у пределов: `min-width`, `min-height`, `max-width`, `max-height`.
    pub(crate) minmax_inherit: [bool; 4],
    /// Сторона письма СОДЕРЖАЩЕГО БЛОКА: избыточный край выбирается по ней,
    /// а не по своей (§9.4.3). Ставится при наследовании.
    pub(crate) cb_rtl: bool,
    /// Письмо СОДЕРЖАЩЕГО БЛОКА: вертикальное ли (в том числе повёрнутый
    /// абзац, чей стиль собран горизонтальным клоном с `rotated_line`), идут ли
    /// блоки справа налево, `sideways-*` ли. По ним у переопределённой оси
    /// абсолюта выбирается отбрасываемый край (css-writing-modes-4 §7.1).
    /// Ставится при наследовании, как `cb_rtl`.
    pub(crate) cb_vertical: bool,
    pub(crate) cb_vertical_rl: bool,
    pub(crate) cb_sideways: bool,
    /// top/right/bottom/left.
    pub(crate) inset_inherit: [bool; 4],
    pub gradient: Option<Gradient>,
    pub shadows: Vec<Shadow>,
    /// Внутренние тени (`box-shadow: inset`) — отдельным списком: рисуются
    /// поверх заливки, а не под фигурой.
    pub inset_shadows: Vec<Shadow>,

    pub color: Option<Color>,
    pub font_size: Option<Len>,
    pub font_weight: Option<u16>,
    pub(crate) font_weight_step: i8,
    pub italic: Option<bool>,
    /// `font-style: oblique` отдельно от `italic`: набору наклон один
    /// (`italic` держит оба), а подбору лица это РАЗНЫЕ запросы (css-fonts-4
    /// §font-style-matching) — от выбранного лица зависит `size-adjust`
    /// (`fonts::size_adjust`, `italic-oblique-fallback`).
    pub oblique: Option<bool>,
    pub underline: Option<bool>,
    pub line_through: Option<bool>,
    pub line_height: Option<Len>,
    /// `orphans`/`widows` (css-break-3 §4.4 «Breaks Between Lines»): сколько
    /// строк блока обязано остаться до/после разрыва внутри него. Наследуются,
    /// начальное значение 2 (`None` = 2).
    pub orphans: Option<u16>,
    pub widows: Option<u16>,
    pub text_align: Option<TextAlign>,
    /// `text-align-last` — выключка ПОСЛЕДНЕЙ строки абзаца. Отдельное
    /// свойство, потому что по умолчанию последняя строка не растягивается:
    /// иначе абзац из одного слова разъехался бы во всю ширину.
    pub text_align_last: Option<TextAlign>,
    /// `match-parent` (css-text-3 §text-align): бит 1 — у `text-align-all`,
    /// бит 2 — у `text-align-last`; решается при наследовании по письму
    /// РОДИТЕЛЯ (`inherit_fonts`).
    pub text_align_match_parent: u8,
    /// `image-rendering` (css-images-3 §image-rendering), наследуется:
    /// `Some(true)` — `pixelated`/`crisp-edges`, растр масштабируется без
    /// сглаживания; `Some(false)` — `auto`/`smooth`/`high-quality`.
    pub image_pixelated: Option<bool>,
    /// `text-justify: none` — выключка запрещена, строка идёт как `start`.
    pub no_justify: Option<bool>,
    /// CSS Text 4: ruby annotation justification excludes word spaces.
    pub ruby_justify: Option<bool>,
    /// `text-justify` expansion opportunities (css-text-3 §7.3): `Some(0)`
    /// `inter-word` (word separators only), `Some(2)` `inter-character` /
    /// `distribute` (between typographic character units), `None`/`Some(1)`
    /// `auto` (word separators plus CJK ideographs, as Blink).
    pub justify_chars: Option<u8>,
    /// Internal ruby unit promoted to a technical block for layout.
    pub ruby_unit: bool,
    /// `hanging-punctuation` — какая пунктуация выходит за край строки.
    /// `text-box-trim` — срезать полулидинг первой/последней строки блока.
    pub text_box_trim_start: bool,
    pub text_box_trim_end: bool,
    /// `text-box-edge` — метрики верхнего и нижнего краёв среза.
    pub text_box_over: TextEdge,
    pub text_box_under: TextEdge,
    /// `text-box-edge` задан явно (в т.ч. `auto`/`text`): свойство
    /// наследуемое, и явное значение перекрывает унаследованное, а начальное
    /// `Text` от него неотличимо без флага
    /// (`text-box-trim-not-ignore-nested-text-box-edge`).
    pub text_box_edge_set: bool,
    pub hanging: Option<Hanging>,
    pub nowrap: Option<bool>,
    /// Переводы строк значимы (`white-space: pre*`).
    pub preserve_newlines: Option<bool>,
    /// Пробелы значимы. У `pre-line` переводы строк значимы, а пробелы нет —
    /// без отдельного поля он схлопывал и то и другое.
    pub keep_spaces: Option<bool>,
    pub monospace: Option<bool>,
    /// First available family supplies font-relative metrics.
    pub font_family: Option<String>,
    pub(crate) font_families: Option<Vec<String>>,
    /// Есть ли у рамки ВИДИМЫЙ рисунок. Толщина без рисунка не считается:
    /// начальное значение `border-style` — `none`, и по CSS такая рамка
    /// вычисляется в ноль (`descendant-static-position-001`: коробка выходила
    /// шире на заданную, но не нарисованную рамку).
    pub border_visible: [Option<bool>; 4],
    /// `border-color: currentColor` — цвет берётся из `color` того же узла.
    ///
    /// Внутренний флаг каскада, а не свойство отрисовки: к моменту выхода из
    /// `resolve` он уже подставлен в `border_color`.
    pub(crate) border_color_is_current: bool,
    /// Сторона получила `currentColor` из БОКОВОГО сокращения без цвета
    /// (`border-top: solid 1em`) поверх общего `border-color`, пришедшего
    /// раньше. Внутренний флаг каскада: цвет текста в `border_colors`
    /// подставляет `inline::inherit`, когда тот известен.
    pub(crate) border_side_current: [bool; 4],
    /// `border-collapse: collapse` — зазор между ячейками пропадает.
    pub border_collapse: Option<bool>,
    /// `empty-cells: hide` — у ПУСТОЙ ячейки не рисуются ни фон, ни рамка
    /// (CSS 2.1 §17.6.1.1). Свойство наследуемое.
    pub empty_cells_hide: Option<bool>,
    /// Форма курсора: у GPUI набор совпадает с CSS почти буква в букву.
    pub cursor: Option<String>,
    /// `visibility: hidden` — место занимает, но не рисуется.
    pub hidden: Option<bool>,
    /// `visibility: collapse` — не «невидимый», а ВЫБРОШЕННЫЙ из строки
    /// гибкого контейнера: перенос и размеры считаются без него.
    pub collapsed: Option<bool>,
    /// Опорная коробка clip-path: 0 border, 1 margin, 2 padding, 3 content.
    pub clip_ref: Option<u8>,
    /// `clip-path` задан ОДНИМ словом коробки (`margin-box`, `padding-box`,
    /// …): обрезка краями этой коробки (css-masking-1 §5.1 «If specified by
    /// itself, uses the edges of the specified box … as clipping path»).
    pub clip_bare_box: bool,
    /// Маска-изображение (`mask-image: url(...)|<gradient>`): источник
    /// строкой до растра при сборке группы.
    pub mask_image: Option<String>,
    pub letter_spacing: Option<Len>,
    pub ellipsis: Option<bool>,
    /// Маркер обрезки `text-overflow: <string>` (css-overflow-4 §5);
    /// None при ellipsis — многоточие по умолчанию.
    pub overflow_marker: Option<String>,
    /// `text-overflow: inherit` (css-cascade-4 §7.3): свойство не
    /// наследуемое, значение родителя переносит `doc::settle_explicit_inherit`
    /// (`text-overflow-004`).
    pub text_overflow_inherit: bool,
    /// `block-ellipsis` (css-overflow-4 §block-ellipsis): None — `auto`
    /// (U+2026), пустая строка — `no-ellipsis`, иначе строка-знак. Отдельно
    /// от `overflow_marker`: `text-overflow` относится к строчной оси и
    /// знака на строке обрыва `line-clamp` не задаёт.
    pub clamp_mark: Option<String>,
    /// `list-style: none` — навигация, свёрстанная на списках, иначе идёт с
    /// точками.
    pub no_marker: Option<bool>,
    /// Вид маркера, если документ его задал.
    pub list_style_type: Option<String>,
    /// `list-style-position: inside` — маркер идёт первым куском содержимого
    /// пункта, а не отдельной колонкой снаружи (css-lists-3 §4).
    pub list_style_inside: Option<bool>,
    /// Строковый маркер: `list-style-type: "→ "` (css-lists-3 §3).
    pub marker_text: Option<String>,
    /// Слой `::marker` (css-lists-3 §marker-properties): ТОЛЬКО объявления
    /// самих правил `::marker` поверх таблицы агента, без копии стиля
    /// хозяина — при отрисовке накладывается на стиль пункта через
    /// `inline::inherit`. Копия стиля пункта сюда не годится: она утянула бы
    /// в маркер рамку, поля и размеры самого `<li>`. Blink делает то же —
    /// текст маркера набирается стилем САМОГО `::marker`
    /// (`CreateAnonymousStyleWithDisplay(marker.StyleRef(), …)`,
    /// `list_marker.cc:267-336`). Не наследуется.
    pub marker_layer: Option<Box<Computed>>,
    /// `content: none` против `content: normal`. У `::before`/`::after`
    /// разницы нет — коробки нет в обоих случаях, — а у `::marker` `none`
    /// гасит маркер, `normal` возвращает к `list-style-*` (css-lists-3
    /// §content-property). Оба сбрасывают `content` в None, поэтому нужна
    /// отдельная метка.
    pub content_none: Option<bool>,
    pub object_fit: Option<String>,
    /// `image-orientation: none` (css-images-3 §5.4) — НЕ разворачивать растр
    /// по метке EXIF. Начальное значение свойства — `from-image`, поэтому
    /// хранится именно отказ, а не разрешение.
    pub image_orient_none: Option<bool>,

    /// `aspect-ratio` — отношение ширины к высоте.
    pub aspect_ratio: Option<f32>,
    /// Отношение из записи `auto <ratio>` (css-sizing-4 §5.1): у замещаемого
    /// оно ЗАПАСНОЕ — природное сильнее. Отдельным полем, потому что
    /// НЕзамещаемой коробке отношение из этой записи наша раскладка пока не
    /// выражает (★ ЗАМЕРЕНО: в общем поле `block-aspect-ratio-002/015/016/018/
    /// 043/047`, `grid-aspect-ratio-005/008` уходили с 0.00 в 14.25).
    pub aspect_ratio_auto: Option<f32>,
    /// Коробка с `aspect-ratio` — элемент ГИБКОГО контейнера (ставит
    /// `render.rs` при раскладке детей ряда/колонки). Её автоминимум по
    /// соотношению считает раскладка (css-flexbox-1 §4.5: подсказка по
    /// содержимому), а не явный минимум `apply::ratio_as_auto_min`
    /// (`flex-aspect-ratio-002/004`).
    pub flex_item_ratio: bool,
    /// Коробка АБСОЛЮТНА, но позиционирование с неё снято ради статической
    /// позиции (`render.rs`). Само `position` там обнуляется, а знать о нём
    /// нужно: размер по свободной строчной оси у абсолюта считается по
    /// содержимому, и без этой пометки вертикальный абзац снова растягивался
    /// бы на весь предел ортогонального потока.
    pub abs_static: bool,
    /// Ортогональный элемент СЕТКИ с невытягивающим выравниванием
    /// (`place-items: start` и родня): по строчной оси он размером в
    /// содержимое, а не в область сетки (css-grid-1 §6.6, css-align-3 §6.1).
    /// Ставится сборщиком детей сетки, глубже не наследуется.
    pub hug_inline: bool,
    /// Родитель — ВЕРТИКАЛЬНАЯ сетка с невытягивающим `justify-*` (или блок
    /// по цепочке под ней, `render.rs: vertical_hug_children`): повёрнутый
    /// абзац заявляет высотой длину своей строки. Отдельно от `hug_inline`:
    /// ортогональным элементам горизонтальной сетки (и лункам) такая заявка
    /// противопоказана (`grid-lanes/.../column-explicit-placement-002`
    /// 0.00 -> 10.02 при общем флаге).
    pub hug_claim: bool,
    /// `order`: визуальный порядок в гибкой строке. Раскладка под нами его не
    /// знает, поэтому детей переставляет сам сборщик дерева.
    pub order: Option<i32>,
    pub align_content: Option<Justify>,
    /// Задано ли `align-content` значением, ОТЛИЧНЫМ от `normal`
    /// (css-align-3 §align-block). Отдельно от `align_content`, потому что
    /// `parse_justify` роняет в `None` два разных случая: `normal`
    /// (выравнивания нет — и контекста тоже) и `baseline`/`first`/`last`
    /// (выравнивание есть, раскладка его пока не знает, но КОНТЕКСТ по спеке
    /// заводится). На блочном контейнере флаг делает коробку корнем блочного
    /// контекста форматирования; у флекса и сетки он безразличен — они и так
    /// заводят свой контекст первой же веткой `own_context`.
    pub align_content_block: bool,
    /// `justify-items`/`justify-self` — поперечная ось В СЕТКЕ.
    pub justify_items: Option<Align>,
    /// Модификатор `safe` у выравниваний (css-align §5.3): при переполнении
    /// области выравнивание падает в `start`, чтобы содержимое не обрезалось.
    /// Без него позиция сохраняется и элемент вылезает (unsafe/дефолт).
    pub justify_self_safe: bool,
    pub justify_items_safe: bool,
    pub align_self_safe: bool,
    /// `align-self: self-start`/`self-end` — начало и конец берутся по письму
    /// САМОГО элемента, а не контейнера (css-align-3 §6.2). Значение при этом
    /// остаётся физическим, а «мерить по себе» помнится здесь: зеркалит его
    /// `inline::inherit`, где известны письмо элемента И письмо родителя.
    pub align_self_own_axis: bool,
    /// Ключевое слово `align-self` — ГИБКОЕ (`flex-start`/`flex-end`): его
    /// концы следуют `wrap-reverse` строки, а `start`/`end`/`self-*` — нет
    /// (css-align-3 §6.1). `Align` различия не несёт, его зеркалит
    /// `inline::inherit` (`self-align-start-end-flex-001`).
    pub align_self_flex_kw: bool,
    /// `justify-self: self-start`/`self-end` — по письму САМОГО элемента, как
    /// `align_self_own_axis` (`align-self-static-position-006`).
    pub justify_self_own_axis: bool,
    /// Preserve line-left/line-right separately from flow-relative start/end.
    pub justify_self_physical: Option<bool>,
    /// `last baseline`: запасное выравнивание — `end`, а не `start`
    /// (css-align-3 §9.3; `align-self-static-position-008`,
    /// `justify-self-static-position-001`). `Align::Baseline` его не различает.
    pub align_self_last: bool,
    /// Авторское объявление `align-self` (внешний `Some`), с его значением.
    /// Нужно, чтобы отличать значение автора от приёмов сборки, которые
    /// пишут в то же поле `align_self` (блок собран колонкой flex): только
    /// авторское значение гасится у коробки вне гибкого контейнера и сетки
    /// (`inline::inherit`), и только оно же переходит по `inherit` к детям —
    /// вычисленное значение родителя от гашения не меняется (css-align-3
    /// §6.1 «Applies to: flex items, grid items, and absolutely-positioned
    /// boxes»; css-cascade-4 §7.3).
    pub(crate) align_self_decl: Option<Option<Align>>,
    /// `align-self: inherit` — значение берёт `inline::inherit` у родителя
    /// (свойство ненаследуемое, слово копирует вычисленное значение).
    pub(crate) align_self_inherit: bool,
    /// `inherit` у `align-items`/`justify-items`/`align-content`/
    /// `justify-content`/`justify-self` (разряды `ainh::*`): свойства
    /// ненаследуемые, слово копирует вычисленное значение родителя
    /// (css-cascade-4 §7.3.1) — его берёт `inline::inherit`.
    pub(crate) align_inherit: u8,
    pub justify_self_last: bool,
    /// `align-items: last baseline` — то же для умолчания детей: раскладка
    /// получает `LastBaseline` (css-align-3 §4.2), а не первую базовую.
    pub align_items_last: bool,
    /// `justify-items: last baseline` — для лунок-колонок (поперёк лунки).
    pub justify_items_last: bool,
    pub align_items_safe: bool,
    pub justify_content_safe: bool,
    pub align_content_safe: bool,
    /// Порог «равных» лунок (`flow-tolerance`): None = `normal`/не задано (= 1em).
    pub lanes_tolerance: Option<Len>,
    /// `grid-lanes-direction: row` — лунки идут РЯДАМИ, элементы укладываются
    /// вдоль строки, а не вдоль колонки.
    pub lanes_row: Option<bool>,
    /// `fill-reverse` — лунки заполняются С ДРУГОГО КОНЦА: первым выбирается
    /// самое правое (нижнее) свободное место, а не левое.
    pub lanes_fill_reverse: bool,
    /// `track-reverse` — сами лунки перечислены в обратном порядке: первая
    /// дорожка списка встаёт последней.
    pub lanes_track_reverse: bool,
    /// `grid-lanes-pack: dense` — элемент встаёт в САМОЕ ВЕРХНЕЕ свободное
    /// место, а не под всё уже уложенное: дыры, оставленные многолуночными
    /// соседями, заполняются следующими элементами.
    pub lanes_dense: bool,
    /// `display: inline grid-lanes` — контейнер лунок строчного уровня:
    /// ширина по дорожкам, не на всю строку.
    pub lanes_inline: bool,
    /// Контейнер лунок, переведённый на путь СЕТКИ: `display` уже
    /// `Grid`/`InlineGrid`, а раскладку лунками делает taffy
    /// (`vendor/taffy/src/compute/grid/lanes.rs`). Ставит
    /// `dom::lanes_as_grid`.
    pub lanes_taffy: bool,
    pub justify_self: Option<Align>,
    /// Explicit normal must not take the parent's justify-items value.
    pub justify_self_normal: bool,

    pub grid_rows: Option<Vec<TrackSize>>,
    pub grid_auto_cols: Option<TrackSize>,
    /// Список неявных дорожек, когда их больше одной: `grid-auto-columns: A B C`.
    /// Пусто — дорожка одна, она в `grid_auto_cols`.
    pub grid_auto_cols_list: Vec<TrackSize>,
    /// То же для неявных РЯДОВ.
    pub grid_auto_rows_list: Vec<TrackSize>,
    pub grid_auto_rows: Option<TrackSize>,
    pub grid_auto_flow: Option<AutoFlow>,
    pub grid_col: Option<(Placement, Placement)>,
    pub grid_row: Option<(Placement, Placement)>,

    /// `z-index`: порядок наложения. У GPUI слоёв нет — вместо них отложенная
    /// отрисовка с приоритетом.
    pub z_index: Option<i32>,
    /// Цвета рамки по сторонам. У GPUI цвет рамки один на элемент, поэтому
    /// разные цвета сторон дорисовываются полосами.
    pub border_colors: [Option<Color>; 4],
    /// `font-size: larger` (+1) / `smaller` (−1): шаг по таблице ключевых
    /// кеглей от кегля родителя (§15.7). Разрешает `inline::inherit`.
    pub(crate) font_size_step: i8,
    /// Ранги стилей кромок по сторонам [верх, право, низ, лево] для
    /// разбора конфликтов сросшихся рамок (CSS 2.1 §17.6.2.1):
    /// 0 none, 1 hidden, 3 inset, 4 groove, 5 outset, 6 ridge, 7 dotted,
    /// 8 dashed, 9 solid, 10 double. `None` — стиль не задавался.
    pub border_side_styles: [Option<u8>; 4],
    /// `caption-side: bottom` — заголовок таблицы под сеткой.
    pub caption_bottom: Option<bool>,
    pub border_dashed: Option<bool>,
    /// `border-style: dotted` — точечный узор.
    pub border_dotted: Option<bool>,
    /// Сырая запись градиента фона: источник для слоя-картинки там, где
    /// градиент рисуется плиткой (фон ряда таблицы).
    pub gradient_raw: Option<String>,
    /// Градиент фона с длинами в единицах шрифта (`green 4em`): позиция
    /// стопа в `em` разбором не читается и терялась (стоп становился «без
    /// позиции»). Ждёт своего кегля и переразбирается в `resolve_em`
    /// (`white-space-intrinsic-size-017/018`).
    pub gradient_em: Option<String>,
    /// `border-spacing` таблицы: горизонтальный и вертикальный зазор.
    pub border_spacing: Option<(Option<Len>, Option<Len>)>,
    pub outline: Option<Outline>,
    /// `backdrop-filter: blur(N)` — размытие того, что под элементом.
    pub backdrop_blur: Option<f32>,
    /// `backdrop-filter`: цветовые функции списка (без размытия). Рисуются
    /// матрицей 4×5 тем же проходом подложки (`Filter::color_matrix`).
    pub backdrop_color: Option<Filter>,

    pub word_spacing: Option<Len>,
    /// Межбуквенный интервал ПОСЛЕ последнего знака этого куска.
    ///
    /// На границе двух элементов зазор задаёт не сам кусок, а ближайший общий
    /// предок обоих знаков (css-text-3 §8.2: зазор «задаётся и рисуется внутри
    /// самого внутреннего элемента, который содержит эту границу»). Видно эту
    /// границу только сборке кусков, она поле и ставит; из стилей документа
    /// оно не приходит и по дереву не наследуется.
    pub letter_spacing_after: Option<Len>,
    pub text_transform: Option<TextTransform>,
    /// Добавки `text-transform` к регистру (css-text-3 §2.1: значение —
    /// `[ case ] || full-width || full-size-kana`, плюс `math-auto`
    /// MathML Core §2.1.5): `TT_FULL_WIDTH` | `TT_KANA` | `TT_MATH`. Живут
    /// вместе с `text_transform` и наследуются вместе с ним.
    pub text_transform_flags: u8,
    pub text_indent: Option<Len>,
    /// `text-indent: … each-line` — отступ повторяется после жёстких разрывов.
    pub text_indent_each_line: Option<bool>,
    /// `text-indent: … hanging` — отступ получают все строки, КРОМЕ первой.
    pub text_indent_hanging: Option<bool>,
    /// `word-break: break-all` и родня — рвать слово, а не переносить целиком.
    pub break_anywhere: Option<bool>,
    /// `overflow-wrap: break-word|anywhere` — рвать слово, ТОЛЬКО если иначе
    /// оно не влезает в строку целиком.
    pub break_word: Option<bool>,
    /// `text-wrap: balance` — строки абзаца одной длины.
    pub balance_lines: Option<bool>,
    /// `unicode-bidi: bidi-override` (и тег `<bdo>`) — порядок знаков задан
    /// силой, разбор двунаправленности внутри куска не работает.
    pub bidi_override: Option<bool>,
    /// Explicit inheritance of the otherwise non-inherited unicode-bidi property.
    pub(crate) bidi_inherit: bool,
    /// `unicode-bidi: isolate` — кусок не влияет на порядок соседей.
    pub bidi_isolate: Option<bool>,
    /// `unicode-bidi: embed` — свой уровень встраивания (RLE/LRE … PDF). Без
    /// него (`normal`) `direction` строчного элемента порядка знаков НЕ
    /// меняет (css-writing-modes-3 §2.2: «normal — the element does not open
    /// an additional level of embedding»). Атрибут `dir` ставит его сам
    /// (`dom.rs: apply_direction`).
    pub bidi_embed: Option<bool>,
    /// `unicode-bidi: plaintext` — сторона письма решается для каждого абзаца
    /// между жёсткими разрывами. HTML ставит это правило на `dir="auto"`.
    pub bidi_plaintext: Option<bool>,
    /// `line-break: anywhere` — разрыв разрешён В ЛЮБОМ месте, включая
    /// соседство с пробелом. Это НЕ то же самое, что `word-break: break-all`:
    /// тот рвёт только внутри слова.
    pub break_anywhere_strict: Option<bool>,
    /// `line-break: normal` (1) / `loose` (2): уровень строгости переноса
    /// CJK (css-text-3 §5.2); `auto`/`strict`/`anywhere` — `None`.
    pub line_break_loose: Option<u8>,
    /// `word-break: keep-all` — иероглифическое письмо переносится ТОЛЬКО по
    /// пробелам, между знаками разрыв запрещён.
    pub keep_all: Option<bool>,
    /// `white-space: break-spaces` — сохранённый пробел НЕ свисает за край:
    /// он занимает место в строке, и после каждого такого пробела разрешён
    /// перенос. Отличие от `pre-wrap`, где хвостовые пробелы висят снаружи.
    pub break_after_spaces: Option<bool>,
    /// `-webkit-line-clamp`: сколько строк оставить.
    pub line_clamp: Option<u32>,
    /// `-webkit-line-clamp`: действует ТОЛЬКО в паре с
    /// `display: -webkit-box` и `-webkit-box-orient: vertical`
    /// (css-overflow-3 §webkit-line-clamp) — поэтому своё поле и гейт.
    /// Какое сокращение записало `line_clamp` последним: `-webkit-line-clamp`
    /// ставит `continue: -webkit-legacy`, который действует только при
    /// `display: -webkit-box` с вертикальной ориентацией (css-overflow-4
    /// §5.1); оба сокращения — одни лонгхенды, побеждает последнее.
    pub clamp_legacy: Option<bool>,
    /// `line-clamp: auto` — обрезка по max-height контейнера
    /// (css-overflow-4 §line-clamp), без счёта строк.
    pub clamp_auto: Option<bool>,
    /// `fill` для SVG-фигур (CSS-презентация, SVG 2).
    pub svg_fill: Option<String>,
    /// `stroke` и `stroke-width` фигуры (SVG 2 §presentation attributes):
    /// правила из `<style>` с селекторами до растеризатора иначе не доедут —
    /// он видит только сериализованную разметку.
    pub svg_stroke: Option<String>,
    pub svg_stroke_width: Option<String>,
    /// CSS-геометрия фигуры (SVG 2 §Geometry properties): `x` и `y`.
    /// Ширина и высота уже живут в `width`/`height`.
    pub svg_x: Option<Len>,
    pub svg_y: Option<Len>,
    pub webkit_box: Option<bool>,
    pub webkit_box_vertical: Option<bool>,
    /// `text-fit` — подбор кегля под ширину коробки.
    pub text_fit: Option<TextFit>,
    /// `hyphenate-character` — чем показывать перенос слова. Пусто — ничем.
    pub hyphen_char: Option<String>,
    /// `hyphens: auto` — слогораздел ставит сам движок, а не разметка.
    pub hyphens_auto: Option<bool>,
    /// Язык узла (атрибут `lang`): по нему выбираются образцы слогораздела.
    pub lang: Option<String>,
    /// `vertical-align` внутри строки и ячейки таблицы.
    pub vertical_align: Option<Align>,
    pub pointer_events_none: Option<bool>,
    /// `table-layout: fixed` — колонки равной ширины, без замера содержимого.
    pub table_fixed: Option<bool>,
    /// `column-count` — на сколько колонок резать содержимое.
    pub column_count: Option<u16>,
    /// `column-width` — минимальная ширина колонки.
    pub column_width: Option<Len>,
    /// `column-height` (css-multicol-2 §ch) — заданная высота колонки;
    /// `auto` хранится отсутствием значения.
    pub column_height: Option<Len>,
    /// `column-wrap` (css-multicol-2 §cwr): `Some(true)` — `wrap`, лишние
    /// колонки уходят в новый ряд; `Some(false)` — `nowrap`, вбок; `None` —
    /// `auto`: как `wrap` при заданном `column-height`, иначе `nowrap`.
    pub column_wrap: Option<bool>,
    /// `column-gap` — зазор между колонками многоколоночного потока.
    /// Умолчание CSS — `normal`, то есть один кегль.
    pub column_gap: Option<Len>,
    /// `translate` — визуальный сдвиг, не меняющий раскладку.
    pub translate: Option<(Len, Len)>,
    /// `text-shadow`: смещение, размытие и цвет — ПЕРВАЯ (верхняя) тень списка.
    pub text_shadow: Option<Shadow>,
    /// Остальные тени `text-shadow` в порядке записи: свойство — СПИСОК, и
    /// «the first shadow is on top» (css-text-decor-3 Overview.bs:881-882).
    /// Первая живёт в `text_shadow`: на неё смотрят зум и наследование.
    /// Прежде хвост отбрасывался — `-1em 0em orange, 1em 0em blue` у эталона
    /// `box-shadow-multiple-001-ref` рисовал одну оранжевую.
    pub text_shadow_rest: Vec<Shadow>,
    /// `text-shadow` с длинами в единицах шрифта ждёт своего кегля, как
    /// `shadow_raw` у `box-shadow` (вычисленное значение — «three absolute
    /// lengths», css-text-decor-3 Overview.bs:868-869), и разбирается в
    /// `resolve_em`.
    pub text_shadow_raw: Option<String>,
    /// `text-shadow: none`, записанное самим элементом: гасит унаследованный
    /// список (без флага пустой разбор читался как «не задано»).
    pub text_shadow_none: bool,
    /// `animation` — ссылка на набор кадров.
    pub animation: Option<AnimSpec>,
    /// `transition` — длительность перехода в секундах.
    pub transition: Option<f32>,
    /// `direction: rtl` — письмо справа налево.
    pub rtl: Option<bool>,
    /// `resize` — по каким осям элемент тянется мышью.
    pub resize: Option<(bool, bool)>,
    /// `transform`/`rotate`/`scale`: поворот в радианах и масштаб по осям.
    pub transform: Option<Transform>,
    /// `rotate` вокруг оси z (радианы) и `scale` по осям — отдельными полями:
    /// разложение в `transform` порядок теряет, а css-transforms-2 §ctm ставит
    /// их СЛЕВА от списка `transform` (п.4-5 перед п.7). `None` — `none`.
    pub rotate_prop: Option<f32>,
    pub scale_prop: Option<(f32, f32)>,
    /// `offset-path` как записано (motion-1 §offset-path): `path('…')`,
    /// `ray(…)`, `<basic-shape>` или `url(#id)`. Разбирается не здесь:
    /// сэмплеру нужны все остальные `offset-*`, а каскад сводит их вразнобой.
    pub offset_path: Option<String>,
    /// `offset-distance`: длина или доля ДЛИНЫ ПУТИ (а не коробки).
    pub offset_distance: Option<Len>,
    /// `offset-rotate` как записано: `auto | reverse | <angle> | auto <angle>`.
    pub offset_rotate: Option<String>,
    /// `offset-anchor` как записано; `auto` — это точка `transform-origin`.
    pub offset_anchor: Option<String>,
    /// `offset-position` как записано: `normal | auto | <position>`.
    pub offset_position: Option<String>,
    /// `backface-visibility: hidden`.
    pub backface_hidden: Option<bool>,
    /// `transform-origin` в долях размера элемента.
    pub transform_origin: Option<(f32, f32)>,
    /// `transform`/`transform-origin` с длинами в единицах шрифта: запись
    /// ждёт своего кегля и разбирается в `resolve_em` (css-transforms-1
    /// §computed value: относительные длины становятся абсолютными).
    pub transform_raw: Option<String>,
    /// `box-shadow` с длинами в единицах шрифта — так же ждёт своего кегля и
    /// разбирается в `resolve_em` (`box-shadow-calc`: `calc(1em + 10px)`
    /// прежде ронял тень целиком — `parse_shadows` пропускает `em`).
    pub shadow_raw: Option<String>,
    /// Есть ли выше трансформированный предок: он — содержащий блок и для
    /// `position: fixed` (css-transforms-1 §transform-rendering: «…for all
    /// of its absolute-position descendants, fixed-position descendants»).
    pub transform_ancestor: bool,
    pub transform_origin_raw: Option<String>,
    /// Точка отсчёта преобразования В ТОЧКАХ по осям — когда записана длиной,
    /// а не долей. Долю из неё делает отрисовка: размер коробки известен там.
    pub transform_origin_px: (Option<f32>, Option<f32>),
    /// Третья координата `transform-origin` в точках (css-transforms-2);
    /// на плоскую матрицу не влияет, на 4×4 — `T(o)·M·T(−o)` по трём осям.
    pub transform_origin_z: Option<f32>,
    /// `perspective` (css-transforms-2 §perspective-property) — расстояние
    /// до глаза в css-точках для ОБЪЁМНЫХ ДЕТЕЙ, уже не меньше 1px («values
    /// less than 1px must be treated as 1px»); `none` = None.
    pub perspective: Option<f32>,
    /// `perspective-origin` долями коробки (умолчание 50% 50%) и в точках по
    /// осям, когда записан длиной — как `transform_origin`/`_px`.
    pub perspective_origin: Option<(f32, f32)>,
    pub perspective_origin_px: (Option<f32>, Option<f32>),
    /// `transform-box: fill-box` у SVG-фигуры (css-transforms-1
    /// §transform-box): длины `transform-origin` отсчитываются от рамки
    /// фигуры, а не от вьюпорта.
    pub transform_box_fill: Option<bool>,
    /// Опорная коробка `transform-box` целиком (css-transforms-1
    /// §transform-box) — для SVG-элементов без CSS-коробки: 0 — `view-box`,
    /// 1 — `fill-box` (и `content-box`: «the used value for content-box is
    /// fill-box»), 2 — `stroke-box` (и `border-box`). None — не задано.
    pub transform_box: Option<u8>,
    /// `vector-effect: non-scaling-stroke` (SVG 2 §vector-effect): толщина
    /// обводки задана в точках экрана.
    pub svg_non_scaling: Option<bool>,
    /// Ячейка матрицы перспективы (см. `PerspectiveFrame`); один и тот же
    /// `Rc` у `e.style` родителя, его `merged` и `inherited` детей.
    pub perspective_frame: Option<PerspectiveFrame>,
    /// `transform-style: preserve-3d` — элемент образует объёмный контекст
    /// (css-transforms-2 §transform-style-property). ИСПОЛЬЗУЕМОЕ значение
    /// гасят «групповые» свойства — это решает `render::flattens_3d`,
    /// потому что они могут быть записаны в блоке ПОСЛЕ `transform-style`.
    pub preserve_3d: Option<bool>,
    /// Ячейка накопленной 4×4 (см. `Frame3d`): заводится при разборе
    /// `transform-style`, наполняется `Transformed::paint` владельца,
    /// читается объёмным путём ПРЯМЫХ детей.
    pub frame_3d: Option<Frame3d>,
    /// `float`: -1 — влево, 1 — вправо, 0 — не обтекается.
    pub float: Option<i8>,
    /// Unemitted adjoining margin at the float's source position (CSS 2.1 §9.5.1).
    pub(crate) float_margin_offset: Option<f32>,
    /// Explicit `clear: inherit`; ordinary `clear` is non-inherited.
    pub(crate) clear_inherit: bool,
    /// `background-attachment: fixed` — плитка считается от области
    /// просмотра, а не от коробки.
    pub bg_fixed: Option<bool>,
    /// `clear` — сторона, с которой обтекание обрывается перед этим блоком:
    /// -1 слева, 1 справа, 0 с обеих. Стороны различаются, потому что
    /// `clear: left` мимо правого флоата проходит насквозь (CSS 2.1 §9.5.2).
    pub clear: Option<i8>,
    /// `writing-mode`: вертикальное письмо — блоки идут по горизонтали.
    pub vertical: Option<bool>,
    /// `writing-mode: vertical-rl` — блоки идут справа налево.
    pub vertical_rl: Option<bool>,
    /// `writing-mode: sideways-*`: глифы повёрнуты, а у `sideways-lr` строка
    /// идёт СНИЗУ вверх — содержимое прижимается к нижнему краю.
    pub sideways: Option<bool>,
    /// `text-combine-upright`: сколько знаков сжимается в один кегль
    /// (0 — `all`, 2..=4 — `digits N`). Наследуется.
    pub combine_upright: Option<u8>,
    /// Служебное: абзац собран для ПОВЁРНУТОЙ отрисовки вертикального
    /// письма — местам с `text-combine-upright` нужен контр-поворот.
    pub rotated_line: Option<bool>,
    /// Ограничение ОРТОГОНАЛЬНОГО потока: определённый размер ближайшего
    /// предка-контейнера прокрутки по оси потока (CSS Writing Modes §7.3).
    /// Наследуется вниз, потому что искать его надо ВВЕРХ по дереву, а на
    /// момент раскладки ребёнка предков уже не видно.
    pub ortho_limit: Option<f32>,
    /// Nearest ancestor scrollport, including an indefinite nearest scroller.
    pub(crate) orthogonal_scrollport: Option<[orthogonal::AxisSizes; 2]>,
    /// Used inline measurement contract of an ordinary orthogonal block.
    pub(crate) orthogonal_inline: Option<orthogonal::InlineConstraint>,
    /// Ячейка таблицы, ПАРАЛЛЕЛЬНОЙ своему письму: доступное инлайн-место у
    /// неё ОПРЕДЕЛЕНО — это мера её КОЛОНКИ (css-tables-3
    /// §computing-column-measures), — и запасной предел §7.3
    /// (`ortho_limit`) применять нельзя: тот стоит на месте НЕОПРЕДЕЛЁННОГО
    /// инлайн-места (css-writing-modes-4 §7.3.1, «an additional constraint is
    /// used as a fallback in place of the available inline space»). Blink
    /// делит эти случаи ровно так же: `space_utils.h:36
    /// SetOrthogonalFallbackInlineSizeIfNeeded` выходит НЕ СДЕЛАВ НИЧЕГО при
    /// `IsParallelWritingMode(таблица, ячейка)`, а
    /// `table_layout_utils.cc:1363 SetupTableCellConstraintSpaceBuilder`
    /// кладёт ячейке `SetAvailableSize({cell_inline_size, …})`, где
    /// `cell_inline_size` собран из `column_locations` — то есть из дорожки.
    /// Флаг ставит `table()` своим ячейкам; ГЛУБЖЕ НЕ НАСЛЕДУЕТСЯ (как
    /// `hug_inline`): `inline::inherit` начинает с `own.clone()`, а у
    /// вложенного элемента поле пусто.
    pub ortho_col: bool,
    /// Physical paragraph of a `text-orientation: upright` vertical stack
    /// (its clone clears `vertical`). Not inherited, like `ortho_col`.
    pub(crate) upright_stack: bool,
    /// Повёрнутый абзац `vertical-lr`: строки-колонки идут слева направо —
    /// подача строк снизу вверх (см. `Paragraph::reversed_lines`).
    pub lines_reversed: Option<bool>,
    /// `overflow-wrap: anywhere` — в отличие от `break-word`, меняет размер
    /// по минимальному содержимому.
    pub wrap_anywhere: Option<bool>,
    /// `word-space-transform` — чем ПОКАЗЫВАТЬ точку переноса (`<wbr>`,
    /// нулевой пробел): обычным пробелом или идеографическим. Точкой переноса
    /// она при этом быть не перестаёт.
    pub word_space_char: Option<char>,
    /// Edge spacer of an inline box (`inline::SPACER`): (box id, physical
    /// left edge, parent direction is rtl). Bidi reordering moves the edges to
    /// the box's visually outermost fragments (CSS 2.1 §8.6).
    pub spacer_edge: Option<(u32, bool, bool)>,
    /// `text-autospace` — зазор в 1/8 кегля между иероглифом и соседней
    /// буквой или цифрой (css-text-4 §7). Наследуется, поэтому живёт здесь;
    /// сами зазоры расставляет `inline::autospace_pieces` по кускам абзаца.
    pub autospace_alpha: Option<bool>,
    pub autospace_numeric: Option<bool>,
    /// Сдвиг куска по вертикали в долях кегля: `vertical-align: super` и
    /// `sub`. Не наследуется — принадлежит самому куску.
    pub vertical_shift: Option<f32>,
    /// То же, но ПРОЦЕНТОМ: доля считается от `line-height` куска, а не от
    /// кегля (§10.8.1), и хранить её вместе с `em` нельзя.
    pub vertical_shift_pct: Option<f32>,
    /// Сдвиг от базовой линии, названный ДЛИНОЙ: хранится в точках, потому
    /// что доля кегля на момент разбора ещё неизвестна — у строчного своего
    /// кегля обычно нет, он приходит наследованием.
    pub vertical_shift_px: Option<f32>,
    /// Сдвиг, названный единицей ШРИФТА (`ex`, `ch`): хранится сырым —
    /// метрики гарнитуры и кегль известны только при наборе строки.
    pub vertical_shift_len: Option<Len>,
    /// `vertical-align: text-top` (`true`) и `text-bottom` (`false`): край
    /// куска равняется по краю ТЕКСТОВОЙ области родителя, а не строки, —
    /// величина зависит от кеглей обоих и считается при наборе.
    pub vertical_align_text: Option<bool>,
    /// Кегль РОДИТЕЛЯ строчного куска: `text-top`/`text-bottom` равняются по
    /// его текстовой области, а не по самой высокой в строке.
    pub vertical_align_base: Option<f32>,
    /// Накопленный относительный сдвиг строчных предков куска в точках
    /// (CSS 2.1 §9.4.3): двигает ТОЛЬКО отрисовку, места в потоке не меняет
    /// и строку не растит.
    pub rel_shift: Option<(f32, f32)>,
    /// `text-orientation: upright` — глифы стоят прямо, а не лежат боком.
    /// Меняет и меру `ch`: продвижение нуля идёт вдоль оси СТРОКИ, а она в
    /// вертикальном письме вертикальна, то есть равна кеглю.
    pub upright: Option<bool>,
    /// `text-orientation: sideways` — у вертикального текста ДОМИНАНТНАЯ
    /// базовая алфавитная, а не центральная (css-writing-modes-4 §4.2:
    /// «In vertical typographic mode, the central baseline is used as the
    /// dominant baseline when text-orientation is mixed or upright»).
    /// `upright` этого не различает: `Some(false)` — и `mixed`, и `sideways`.
    pub text_sideways: Option<bool>,
    /// Родитель — сетка (не лунки): 1 — горизонтальная, 2 — `vertical-lr`,
    /// 3 — `vertical-rl`. Ставится при наследовании; по нему элементу
    /// переставляются оси выравнивания вертикальной сетки и пишутся биты
    /// базовой по оси x (`apply.rs`).
    pub(crate) parent_grid: u8,
    /// Родитель — гибкий контейнер, сетка или лунки: элемент блокифицирован
    /// (css-display-3 §2.7), хотя `display` в стиле остаётся строчным.
    pub(crate) parent_flex_grid: bool,
    /// Родитель — лунки (`display: grid-lanes`).
    pub(crate) parent_lanes: bool,
    /// Родитель — подсетка (`grid-template-*: subgrid`).
    pub(crate) parent_subgrid: bool,
    /// Абзац вертикального письма набирается САМ, по оси строки решённой
    /// раскладкой (`lines::Paragraph::vertical`): `Some(rl)`. Ставится только
    /// на копию стиля внутри `render::paragraph`.
    pub(crate) para_vertical: Option<bool>,
    /// Логические стороны и размеры до перевода в физические.
    pub logical: Option<Box<Logical>>,
    /// Ширина пришла из ЛОГИЧЕСКОГО `inline-size` при вертикальном письме:
    /// физически это высота (оси не переставляются, см. `resolve_logical`) —
    /// потребители обязаны отличать её от настоящего `width`
    /// (table-cell-align-005 против table-progression-htb-001).
    pub width_from_inline: bool,
    /// `hyphens`: разрешён ли перенос по мягкому переносу.
    pub hyphenate: Option<bool>,
    /// `user-select: none` — текст не выделяется.
    pub no_select: Option<bool>,
    /// `frame-sizing: content-height` (css-sizing-4
    /// §frame-sizing): высота `<iframe>` — по содержимому вложенного
    /// документа, если он сам согласился (`<meta name=responsive-embedded-sizing>`).
    pub frame_sizing_height: bool,
    /// Стиль первой буквы абзаца (`::first-letter`).
    ///
    /// Живёт в стиле, а не в элементе, потому что абзац собирается из кусков
    /// уже без узла-родителя: до кусков доезжает только вычисленный стиль.
    pub first_letter: Option<Box<Computed>>,
    /// Renderer metadata: marker text is outside first-letter selection.
    pub first_letter_excluded: bool,
    /// Own declarations remain separate for fictitious inheritance in descendants.
    pub first_letter_own: Option<Box<Computed>>,
    /// Стиль первой строки абзаца (`::first-line`).
    pub first_line: Option<Box<Computed>>,
    /// Только объявления `::first-line` самого узла — их получает первый
    /// блок-потомок, несущий первую строку (`render/first_line_descendants`).
    pub first_line_own: Option<Box<Computed>>,
    /// `initial-letter` (css-inline-3 §initial-letter): размер буквицы в
    /// строках и её осадка (sink) — на базовой какой строки она стоит.
    /// `None` — `normal`, обычная буква. Живёт в слое `::first-letter`.
    pub initial_letter: Option<(f32, u32)>,
    /// Фон строчного бокса: `<span style="background">` внутри абзаца.
    ///
    /// Обычный фон принадлежит коробке, а у строчного бокса коробки нет — он
    /// тянется по строкам вместе с текстом и рвётся на переносах. Поэтому
    /// цвет едет вниз вместе с текстовыми свойствами и попадает в прогон.
    pub inline_bg: Option<Color>,
    /// Рамка СТРОЧНОЙ коробки — цвет и толщина. Рисует её прогон текста
    /// вместе с фоном: перенос режет коробку на куски, и рамка каждого куска
    /// своя. Коробки в раскладке у такого `<span>` нет — иначе его текст
    /// перестаёт переноситься вместе с абзацем.
    pub inline_border: Option<(Color, [f32; 4])>,
    /// Поля вокруг фона строчного бокса: `padding` у `<span>`, по четырём
    /// сторонам в порядке `[верх, право, низ, лево]`.
    pub inline_pad: Option<[f32; 4]>,
    /// Скругление фона строчного бокса.
    pub inline_radius: Option<f32>,
    /// Сплошная заливка предка с `background-clip: text` (css-backgrounds-4
    /// §background-clip): глифы поддерева красятся цветом текста ПОВЕРХ неё.
    /// Ставит и снимает `inline::inherit`; `None` — такого предка нет.
    pub text_clip_fill: Option<Color>,
    /// Цвет текста ДО наложения на `text_clip_fill`: его получают внепоточные
    /// потомки — в геометрию текста они не входят.
    pub text_clip_raw: Option<Color>,
    /// `background-clip`: до какого края красится фон. `None` — до внешнего
    /// края рамки, как по умолчанию в CSS.
    pub bg_clip: Option<BgClip>,
    /// `border-image`: картинка вместо рамки.
    pub border_image: Option<BorderImage>,
    /// `background-origin`: от какого края коробки отсчитывается фоновая
    /// картинка. `None` — от внутреннего края рамки, как по умолчанию в CSS.
    pub bg_origin: Option<BgClip>,
    /// `overflow-clip-margin`: на сколько обрезка отступает НАРУЖУ от коробки.
    /// Узел, чей фон красит КАНВАС (CSS 2.2 §14.2): корневой html, а без
    /// его фона — body. Ставится сборкой документа, не каскадом.
    pub(crate) canvas_bg: bool,
    /// Письмо `<body>` НЕ стало письмом области просмотра: обособление есть
    /// либо на `<html>`, либо на самом `<body>`, и распространение свойств
    /// тела наружу выключено (css-contain-2 §containment-types: «when any
    /// containments are active on either the HTML html or body elements,
    /// propagation of properties from the body element to the initial
    /// containing block, the viewport, or the canvas background, is
    /// disabled»). Вычисленное письмо тела при этом остаётся при нём
    /// (css-writing-modes §3.1 — распространяется только used корневой
    /// коробки), поэтому отличить главный поток по одному лишь письму тела
    /// нельзя, и признак приходится нести пометкой. Ставится сборкой
    /// документа, не каскадом.
    pub(crate) wm_contained: bool,
    /// Коробка КОРНЯ документа: её содержащий блок — начальный, и высота его
    /// определена всегда (§10.5). Ставится вместе с пометкой канваса, чтобы
    /// снимаемая обёртка уносила признак с собой.
    pub(crate) root_box: bool,
    /// ОПРЕДЕЛЕНА ли высота содержащего блока: от неё зависит, считается ли
    /// доля высоты вообще (§10.5 — иначе значение равно `auto`).
    pub(crate) cb_height_def: bool,
    /// Режим quirks: высота, от которой ДЕТИ этой коробки решают долю
    /// высоты при неопределённом содержащем блоке (Quirks Mode §3.5 «The
    /// percentage height calculation quirk» — ближайший предок с не-`auto`
    /// высотой). `None` — квирка нет или опоры не нашлось.
    pub(crate) quirk_pct_base: Option<f32>,
    /// `calc-size(<basis>, <expr>)` у `width`, `height`, `min-width`,
    /// `min-height` (css-values-5 §calc-size): `(mul, add, max, min)` над
    /// размером основы-ключевого слова; само свойство при этом `auto`, а
    /// выражение применяет раскладка (`taffy::Style::calc_size`).
    pub(crate) calc_size: [Option<(f32, f32, f32, f32)>; 4],
    /// Коробка РАСТЯНУТА раскладкой: элемент гибкого контейнера или сетки без
    /// своей высоты получает её от полосы, и для потомков она определена.
    pub(crate) stretched: bool,
    /// Элемент КОЛОНКИ гибкого контейнера: определён ли главный размер
    /// контейнера (css-flexbox-1 §9.8 п.1). `None` — не элемент колонки.
    pub(crate) flex_main_def: Option<bool>,
    /// Элемент ГИБКОГО контейнера (родитель `display: flex | inline-flex`).
    /// Ставится сборкой детей ряда/колонки в `render::blocks`, не каскадом и
    /// не наследуется (`inline::inherit` клонирует СВОЙ стиль ребёнка). Нужен
    /// таблице: у неё гибким элементом становится обёртка с подписями
    /// (css-flexbox-1 §4).
    pub(crate) flex_item: bool,
    /// Floats of this ordinary block reach nothing after it (`dom::float_tail`):
    /// its float host may use in-flow auto height (CSS 2.1 §10.6.3).
    pub(crate) float_tail: bool,
    /// Довод `fit-content(<length-percentage>)` у `width`, `min-width`,
    /// `max-width` (по порядку); само значение остаётся `Len::FitContent`.
    pub(crate) fit_arg: [Option<Len>; 3],
    /// The size slot came from `stretch` (stored as `Len::Pct(1.0)`), in the
    /// order width, height, min-width, max-width, min-height, max-height.
    /// css-sizing-4 §4.1 fills the containing block with the margin box, so
    /// it is not a percentage for the content-box contract.
    pub(crate) stretch_size: [bool; 6],
    /// Intrinsic min/max constraints rewritten as a preferred keyword retain their sizing wrapper.
    pub(crate) intrinsic_wrapper_required: bool,
    pub clip_margin: Option<f32>,
    /// Коробка отсчёта края обрезки: 0 content, 1 padding, 2 border;
    /// None — умолчание (padding-box).
    pub clip_margin_box: Option<u8>,
    /// `clip-path: polygon(…)`: вершины в долях или точках коробки.
    pub clip_polygon: Option<Vec<(Len, Len)>>,
    /// Правило намотки полигона `clip-path: polygon(evenodd, …)`.
    pub clip_polygon_evenodd: bool,
    /// `mix-blend-mode`: как слой смешивается с тем, что под ним.
    pub blend: Option<u8>,
    /// `isolation: isolate`: поддерево смешивается внутри себя, а с кадром —
    /// уже готовой картинкой.
    pub isolate: Option<bool>,
    /// `will-change` (css-will-change-1): разряды `wc::*`; ноль — `auto`.
    /// Слоёв композитора у нас нет, поэтому от обещания остаётся ровно то,
    /// что дало бы само свойство: содержащий блок и контекст наложения.
    pub will_change: u8,
    /// `font-stretch` — ширина начертания в процентах от обычной.
    ///
    /// Это НЕ возможность OpenType: узкое начертание — отдельный шрифт
    /// семейства, и выбирается он при подборе.
    pub font_stretch: Option<f32>,
    /// `font-size-adjust` (css-fonts-5): метрика (0 ex-height, 1 cap-height,
    /// 2 ch-width, 3 ic-width, 4 ic-height; `u8::MAX` — `none`) и желаемая
    /// доля кегля (`NaN` — `from-font`). Наследуется.
    pub font_size_adjust: Option<(u8, f32)>,
    /// Вычисленный кегль и заданная высота строки ДО подгонки кегля. Детям
    /// уходит ИМЕННО вычисленный кегль («otherwise the effect would
    /// compound»), и от него же считаются `em` и числовой `line-height`.
    /// Пусто, пока используемый кегль равен вычисленному.
    pub font_adjust_base: Option<(f32, Option<Len>)>,
    /// `tab-size` — во сколько пробелов раскрывается табуляция.
    pub tab_size: Option<f32>,
    /// `tab-size` в ДЛИНЕ: шаг табуляции задан не числом знаков, а величиной.
    /// Наследуется абсолютным (`tab-size-inheritance-001`), поэтому к детям
    /// уходит уже в точках — перевод делает `inline::inherit`.
    pub tab_size_len: Option<Len>,
    /// `contain: paint` — содержимое обрезается по коробке.
    pub contain_paint: Option<bool>,
    /// `contain: size` — коробка меряется ПУСТОЙ (css-contain-1 §3):
    /// размер задают явные свойства и `contain-intrinsic-size`.
    pub contain_size: Option<bool>,
    /// `contain: inline-size` — обособлена только СТРОЧНАЯ ось
    /// (css-contain-2 §inline-size): содержимое не влияет на неё, но
    /// блочную ось по-прежнему задаёт.
    pub contain_inline_size: Option<bool>,
    /// `contain: layout|content` — независимый контекст форматирования.
    pub contain_layout: Option<bool>,
    /// `display: flow-root` — свой контекст форматирования (коробка Block).
    pub flow_root: Option<bool>,
    /// Статичная блочная коробка в потоке (не строчная, не таблица, не
    /// поле формы, не float): её чистый px-`transform` раскладка берёт на
    /// себя (`folded_shift`). Ставит `dom` после `finish_inline_display`.
    pub plain_block_box: bool,
    /// `display: inline` дословно (не inline-block): §9.7/§10.2 дорешиваются
    /// после каскада — см. `dom::finish_inline_display`.
    pub inline_display: Option<bool>,
    /// Абсолют/фиксированный, чей `display` до блокификации (§9.7) был
    /// строчного уровня (`inline-block`, `inline-flex`, …): статическая
    /// позиция считается для гипотетической коробки «если бы position был
    /// static» (CSS 2.1 §10.3.7, §10.6.4), то есть В СТРОКЕ. Ставит `dom`.
    pub abs_inline_level: bool,
    /// `display: run-in` — вбегание решает `dom::fold_run_ins`.
    pub run_in: Option<bool>,
    /// Есть ли выше по дереву коробка, устанавливающая содержащий блок для
    /// внепоточных потомков (§10.1 п.4). Ставится при наследовании: сам
    /// каскад предков не видит.
    pub(crate) cb_ancestor: bool,
    /// An ancestor is a multi-column container: the box may be fragmented
    /// across columns (css-break-3 §box-splitting). Set by inheritance.
    pub(crate) in_multicol: bool,
    /// Есть ли выше по дереву корень подложки (filter-effects-2
    /// §BackdropRoot): прозрачность, фильтр, маска, clip-path, смешивание,
    /// `backdrop-filter`, `will-change` с ними. Ставится при наследовании.
    pub(crate) backdrop_root_above: bool,
    /// `will-change` называет свойство, создающее корень подложки.
    pub(crate) will_change_root: bool,
    /// `backdrop-filter` задан (не `none`): корень подложки при ЛЮБОМ списке,
    /// даже тождественном `invert(0)`, у которого матрицы нет
    /// (filter-effects-2 Overview.bs:119; Blink
    /// paint_property_tree_builder.cc:1846 `!BackdropFilter().IsEmpty()`).
    pub(crate) backdrop_filter_set: bool,
    /// `view-transition-name` не `none` — тоже корень подложки
    /// (css-view-transitions-1 Overview.bs:582).
    pub(crate) vt_name: bool,
    /// `backdrop-filter: url(#id)` — ссылка на SVG `<filter>`; сводится к
    /// матрице 4×5 на отрисовке (`render::svg_filter_matrix`).
    pub(crate) backdrop_ref: Option<String>,
    /// `display: table-caption` — метка для таблицы.
    pub is_caption: Option<bool>,
    /// Род группы рядов: 0 — шапка, 1 — тело, 2 — подвал. `Display` у всех
    /// трёх ОДИН (`TableRowGroup`), раскладка у них одинаковая, — а §17.5.3
    /// требует переставить шапку вверх, подвал вниз. Различить их по
    /// `Display` нечем, поэтому род хранится отдельно.
    pub row_group_kind: Option<u8>,
    /// Колоночная роль: 0 — `display: table-column`, 1 —
    /// `table-column-group`. `Display` при этом ОСТАЁТСЯ `None`: коробки
    /// колонка не порождает (§17.2.1), и весь поток обходит её ровно как
    /// раньше. Метка — единственное, что сохраняет узел в дереве: из него
    /// берутся ширина дорожки (§17.5.2.1), слой краски (§17.5.1) и рамка
    /// для разбора сросшихся кромок (§17.6.2.1).
    pub col_role: Option<u8>,
    /// `contain: style` — счётчики и кавычки не выходят из поддерева.
    pub contain_style: Option<bool>,
    /// `contain-intrinsic-size`: подменная своя величина (css-sizing-5 §5).
    pub contain_intrinsic: (Option<f32>, Option<f32>),
    /// `content-visibility: hidden` — детей не собирать вовсе.
    pub skip_content: Option<bool>,
    /// `clip-path`/`mask`: обрезка по кругу или скруглённому прямоугольнику.
    /// Хранится долей радиуса от меньшей стороны либо радиусом в точках.
    pub clip_round: Option<f32>,
    /// The uniform `round` radius of `inset()`/`rect()`/`xywh()` as written
    /// (points or a percentage of the reference box).
    pub clip_round_len: Option<Len>,
    /// `clip-path: circle(...)|ellipse(...)` с параметрами: сырые аргументы
    /// формы (`shape:circle(...)`). Радиусы и центр зависят от размера
    /// коробки — он известен только отрисовке, поэтому форма растрируется
    /// маской буфера группы (см. `background::source`).
    pub clip_shape: Option<String>,
    /// `mask-size`: размер плитки маски; None — auto (интринзик картинки).
    pub mask_size: Option<(Len, Len)>,
    /// `mask-repeat`: пооосный запрет мощения (no-x, no-y) — ПЕРВОГО слоя.
    pub mask_no_repeat: Option<(bool, bool)>,
    /// То же ПО СЛОЯМ (css-masking-1 §7.6, `<repeat-style>#`): запись
    /// `no-repeat, repeat` задаёт свою укладку каждому слою. Список короче
    /// набора слоёв повторяется (css-backgrounds-3 §2.2).
    pub mask_repeat_list: Option<Vec<(bool, bool)>>,
    /// Per-layer `space`/`round` axes (css-masking-1 §7.6 →
    /// css-backgrounds-3 §3.4): 2 space, 3 round, anything else as
    /// `mask_repeat_list` says.
    pub mask_repeat_modes: Option<Vec<(u8, u8)>>,
    /// `mask-size: contain|cover` (1|2): вписывание по интринзику.
    pub mask_fit: Option<u8>,
    /// `mask-mode: luminance` — маскирует светимость, а не альфа.
    pub mask_luminance: Option<bool>,
    /// `mask-mode: alpha` — альфа и для ссылки на `<mask>` (css-masking-1
    /// §7.2: `match-source` берёт `mask-type` определения).
    pub mask_alpha_mode: Option<bool>,
    /// `mask-type: alpha` у элемента `<mask>` (css-masking-1 §7.16).
    pub mask_type_alpha: Option<bool>,
    /// `mask-origin`: коробка укладки плитки (0 border, 2 padding, 3 content).
    pub mask_origin: Option<u8>,
    /// Готовые коробки маски в CSS-точках (укладка t/r/b/l от коробки
    /// слоя внутрь; окраска — то же либо None = без обрезки) — для SVG-детей,
    /// у которых fill-/stroke-/view-box считает `svg::masked_layers`, а не
    /// рамка и отбивка (`render::grouped`).
    pub mask_box_override: Option<([f32; 4], Option<[f32; 4]>)>,
    /// Блок, вынесенный расщеплением строчного хозяина (block-in-inline,
    /// `render::blocks`): в дереве отрисовки он брат хозяина, а по DOM — его
    /// ребёнок. Объёмный контекст и перспектива деда на него не действуют:
    /// плоский строчный хозяин — лист контекста, поддерево сплющивается в его
    /// плоскость (css-transforms-2 §3d-rendering-context; §perspective — только
    /// прямые дети). Ставится при выносе, читает `render::transformed`.
    pub hoisted_block: bool,
    /// Пользовательская единица SVG-ребёнка в CSS-точках (масштаб `viewBox`
    /// или `zoom`); 0 — не задано (= 1). Интринзик плитки маски у такого
    /// ребёнка считается в его единицах (`interact::Grouped::mask_scale`).
    pub mask_user_scale: f32,
    /// `mask-clip`: коробка окраски маски; вне её элемент скрыт. 255 — no-clip.
    pub mask_clip: Option<u8>,
    /// `mask-composite` по слоям: 0 add, 1 subtract, 2 intersect, 3 exclude.
    pub mask_composite: Option<Vec<u8>>,
    /// `clip: rect(t r b l)` (CSS 2.1 §11.1.2, только absolute): координаты
    /// видимой области от углов border-box; None в позиции — auto (край).
    pub clip_rect: Option<[Option<f32>; 4]>,
    /// Тот же прямоугольник, но КАК НАПИСАН: единицы шрифта на разборе ещё не
    /// меряются, а `clip: rect(1em, …)` без них читался как `auto` и не
    /// обрезал вовсе (`visufx/clip-079/080/091/092`). Сводится к точкам в
    /// `resolve_em`, где кегль и метрики семейства уже известны.
    pub clip_len: Option<[Option<Len>; 4]>,
    /// `clip-path: inset(t r b l ...)`: срезы краёв видимой области.
    pub clip_inset: Option<[Len; 4]>,
    /// `clip-path: rect(t r b l)` — координаты КРАЁВ от верхнего-левого
    /// угла; None = auto (край коробки).
    pub clip_edges: Option<[Option<Len>; 4]>,
    /// `clip-path: xywh(x y w h)` — прямоугольник от угла.
    pub clip_xywh: Option<[Len; 4]>,
    /// `column-fill: auto` — колонки заполняются по очереди, без баланса.
    pub column_fill_auto: Option<bool>,
    /// `scroll-marker-group` (css-overflow-5): `Some(true)` — группа маркеров
    /// ПЕРЕД скроллером (`before`), `Some(false)` — после (`after`), `None` —
    /// `none`. Не наследуется.
    pub scroll_marker_group: Option<bool>,
    /// `column-span: all` — блок растянут на все колонки.
    pub column_span: Option<bool>,
    /// `page: <custom-ident>` — именованная страница (css-page-3 §"Using named
    /// pages"). `auto` хранится отсутствием значения: используемое значение
    /// берётся у ближайшего предка с именем (там же, шаг 1 алгоритма).
    pub page: Option<String>,
    /// ★ ЗАМЕРЕНО И ОТКАЧЕНО (07.09, v153, `scout-boxdeco-2026-09.md`):
    /// `box-decoration-break: clone` — украшение на каждом фрагменте
    /// (21 хунк: разбор, `Kid::clone_dec`, ветка в `fill_at`, `frags_of`,
    /// `clone_fragment`). Срез `L-brk` 2874: +2/−2 при ожидании +6…+19 —
    /// `clone-004`, `-012` взяты, но `clone-005.tentative` 0.00 → 99.00 и
    /// `clone-007` 0.00 → 2.08. Ветка `clone` в `fill_at` ломает уже
    /// работавший `slice` у вложенных случаев; нужен отдельный проход
    /// планирования фрагментов, а не правка общей укладки.
    /// `break-inside: avoid*` — коробку нельзя разрывать между колонками и
    /// страницами (css-break-3 §4.1). Свойство не разбиралось вовсе, и
    /// отличить монолит от обычной коробки было нечем.
    pub break_inside_avoid: bool,
    /// `box-decoration-break: clone` (css-break-4 §break-decoration): «Each
    /// box fragment is independently wrapped with the border, padding, and
    /// margin … The background is drawn independently in each fragment».
    /// Не наследуется; начальное `slice` = `false`.
    pub bdb_clone: bool,
    /// `break-before`/`break-after` (css-break-4 §3.1): принудительный разрыв
    /// колонки/страницы перед или после коробки.
    pub break_before_force: bool,
    pub break_after_force: bool,
    /// Те же свойства со ЗАПРЕЩАЮЩИМИ значениями — css-break-4 §3.1 «avoid
    /// break values»: `avoid`, `avoid-page`, `avoid-column`, `avoid-region`.
    /// Правило 1 §4.3: «A fragmented flow may break at a class A break point
    /// only if all the break-after and break-before values applicable to this
    /// break point allow it». Разбирались ТОЛЬКО принудительные значения, и
    /// сообщить движку запрет разрыва МЕЖДУ соседями было нечем.
    pub break_before_avoid: bool,
    pub break_after_avoid: bool,
    /// `margin-trim` (css-box-4 §margin-trim): биты обрезаемых ЛОГИЧЕСКИХ
    /// краёв: 1 — `block-start`, 2 — `block-end`, 4 — `inline-start`,
    /// 8 — `inline-end`. Начальное `none` (0). Блочный контейнер исполняет
    /// только блочные биты (`render::collapse_margins`), гибкий и сетка —
    /// все четыре (раскладка, `apply` переводит их в физические).
    pub margin_trim: u8,
    /// `zoom` (css-viewport-1 §zoom-property): СВОЙ множитель элемента, как
    /// написан; `None` — не задан. `0`/`0%` по спеке читаются единицей.
    /// Читает его ТОЛЬКО проход `zoom::resolve` после каскада.
    pub zoom: Option<f32>,
    /// Действующий зум («effective zoom», §599): произведение по цепочке
    /// предков вместе со своим. `None` ≡ 1 — выведенный `Default`
    /// тождество, и страница без `zoom` не несёт ни множителя, ни ветки.
    /// Ставится проходом `zoom::resolve` на каждый элемент под зумом; в
    /// слитый стиль попадает через `own.clone()` в `inline::inherit` —
    /// своей строки там не имеет. Читатели шага 2: природный размер
    /// картинки, `resolve_viewport`.
    pub zoom_eff: Option<f32>,
    /// `column-rule-*`: линейка между колонками.
    pub column_rule_width: Option<Len>,
    pub column_rule_visible: Option<bool>,
    pub column_rule_color: Option<Color>,
    /// `row-rule-*` (css-gaps-1 §color-style-width): линейка в ПОПЕРЕЧНОМ
    /// промежутке сетки/гибкого контейнера. Начальные значения те же, что у
    /// `column-rule-*`: `currentcolor`, `none`, `medium` — то есть без
    /// заданного стиля линейки нет.
    pub row_rule_width: Option<Len>,
    pub row_rule_visible: Option<bool>,
    pub row_rule_color: Option<Color>,
    /// `column-rule-break`/`row-rule-break` (css-gaps-1 §break): 0 — `none`,
    /// 1 — `normal` (начальное), 2 — `intersection`. Шаг 1 разбирает
    /// значение, но рисует всегда непрерывно: в сетке БЕЗ спанов все стыки
    /// крестовые, и `normal` по спеке проходит сквозь них.
    pub column_rule_break: Option<u8>,
    pub row_rule_break: Option<u8>,
    /// Списки значений линеек по промежуткам (css-gaps-1 §lists); `None` —
    /// значение одно и лежит в скалярных полях выше. Цвет `None` в списке —
    /// `currentcolor`.
    pub column_rule_widths: Option<GapList<Len>>,
    pub column_rule_styles: Option<GapList<bool>>,
    /// Single `double` rule style (css-gaps-1 §color-style-width: line styles
    /// as for borders) — painted as two lines; other styles stay solid.
    pub column_rule_double: bool,
    pub row_rule_double: bool,
    pub column_rule_colors: Option<GapList<Option<Color>>>,
    pub row_rule_widths: Option<GapList<Len>>,
    pub row_rule_styles: Option<GapList<bool>>,
    pub row_rule_colors: Option<GapList<Option<Color>>>,
    /// §inset: [cap-start, cap-end, junction-start, junction-end]; начальное 0.
    pub column_rule_inset: Option<[GapInset; 4]>,
    pub row_rule_inset: Option<[GapInset; 4]>,
    /// §visibility-items: 0 `normal`, 1 `all`, 2 `around`, 3 `between`.
    pub column_rule_visibility: Option<u8>,
    pub row_rule_visibility: Option<u8>,
    /// `rule-overlap: column-over-row` — колонки поверх рядов.
    pub rule_column_over_row: Option<bool>,
    /// `shape-outside`: сырая запись формы обтекания плавающего блока.
    pub shape_outside: Option<String>,
    /// `shape-margin`: поле вокруг формы обтекания; доля — от ширины
    /// содержащего блока.
    pub shape_margin: Option<Len>,
    /// `shape-image-threshold`: порог альфы для формы из картинки.
    pub shape_threshold: Option<f32>,
    /// Вырезы обтекания для абзацев ПОД этим элементом: формы слева и
    /// справа от верха первого абзаца (заполняет сборка shape-flow).
    pub flow_shapes: Option<
        std::sync::Arc<(
            Vec<crate::layout::float::shapes::FloatShape>,
            Vec<crate::layout::float::shapes::FloatShape>,
        )>,
    >,
    /// `mask-position`: смещение плитки; доля — от свободного места
    /// (коробка минус плитка), как у `background-position`.
    pub mask_pos: Option<(Len, Len)>,
    /// Смещение отсчитано от ПРАВОГО/НИЖНЕГО края (`right 30px bottom 25px`).
    pub mask_pos_far: (bool, bool),
    /// `mask-position` ПО СЛОЯМ (css-masking-1 §7.7, `<position>#`):
    /// `(x, y, от правого края, от нижнего края)`. Список короче набора
    /// слоёв повторяется (css-backgrounds-3 §2.2).
    pub mask_pos_list: Option<Vec<(Len, Len, bool, bool)>>,
    /// Эллиптические радиусы углов (`border-radius: H / V`), tl/tr/br/bl:
    /// растеризатор круглит только окружностью — такой угол уходит
    /// альфа-маской буфера группы (`shape:rrect(...)`).
    pub radius_ell: Option<[Option<(Len, Len)>; 4]>,
    /// Форма углов `corner-shape` (css-borders-4 §corner-shaping): параметр
    /// суперэллипса K по углам tl/tr/br/bl — `round`=1, `squircle`=2,
    /// `square`=+∞, `bevel`=0, `scoop`=−1, `notch`=−∞, `superellipse(K)`.
    /// `None` — все углы круглые (начальное значение). Угол с K≠1 при
    /// ненулевом радиусе рисуется растровой маской (`Computed::corner_shaped`).
    pub corner_shape: Option<[f32; 4]>,
    /// `border-shape` (css-borders-4 §border-shape); `None` — начальное `none`.
    /// Контур режет буфер группы (`render::grouped`), рамку красит слой
    /// (`render::decorations`), `border-radius` при этом игнорируется.
    pub border_shape: Option<BorderShape>,
    /// `filter`: цветовые преобразования, применённые к собственным цветам.
    pub filter: Option<Filter>,
    /// `filter: url(#id)` — ссылка на SVG-`<filter>` документа; рисуется
    /// растровым слоем поверх коробки (`interact::FilterLayer`).
    pub filter_ref: Option<String>,
    /// `filter: drop-shadow(...)` — тень фильтра; у коробки со сплошным
    /// фоном становится внешней тенью (`inline::inherit`).
    pub drop_shadow: Option<Shadow>,

    /// `background-image: url(...)` — ссылка на картинку-заливку.
    pub bg_image: Option<String>,
    pub bg_size: BgSize,
    pub bg_pos: BgPos,
    /// `object-position` замещаемого содержимого (css-images-3 §5.2).
    pub object_position: Option<BgPos>,
    /// `object-view-box` (css-images-4 §object-view-box): видимая область
    /// природного объекта как вырез `inset(top right bottom left)` в точках
    /// или долях природного размера — `rect()` и `xywh()` сводятся к нему
    /// при отрисовке (`render::view_box_rect`). Флаг — вид записи:
    /// 0 `inset`, 1 `rect`, 2 `xywh`.
    pub(crate) object_view_box: Option<(u8, [Len; 4])>,
    /// Сырые СПИСКИ фоновых свойств со слоями через запятую: (свойство,
    /// запись). Поля `bg_*` несут верхний слой; все слои строит `bg_layers`.
    pub(crate) bg_lists: Vec<(String, String)>,
    pub bg_repeat: Option<BgRepeat>,
    /// `content` псевдоэлемента — СПИСОК составляющих (css-content-3 §2):
    /// строки, `counter()`, `counters()`, `attr()` в любом порядке.
    pub content: Option<Vec<ContentItem>>,
    /// `quotes` (css-content-3 §4.1): пары кавычек по уровням вложенности.
    /// `None` inherits; `Some(None)` suppresses marks but keeps nesting depth.
    /// `Some(Some(empty))` is explicit auto; nonempty pairs are a custom system.
    /// Tree traversal resolves inheritance before generating pseudo content.
    pub quotes: Option<Option<Vec<(String, String)>>>,
    /// `counter-reset` — обнулить счётчик с этого узла.
    pub counter_reset: Option<String>,
    /// `counter-increment` — увеличить счётчик на этом узле.
    pub counter_increment: Option<String>,
    /// `counter-set` — присвоить счётчику значение (css-lists-3 §5).
    pub counter_set: Option<String>,
    /// Возможности шрифта (`font-feature-settings`, `font-variant`).
    pub font_features: Vec<(String, u32)>,
    /// Значение СВОЙСТВА `font-feature-settings` целиком (`normal` — пустой
    /// список). Отдельно от `font_features`: по css-fonts-4 §7.2 оно старше
    /// `font-variant-*` при ЛЮБОМ порядке объявлений и наследуется своим
    /// значением, а не пропадает, стоит ребёнку задать `font-variant`
    /// (`font-variant-04`).
    pub font_settings: Option<Vec<(String, u32)>>,
    /// Font-specific names stay unresolved until the used family is known
    /// (CSS Fonts 4 §font-variant-alternates-prop).
    pub font_alternates: Option<crate::text::fonts::alternates::Alternates>,
    /// `font-synthesis-weight|style|small-caps: none` — подмена начертания
    /// запрещена (css-fonts-4 §6.5). Ложь = `none`, пусто = `auto`.
    pub font_synth: (Option<bool>, Option<bool>, Option<bool>),
    /// `font-kerning`: 0 `none`, 1 `normal`, 2 `auto`; inherits independently
    /// of font-variant and resolves before font-feature-settings.
    pub font_kerning: Option<u8>,
    /// Знак акцента (`text-emphasis-style`, css-text-decor-3 §5): рисуется
    /// над каждым знаком базы, как надстрочная аннотация руби.
    pub text_emphasis: Option<String>,
    /// Акцент СНИЗУ (`text-emphasis-position: under`).
    pub emphasis_under: bool,
    /// `text-emphasis-color` (css-text-decor-3 §5.2); пусто — `currentColor`.
    pub emphasis_color: Option<Color>,
    /// `text-decoration-line` самой коробки (биты `DECOR_*`); пусто — не
    /// задано (`none`). Не наследуется: потомкам линии достаются через
    /// `decors` (css-text-decor-3 §2 «propagated»).
    pub td_lines: Option<u8>,
    /// `text-decoration-style` (не наследуется).
    pub td_style: Option<DecorStyle>,
    /// `text-decoration-color`; пусто — `currentColor` (не наследуется).
    pub td_color: Option<Color>,
    /// `text-decoration-thickness` (не наследуется).
    pub td_thickness: Option<DecorLen>,
    /// `text-decoration-inset` (css-text-decor-4 §4.1, не наследуется):
    /// `None` — 0, `Some(None)` — `auto`.
    pub td_inset: Option<Option<[DecorLen; 2]>>,
    /// `text-underline-offset` (наследуется).
    pub underline_offset: Option<DecorLen>,
    /// `text-underline-position` (наследуется), биты `UPOS_*`.
    pub underline_pos: Option<u8>,
    /// Блочный тег (`<p>`, `<div>`…): при пустом `display` коробка блочная
    /// (`dom.rs`, не наследуется) — украшениям нужна своя строчная коробка.
    pub block_tag: bool,
    /// `text-decoration-skip-ink` (наследуется): 0 `none`, 1 `auto`, 2 `all`.
    pub skip_ink: Option<u8>,
    /// `text-decoration-skip-spaces` (наследуется): 1 `start`, 2 `end`,
    /// 4 `all`, 0 `none`; пусто — начальное `start end`.
    pub skip_spaces: Option<u8>,
    /// Украшения, наложенные на текст коробки её предками и ею самой
    /// (css-text-decor-3 §2.1), от внешнего к внутреннему.
    pub decors: Vec<Decor>,
    /// `ruby-position` (css-ruby-1 §4.1): `Some(true)` — аннотация ПОД базой
    /// (`under`), `Some(false)` — над (`over`/`alternate`/`inter-character`),
    /// `None` — не задано. Наследуется (`inline::inherit`). Прежде делил флаг
    /// с акцентом, и `text-emphasis-position: under` переворачивал руби.
    pub ruby_under: Option<bool>,
    /// `ruby-align` (css-ruby-1 §4.3); `None` — начальное `space-around`.
    pub ruby_align: Option<RubyAlign>,
    /// `ruby-overhang` (css-ruby-1 §4.4); `None` — начальное `auto`. Наследуется.
    pub ruby_overhang: Option<RubyOverhang>,
    /// CSS Ruby §ruby-merge: 0 separate, 1 merge, 2 auto.
    pub ruby_merge: Option<u8>,
    /// Роль руби-коробки из `display: ruby*` (css-ruby-1 §2.1). Не
    /// наследуется. `display` при этом остаётся строчным (`InlineBlock` +
    /// `inline_display`), у `block ruby` — `Block`: все `match` по `Display`
    /// остаются как есть, роль читается отдельно.
    pub ruby_role: Option<RubyRole>,
    /// `caret-color` поля ввода.
    pub caret_color: Option<Color>,
    /// `accent-color` флажков и переключателей.
    pub accent_color: Option<Color>,
}
