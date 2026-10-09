//! Содержащий блок абсолютных коробок (поздняя расстановка).
// owner: A

use crate::layout::positioned::spot_geometry;
use gpui::{AnyElement, App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, IntoElement, LayoutId, ParentElement, Pixels, Styled, Window, px};

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
    /// Слои верхней отрисовки: по одному на каждый блок-контейнер в работе.
    /// Позиционированный элемент кладёт себя в верхний слой, а контейнер
    /// забирает слой целиком и дописывает его последними детьми.
    pub(crate) static LATE: std::cell::RefCell<Vec<Vec<(SpotCell, AnyElement)>>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

// Открыть слой на время сборки детей контейнера.
thread_local! {
    /// Слои НАЧАЛЬНОГО содержащего блока: внепоточные элементы, которым не
    /// нашлось позиционированного предка. По §10.1 их содержащий блок —
    /// область просмотра, а не ближайший родитель, поэтому они дописываются
    /// последними детьми документа.
    /// Пара `(SpotCell, AnyElement)`: по ПУСТОЙ оси элемент стоит на
    /// статической позиции, и её сообщает щуп с его места в потоке.
    pub(crate) static ICB: std::cell::RefCell<Vec<Vec<(SpotCell, AnyElement)>>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

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

/// Открыть слой ICB: документ, блок ленты или вложенный документ.
pub fn icb_open() {
    ICB.with(|s| s.borrow_mut().push(Vec::new()));
}

/// Забрать накопленное верхним слоем ICB и закрыть его.
pub fn icb_close() -> Vec<AnyElement> {
    ICB.with(|s| s.borrow_mut().pop())
        .unwrap_or_default()
        .into_iter()
        .map(|(spot, el)| icb_place(spot, el))
        .collect()
}

/// Заместитель слоя ICB — БЕЗ обёртки, в отличие от `spot_place`.
///
/// Любая коробка вокруг стала бы для раскладки содержащим блоком абсолютного
/// ребёнка (понятия «позиционированный предок» у раскладки нет), и края
/// считались бы от неё, а не от области просмотра — то есть ровно то, ради
/// чего затеян вынос. `LatePlace` своей коробки не заводит: он отдаёт
/// `layout_id` ребёнка.
fn icb_place(spot: SpotCell, child: AnyElement) -> AnyElement {
    LatePlace {
        child: Some(child),
        spot,
    }
    .into_any_element()
}

/// Открыт ли слой начального содержащего блока.
///
/// Поддерево ленты прокрутки строится в замыкании, а зовёт его
/// `ScrollArea::request_layout` — уже ПОСЛЕ `icb_close()`. Вынимать оттуда
/// кандидатов можно только пока слой ещё есть.
pub fn icb_active() -> bool {
    ICB.with(|s| !s.borrow().is_empty())
}

/// Отдать элемент слою ICB. Слоя нет — элемент возвращается, рисовать на
/// месте.
pub fn icb_push(spot: SpotCell, el: AnyElement) -> Option<AnyElement> {
    ICB.with(|s| match s.borrow_mut().last_mut() {
        Some(layer) => {
            layer.push((spot, el));
            None
        }
        None => Some(el),
    })
}

pub fn late_open() {
    LATE.with(|s| s.borrow_mut().push(Vec::new()));
}

/// Забрать накопленное верхним слоем и закрыть его.
pub fn late_close() -> Vec<AnyElement> {
    LATE.with(|s| s.borrow_mut().pop())
        .unwrap_or_default()
        .into_iter()
        .map(|(spot, el)| spot_place(spot, el))
        .collect()
}

/// Есть ли в открытом верхнем слое накопленное содержимое. Спрашивает цикл
/// детей контейнера перед позиционированным соседом: слой выпускается
/// раньше него, чтобы абсолют на статической позиции красился в порядке
/// дерева (CSS 2.1 прил. E, шаг 8), а не поверх всех позиционированных
/// соседей после него.
pub fn late_pending() -> bool {
    LATE.with(|s| s.borrow().last().is_some_and(|layer| !layer.is_empty()))
}

/// Отдать содержимое верхнему слою. Если слоя нет (элемент собирают вне
/// блока-контейнера), содержимое возвращается — рисовать его на месте.
pub fn late_push(spot: SpotCell, el: AnyElement) -> Option<AnyElement> {
    LATE.with(|s| match s.borrow_mut().last_mut() {
        Some(layer) => {
            layer.push((spot, el));
            None
        }
        None => Some(el),
    })
}

/// Нулевая распорка на месте элемента: занимает его место в потоке и
/// запоминает, где это место оказалось. `full` — распорка в потоке блоков (во
/// всю ширину), иначе — внутри строки.
pub fn spot_probe(spot: SpotCell, full: bool) -> AnyElement {
    let mut probe = gpui::div().h_0().flex_shrink_0();
    if full && spot.get().vertical {
        // Вертикальное письмо: блочный поток горизонтален (контейнер — ряд),
        // распорка встаёт колонкой нулевой ширины на месте элемента в потоке.
        probe = probe.w_0().h_full();
    } else if full {
        probe = probe.w_full();
    } else {
        // Внутри строки распорка прижимается к её ВЕРХУ: на базовой линии
        // содержимое уезжало бы под строку.
        probe = probe.w_0();
        probe.style().align_self = Some(gpui::AlignItems::FlexStart);
    }
    spot_geometry::probe(spot, full, probe.into_any_element())
}

/// Заместитель в конце списка: рисуется последним, но встаёт туда, где стояла
/// распорка. Сдвиг считается прямо в подготовке кадра — распорка идёт раньше
/// по списку детей, поэтому её место к этому моменту уже известно.
pub fn spot_place(spot: SpotCell, child: AnyElement) -> AnyElement {
    // Ряд, а не столбец: у абсолютного элемента ширина «по содержимому»
    // (CSS 2.1 §10.3.7), а ребёнок столбца растягивался бы во всю ширину
    // родителя — при письме справа налево содержимое такой коробки уезжало за
    // её край на всю эту ширину.
    let now = spot.get();
    // Спан с СОБСТВЕННЫМ вертикальным письмом в горизонтальном содержащем
    // блоке и с заданной осью: нулевая обёртка вставала после абзаца и сама
    // становилась точкой отсчёта (`abs-pos-non-replaced-vlr-121`: зелёный
    // на y = 0 при эталоне 160). Голый заместитель считает сдвиг от дырки
    // щупа сам; `!vertical` обязателен — вертикальные классы с обёрткой
    // зелёные (`vlr-087`, `vlr-119`). Разбор:
    // `target/scout-vabs-stretch-2026-09.md`, часть 5.
    // ★ ЗАМЕРЕНО: снятие `own_vertical` целиком (коммит 26b36d3) взяло 26
    // пар `abs-pos-non-replaced-v*`, но обвалило четыре
    // `absolute-replaced-height-012/019/026/033` с 0.02 в «красное видно»:
    // абсолютный `<iframe>` с процентной высотой — ЗАМЕЩАЕМЫЙ, у него
    // §10.6.2, а не §10.6.4. Признак содержащего блока (`ortho_limit`) в
    // роли различителя ЗАМЕРЕН И ОТКАЧЕН: он истинен в обоих случаях,
    // срез 480 пар дал +0/−28. Разводит именно замещаемость.
    if now.fixed_axes != (false, false)
        && !now.vertical
        && (now.own_vertical || !now.replaced)
    {
        return LatePlace {
            child: Some(child),
            spot,
        }
        .into_any_element();
    }
    // При вертикальном письме поток строк идёт поперёк: распорка тянется по
    // высоте, а не по ширине, иначе она уводила бы содержимое вниз. Ветка
    // нужна только позиции В СТРОКЕ (next_line): блочный заместитель без неё
    // стоит в горизонтальном потоке блоков и с вертикальной обёрткой уезжал
    // за край ячейки.
    let vertical_flow = now.vertical && now.next_line.is_some();
    // Блочный заместитель в вертикальном письме: контейнер — ряд, любая
    // распорка с размером двигала бы соседей. Обёртка нулевая, положение
    // целиком считает сдвиг заместителя (LatePlace).
    if now.vertical && now.next_line.is_none() {
        return gpui::div()
            .w_0()
            .h_0()
            .flex_shrink_0()
            .child(LatePlace {
                child: Some(child),
                spot,
            })
            .into_any_element();
    }
    let mut wrap = if vertical_flow {
        // Распорка НУЛЕВАЯ и по строчной оси тоже: при вертикальном письме
        // `h_full` отдавал контейнеру ДО-ПОВОРОТНУЮ высоту, равную
        // собственной ширине абсолюта, и коробка контейнера росла ровно на
        // неё (замерено на голой пробе: рамка 122.4 CSS против 42.4 у
        // эталона, излишек 80 = ширина абсолюта). Место абсолюта считает
        // сдвиг (`LatePlace`), распорке размер не нужен.
        gpui::div().h_0().w_0().flex_shrink_0().flex().flex_col()
    } else {
        gpui::div().w_full().h_0().flex_shrink_0().flex().flex_row()
    };
    wrap = wrap.items_start();
    // Начало строчной оси — правый край: при письме справа налево в
    // горизонтальном режиме и при vertical-rl (блочный поток идёт от правого
    // края, css-writing-modes §block-flow). У vertical-lr `direction`
    // строчную ось держит вертикальной, горизонталь остаётся левой.
    // ★ ЗАМЕРЕНО И ОТКАЧЕНО (04.09): якорь по `direction` и в вертикальном
    // письме (css-writing-modes-4 §7.1: `direction` ведёт строчную ось) плюс
    // rtl-ветки `LatePlace` с отражением по y для вертикального контейнера —
    // css-writing-modes 570 -> 570, L-a12 408 -> 408: ни одна пара не
    // сдвинулась. Класс «rtl в вертикальном письме» (12 пар, 2.67) держится
    // не на якоре, а на статической точке rtl-строки (корень B части 5
    // `target/scout-vabs-stretch-2026-09.md`).
    let anchor_end = if now.vertical {
        now.vertical_rl
    } else {
        now.rtl
    };
    wrap = if anchor_end {
        wrap.justify_end()
    } else {
        wrap.justify_start()
    };
    wrap.child(LatePlace {
        child: Some(child),
        spot,
    })
    .into_any_element()
}

pub struct LatePlace {
    pub(crate) child: Option<AnyElement>,
    pub(crate) spot: SpotCell,
}

impl Element for LatePlace {
    type RequestLayoutState = LayoutId;
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, LayoutId) {
        let layout_id = self.child.as_mut().unwrap().request_layout(window, cx);
        (layout_id, layout_id)
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        layout_id: &mut LayoutId,
        window: &mut Window,
        cx: &mut App,
    ) {
        let bounds = Bounds {
            origin: window.layout_origin_unrounded(*layout_id),
            size: window.layout_size_unrounded(*layout_id),
        };
        let now = self.spot.get();
        let shift = match (now.hole, now.next_line) {
            // Блочный: слева — край содержимого родителя (там же, где стоит сам
            // заместитель), сверху — низ строки, в которой он оказался.
            // Поперёк потока строк заместитель уже стоит у нужного края (его
            // ставит распорка ряда), поэтому правится только та ось, вдоль
            // которой идут строки.
            (Some(hole), Some(line)) if now.vertical && now.vertical_rl => {
                gpui::point(hole.origin.x - px(line) - bounds.origin.x, px(0.0))
            }
            (Some(hole), Some(line)) if now.vertical => {
                gpui::point(hole.origin.x + px(line) - bounds.origin.x, px(0.0))
            }
            (Some(hole), Some(line)) => {
                gpui::point(px(0.0), hole.origin.y + px(line) - bounds.origin.y)
            }
            // Начало строчной оси при письме справа налево — правый край:
            // поперёк потока заместителя уже выровняла обёртка (justify_end /
            // items_end), правится только ось потока строк.
            // Блочный заместитель в вертикальном письме: начало блочного
            // потока при vertical-rl — ПРАВЫЙ край, коробка уходит от него
            // влево (css-writing-modes §block-flow).
            (Some(hole), None) if now.vertical => {
                let x = if now.vertical_rl {
                    hole.origin.x - bounds.size.width
                } else {
                    hole.origin.x
                };
                gpui::point(x - bounds.origin.x, hole.origin.y - bounds.origin.y)
            }
            // Письмо справа налево: заместитель вешается ПРАВЫМ краем на
            // начало строчной оси (CSS 2.1 §10.3.7: статическая позиция в rtl
            // отсчитывается от правого края). У блочной распорки во всю
            // ширину правый край — правый край содержимого, у точечной в
            // строке — сама точка.
            // Правокрайняя формула верна для БЛОЧНОЙ распорки во всю
            // ширину; у ТОЧЕЧНОГО щупа в строке ширина нулевая, и вычитание
            // своей ширины уводило коробку влево на неё целиком
            // (htb-rtl-*: регресс 08-19).
            // ЗАМЕРЕНО, ЭФФЕКТА НЕТ (03.09): в вертикальном письме отсчитывать
            // `rtl` от НИЖНЕГО края дырки, а не от правого (css-writing-modes-3
            // §7.1: `direction` переворачивает строчную ось, а она вертикальна).
            // Правка рассуждением верна, но на двенадцати пробах `apw-*` не
            // сдвинула НИ ОДНОЙ сотой, и сборка HEAD без неё даёт те же числа.
            // Прежняя запись «apw-l 2.67 -> 0.00» была ЛОЖНОЙ: список пар
            // собирался конкатенацией, `"\a"` давал байт BEL, обе стороны не
            // грузились, и пустая страница сходилась с пустой. Списки строить
            // только через `Path`, проверять `od -c`.
            // Возвращать вместе с независимым решением осей (корень B): до
            // боевых пар этот рукав просто не доезжает.
            // Повёрнутый абзац: дырка уже экранная (`vt_map`). Строчная ось
            // здесь вертикальна, и при `direction: rtl` она идёт снизу вверх
            // — щуп отмечает НИЖНИЙ край коробки, а не верхний
            // (`abs-pos-non-replaced-vrl-126`: коробка стояла ровно на свою
            // высоту ниже нужного). Горизонтальные rtl-рукава ниже к такой
            // дырке не относятся — они сдвигают по x.
            (Some(hole), None) if now.rotated && now.rtl => gpui::point(
                hole.origin.x - bounds.origin.x,
                hole.origin.y - bounds.size.height - bounds.origin.y,
            ),
            (Some(hole), None) if now.rotated => hole.origin - bounds.origin,
            // Гейт — ПОРОДА щупа, а не размер дырки. Полоса выравнивания
            // может быть нулевой (содержащий блок нулевой ширины), но сторона
            // отсчёта от этого не меняется: §10.3.7 при rtl вешает на
            // статическую точку `right`, а не `left`. Blink разводит эти два
            // шага явно — прибавка `available_inline_size` (может быть нулём,
            // `block_layout_algorithm.cc:1740`) и `InsetBias::kEnd`
            // (`absolute_utils.cc:34`).
            // Прямоугольник статической позиции с `justify-self`/`align-self`
            // (`Spot::self_align`): правило выше строк rtl — сторону уже
            // посчитал `render::static_self_align` (при rtl `start` — доля 1,
            // та же формула, что у рукава ниже) (`align-self-static-position-
            // 001/006/008`, `justify-self-static-position-001`).
            (Some(hole), None) if now.block_strut && now.self_align.is_some() => {
                let (kx, ky) = now.self_align.unwrap_or_default();
                gpui::point(
                    hole.origin.x + (hole.size.width - bounds.size.width) * kx - bounds.origin.x,
                    hole.origin.y - bounds.size.height * ky - bounds.origin.y,
                )
            }
            (Some(hole), None) if now.rtl && now.block_strut => gpui::point(
                hole.origin.x + hole.size.width * now.line_align.unwrap_or(1.0)
                    - bounds.size.width
                    - bounds.origin.x,
                hole.origin.y - bounds.origin.y,
            ),
            // Статическая позиция по СВОБОДНОЙ строчной оси при `direction:
            // rtl` содержащего блока: §10.3.7 вешает на точку не `left`, а
            // `right`, то есть коробка стоит на ней ПРАВЫМ краем и растёт
            // назад (Blink: `static_position.h: ConvertToLogical` даёт
            // `kInlineEnd`, `absolute_utils.cc: GetStaticPositionInsetBias`
            // переводит его в `InsetBias::kEnd`).
            //
            // Гейт — набор осей, а не одно `now.rtl`. `(false, true)` ставят
            // РОВНО две ветки `atom_element` с `x_set != y_set`
            // (замещаемая и незамещаемая коробка с заданным только `top`
            // и/или `bottom`), и обе уходят `Piece::Overlay`: абзац остаётся
            // ТЕКСТОВЫМ, а дырку щупу даёт `lines.rs: point_of`. Замер по
            // снимкам всех двенадцати боевых пар
            // `abs-pos-non-replaced-v{lr,rl}-{128,129,160,161,176,177,192,
            // 193,208,209,224,225}`: зелёный квадрат стоит на x 168.0..248.0
            // при эталонных 88.0..168.0 — расхождение РОВНО в свою ширину 80
            // и ни в чём больше (вне двух квадратов разошедшихся точек 0).
            //
            // Чисто статическая коробка `(false, false)` сюда НЕ пускается
            // намеренно: её абзац уходит `Piece::Atom` в гибкий ряд
            // (`inline.rs: as_wrapped_row`), ряд при `rtl` не разворачивается,
            // и щуп садится в его конец — замер тех же снимков даёт x = 328.0
            // (правый край содержащего блока) при верных 168.0. Вычитание
            // своей ширины там сложило бы вторую ошибку с первой — это и есть
            // откат 08-19 из комментария выше. Блочный щуп во всю ширину
            // забирает рукав ВЫШЕ (`hole.size.width > 0`), поэтому семья
            // `css-position/static-position/htb-*` не задета.
            (Some(hole), None) if now.rtl && now.fixed_axes == (false, true) => gpui::point(
                hole.origin.x - bounds.size.width - bounds.origin.x,
                hole.origin.y - bounds.origin.y,
            ),
            (Some(hole), None) if now.rtl => gpui::point(
                hole.origin.x - bounds.origin.x,
                hole.origin.y - bounds.origin.y,
            ),
            // Ширину коробки прибавляет только БЛОЧНЫЙ щуп
            // (`abs-pos-border-offset-003`: ортогональный `.parent` прижат к
            // правому краю vrl-контейнера); строчный щуп 0×0 сдвига не
            // получает.
            // Прижим к ПРАВОМУ краю дырки верен только когда сторону задаёт
            // `direction: rtl` САМОГО КОНТЕЙНЕРА (`now.rtl` — направление
            // потока, а не собственное письмо коробки: css-writing-modes-4
            // §7.1, Blink `absolute_utils.cc` берёт сторону у
            // `container_writing_direction`). При `direction: ltr` статическая
            // позиция — левый край дырки без добавки (рукав ниже).
            (Some(hole), None)
                if now.own_vertical && !now.vertical && now.rtl && hole.size.width > px(0.0) =>
            {
                gpui::point(
                    hole.origin.x + bounds.size.width - bounds.origin.x,
                    hole.origin.y - bounds.origin.y,
                )
            }
            // Строка выровнена не по началу: статическая точка едет вдоль неё
            // на долю `line_align` (ltr вешает на неё ЛЕВЫЙ край коробки).
            (Some(hole), None) => gpui::point(
                hole.origin.x + hole.size.width * now.line_align.unwrap_or(0.0)
                    - bounds.origin.x,
                hole.origin.y - bounds.origin.y,
            ),
            (None, _) => gpui::point(px(0.0), px(0.0)),
        };
        // Ось, которую задал содержащий блок, раскладка уже разрешила — щуп
        // её не трогает; по свободной оси к дырке добавляется поле (§10.3.7:
        // от статической позиции коробку отодвигает `margin`).
        let shift = gpui::point(
            if now.fixed_axes.0 {
                px(0.0)
            } else {
                shift.x + px(now.free_margin.0)
            },
            if now.fixed_axes.1 {
                px(0.0)
            } else {
                shift.y + px(now.free_margin.1)
            },
        );
        let child = self.child.as_mut().unwrap();
        window.set_layout_placed_origin(*layout_id, bounds.origin + shift);
        child.prepaint_at(gpui::point(px(0.0), px(0.0)), window, cx);
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request: &mut LayoutId,
        _prepaint: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        // Заместитель своего контекста краски не заводит (сдвиг — в
        // подготовке): собиратель шага 8 (`gpui::PaintLast`) проходит его
        // насквозь, как обычную коробку.
        let child = self.child.as_mut().unwrap();
        gpui::paint_reopen(|| child.paint(window, cx));
    }
}

