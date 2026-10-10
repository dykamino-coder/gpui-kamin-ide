//! Содержащий блок абсолютных коробок (поздняя расстановка).
// owner: A

use gpui::{AnyElement, Bounds, Pixels};
mod late;
pub use late::LatePlace;
pub use late::spot_place;
pub use late::spot_probe;
mod hang;
pub use hang::InlineStartHang;
mod layers;
pub(crate) use layers::ICB;
pub(crate) use layers::LATE;
pub use layers::icb_active;
pub use layers::icb_close;
pub use layers::icb_open;
use layers::icb_place;
pub use layers::icb_push;
pub use layers::late_close;
pub use layers::late_open;
pub use layers::late_pending;
pub use layers::late_push;

#[derive(Clone, Copy, Default)]
pub struct Spot {
    pub hole: Option<Bounds<Pixels>>,
    /// Высота строки, если элемент блочный: его статическая позиция — начало
    /// СЛЕДУЮЩЕЙ строки, а не точка в текущей (CSS 2.1 §10.6.4).
    pub next_line: Option<f32>,
    /// Письмо справа налево: начало строки у такого блока — правый край.
    pub rtl: bool,
    /// Вертикальное письмо: строки идут поперёк, поэтому «следующая строка» —
    /// сдвиг по горизонтали, а не по вертикали.
    pub vertical: bool,
    /// Вертикальное письмо справа налево: следующая строка левее текущей.
    pub vertical_rl: bool,
    /// СОБСТВЕННОЕ письмо элемента вертикально (поток вокруг — нет):
    /// статическая позиция такого абсолюта смещена на его ширину —
    /// блок vrl вешается своим блок-началом, правым краем
    /// (abs-pos-border-offset-003, ref: left 55 = контент-лево + width).
    pub own_vertical: bool,
    /// Коробка ЗАМЕЩАЕМАЯ. Голый заместитель решает уравнение §10.6.4 —
    /// «Absolutely positioned, non-replaced elements»; у замещаемых своё,
    /// §10.6.2, где высота берётся от природного размера, а не от
    /// содержащего блока. ★ ЗАМЕРЕНО: без этого различения абсолютный
    /// `<iframe>` с процентной высотой обваливался с 0.02 в «красное видно»
    /// (`absolute-replaced-height-012/019/026/033`).
    pub replaced: bool,
    /// Ось, которую уже разрешил СОДЕРЖАЩИЙ БЛОК (край в `inset` задан):
    /// сдвиг по ней не нужен. `(x, y)`. Умолчание `(false, false)` — правятся
    /// обе оси, то есть прежнее поведение верхнего слоя.
    ///
    /// Слою начального содержащего блока нужна ровно ПУСТАЯ ось: заданную
    /// считает раскладка от области просмотра, а статическая позиция нужна
    /// только там, где обе стороны `auto` (§10.3.7, §10.6.4).
    pub fixed_axes: (bool, bool),
    /// Толщина колонки строки в повёрнутом абзаце: на неё отступает экранная
    /// точка дырки от правого края рамки. Ноль — коробка вне поворота.
    pub line_thickness: f32,
    /// Дырка уже отображена из повёрнутого абзаца в экранную систему
    /// (`vt_map`): дальше её ставит не горизонтальный рукав щупа, а свой —
    /// сдвиг только по свободной оси, при `rtl` — от НИЖНЕГО края.
    pub rotated: bool,
    /// Поле по СВОБОДНОЙ оси, в точках. Базовый сдвиг ставит коробку ровно в
    /// дырку, а по CSS от статической позиции её отодвигает собственное поле.
    pub free_margin: (f32, f32),
    /// Доля вдоль СТРОЧНОЙ оси, где стоит статическая точка: 0 — начало
    /// строки, 0.5 — середина, 1 — конец. Её задаёт `text-align` содержащего
    /// блока: гипотетическая коробка строчного абсолюта лежит в строке и
    /// выравнивается вместе с ней (CSS 2.1 §10.3.7 «где коробка была бы при
    /// `position: static`», css-align-3 §abspos). `None` — прежний ход:
    /// начало строки по `direction` (0 при ltr, 1 при rtl).
    pub line_align: Option<f32>,
    /// Щуп — БЛОЧНАЯ распорка во всю ширину содержащего блока
    /// (`spot_probe(_, true)`), а не точечный щуп в строке.
    ///
    /// Порода щупа решает, вешается ли коробка при `direction: rtl` на
    /// статическую точку своим ПРАВЫМ краем (§10.3.7: «otherwise set 'right'
    /// to the static position»). У распорки — да: её правый край и есть
    /// правый край содержащего блока. У точечного щупа в строке — нет: точка
    /// уже готова, а вычет своей ширины уводил бы коробку влево целиком
    /// (откат `htb-rtl-*` 08-19).
    ///
    /// Прежде порода узнавалась косвенно, по `hole.size.width > 0`. Признак
    /// ложен у распорки в содержащем блоке НУЛЕВОЙ ширины:
    /// `containing-block-020/022` (`div{width:0; padding:1in}`) уезжали ровно
    /// на свою ширину. Blink держит эти два шага раздельно: полосу
    /// выравнивания прибавляет величиной
    /// (`block_layout_algorithm.cc:1734-1745`, `available_inline_size` может
    /// быть нулём), а сторону — отдельным `InsetBias::kEnd`
    /// (`absolute_utils.cc:34`).
    pub block_strut: bool,
    /// Выравнивание в ПРЯМОУГОЛЬНИКЕ СТАТИЧЕСКОЙ ПОЗИЦИИ (css-position-3
    /// §static-position-rectangle; css-align-3 §justify-abspos,
    /// §align-abspos): физические доли `(x, y)`. По строчной оси прямоугольник
    /// — дырка блочной распорки (края содержимого родителя), коробка встаёт
    /// в долю `x` свободного места; по блочной он нулевой, и коробка висит от
    /// статической точки на долю `y` своей высоты. `None` — прежний путь.
    pub self_align: Option<(f32, f32)>,
}

