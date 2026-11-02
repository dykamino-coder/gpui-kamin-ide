//! CSS-операнды группы grid; значение за общим COW-указателем.
use super::super::*;
use crate::style::values::value::Len;
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ResetGrid {
    pub grid_cols: Option<u16>,
    /// Список дорожек, если он выразим: `auto`, `1fr`, px, `%`, `minmax()`.
    pub grid_tracks: Option<Vec<TrackSize>>,
    /// `grid-template-areas`: сетка имён по строкам.
    ///
    /// Именованных областей нет ни в GPUI, ни в taffy, поэтому имена
    /// разворачиваются в номера линий при сборке дерева — там, где известны и
    /// контейнер, и его дети.
    pub grid_areas: Option<Vec<Vec<String>>>,
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
    /// скалярном `subgrid`. Перевод среза на поосевые признаки без второго
    /// прохода ломает наследование дорожек (заметка у `subgrid_inhibited`).
    pub subgrid_cols: bool,
    pub subgrid_rows: bool,
    pub auto_repeat_cols: Option<AutoRepeat>,
    pub auto_repeat_rows: Option<AutoRepeat>,
    /// Тело авто-повтора ДОРОЖКА ЗА ДОРОЖКОЙ: `repeat(auto-fill, max-content
    /// min-content)` — это два РАЗНЫХ размера, а не два одинаковых.
    /// css-grid-3 §7.2.1 («The hypothetical size of each track in the repeat()
    /// listing is given by the largest track corresponding to that entry (by
    /// index)», `csswg-drafts/css-grid-3/Overview.bs:469-471`) требует считать
    /// каждую запись тела по ЕЁ функции; скалярные `track`/`intrinsic_min`
    /// этого не выражают. Пишется РЯДОМ с `AutoRepeat` и `grid_tracks` не
    /// трогает: тот путь ломает `auto-repeat-auto-011` и родню (заметка в
    /// `props/grid.rs`).
    pub auto_repeat_body_cols: Option<Vec<TrackSize>>,
    pub auto_repeat_body_rows: Option<Vec<TrackSize>>,
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
}