impl IntoElement for LatePlace {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

/// Коробка, повешенная на точку своим строчным НАЧАЛОМ.
///
/// Текстовый путь абзаца кладёт кусок вне потока ЛЕВЫМ верхним углом на точку
/// (`lines.rs: point_of` -> `prepaint_at`). При `direction: rtl` строчное
/// начало — ПРАВЫЙ край (CSS 2.1 §10.3.7: «otherwise, set 'right' to the
/// static position»; css-position-3 §staticpos-rect: прямоугольник
/// статической позиции «positioned at its inline-start static position»),
/// поэтому коробку надо сдвинуть назад ровно на свою ширину. Обёртка
/// возвращает раскладке `layout_id` ребёнка, то есть в потоке ничего не
/// добавляет и ничего не меряет заново.
///
/// Blink: `geometry/static_position.h:86` даёт `kInlineEnd` при `!IsLtr()`,
/// `absolute_utils.cc:27-37 GetStaticPositionInsetBias` переводит его в
/// `InsetBias::kEnd`.
///
/// Поля в сдвиг не входят — ровно как и у ltr-ветки, где на точку садится край
/// РАМОЧНОЙ коробки, а не отбивочной. Обе стороны считаются одинаково, и
/// разница проявилась бы только у абсолюта с ненулевым боковым полем.
pub struct InlineStartHang {
    pub(crate) child: Option<AnyElement>,
}

impl InlineStartHang {
    pub fn new(child: AnyElement) -> Self {
        Self { child: Some(child) }
    }
}

impl Element for InlineStartHang {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let layout_id = self.child.as_mut().unwrap().request_layout(window, cx);
        (layout_id, ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _state: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        let child = self.child.as_mut().unwrap();
        let shift = gpui::point(px(0.0) - bounds.size.width, px(0.0));
        window.with_exact_element_offset(shift, |window| child.prepaint(window, cx));
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request: &mut (),
        _prepaint: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.child.as_mut().unwrap().paint(window, cx);
    }
}

impl IntoElement for InlineStartHang {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}