pub type SpotCell = std::rc::Rc<std::cell::Cell<Spot>>;

thread_local! {
    /// Слой БЛИЖАЙШЕГО содержащего блока: абсолютная коробка, чей родитель
    /// содержащим блоком не является, переезжает сюда.
    ///
    /// Раскладка под нами считает края абсолютной коробки от НЕПОСРЕДСТВЕННОГО
    /// родителя — понятия «позиционированный предок» у неё нет. §10.1 требует
    /// ближайшего предка с `position` не `static`, поэтому коробка собирается
    /// на своём месте, а детём становится этому предку.
    pub(crate) static CB: std::cell::RefCell<Vec<Vec<(SpotCell, AnyElement)>>> =
        const { std::cell::RefCell::new(Vec::new()) };
    /// Параллельно `CB`: содержит ли коробку слоя и `position: fixed`.
    pub(crate) static CB_FIXED: std::cell::RefCell<Vec<bool>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Открыть слой содержащего блока вокруг детей позиционированной коробки.
pub fn cb_open() {
    cb_open_with(false);
}

/// Открыть слой содержащего блока; `fixed_cb` — коробка содержит и
/// `position: fixed` (трансформ, `contain: layout|paint`, css-transforms-1
/// §transform-rendering: «establishes a containing block for all
/// descendants»). Такой слой забирает фиксированных потомков в обход
/// промежуточных позиционированных предков (`cb_push_fixed`).
pub fn cb_open_with(fixed_cb: bool) {
    CB.with(|s| s.borrow_mut().push(Vec::new()));
    CB_FIXED.with(|s| s.borrow_mut().push(fixed_cb));
}

/// Отдать `position: fixed` слою ближайшего предка, содержащего `fixed`
/// (не ближайшего позиционированного, CSS 2.1 §10.1 п.3 + css-transforms-1).
/// Слоя нет — элемент возвращается, рисовать на месте.
pub fn cb_push_fixed(spot: SpotCell, el: AnyElement) -> Option<AnyElement> {
    let at = CB_FIXED.with(|f| f.borrow().iter().rposition(|x| *x));
    match at {
        Some(i) => CB.with(|s| match s.borrow_mut().get_mut(i) {
            Some(layer) => {
                layer.push((spot, el));
                None
            }
            None => Some(el),
        }),
        None => Some(el),
    }
}

/// Забрать накопленное верхним слоем содержащего блока и закрыть его.
pub fn cb_close() -> Vec<AnyElement> {
    CB_FIXED.with(|s| s.borrow_mut().pop());
    CB.with(|s| s.borrow_mut().pop())
        .unwrap_or_default()
        .into_iter()
        .map(|(spot, el)| icb_place(spot, el))
        .collect()
}

/// Отдать элемент слою ближайшего содержащего блока. Слоя нет — элемент
/// возвращается, рисовать на месте.
pub fn cb_push(spot: SpotCell, el: AnyElement) -> Option<AnyElement> {
    CB.with(|s| match s.borrow_mut().last_mut() {
        Some(layer) => {
            layer.push((spot, el));
            None
        }
        None => Some(el),
    })
}
