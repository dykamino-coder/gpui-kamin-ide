//! Вертикальный текст.
// owner: A

use crate::interact::*;

/// Строка вертикального письма: `writing-mode: vertical-rl` и `vertical-lr`.
///
/// Поворота мало: у повёрнутого текста меняются местами ширина и высота, и
/// раскладка обязана считать их поменянными — иначе строка занимает место как
/// горизонтальная и наезжает на соседей. Поэтому это свой элемент: он
/// измеряет текст сам, отдаёт раскладке перевёрнутый размер, а на отрисовке
/// разворачивает содержимое на четверть оборота по часовой стрелке.
/// `text-combine-upright`: составной знак в вертикальной строке.
///
/// Абзац вертикального письма рисуется ПОВОРОТОМ на четверть по часовой;
/// сжатый кусок обязан остаться стоячим — он контр-поворачивается вокруг
/// СВОЕГО ЦЕНТРА (квадрат кегля переходит в себя) и ужимается по строчной
/// оси в один кегль (css-writing-modes-3 §9.1).
pub(crate) mod combined_text;
pub struct CombinedUpright {
    pub(crate) child: Option<AnyElement>,
    /// Кегль — сторона квадрата, который кусок занимает в строке.
    pub(crate) em: f32,
    /// Сжимать ли содержимое в кегль: у `text-combine-upright` — да, у
    /// стоячего `inline-block` с горизонтальным письмом — нет, он просто
    /// переполняет свой квадрат.
    pub(crate) compress: bool,
    pub(crate) natural: gpui::Size<Pixels>,
}

impl CombinedUpright {
    pub fn new(child: AnyElement, em: f32) -> Self {
        CombinedUpright {
            child: Some(child),
            em,
            compress: true,
            natural: gpui::Size::default(),
        }
    }

    /// Стоячая коробка без сжатия (см. поле `compress`).
    pub fn upright_box(child: AnyElement, em: f32) -> Self {
        CombinedUpright {
            child: Some(child),
            em,
            compress: false,
            natural: gpui::Size::default(),
        }
    }
}

impl Element for CombinedUpright {
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
        let space = gpui::size(
            gpui::AvailableSpace::MaxContent,
            gpui::AvailableSpace::MaxContent,
        );
        self.natural = self
            .child
            .as_mut()
            .unwrap()
            .layout_as_root(space, window, cx);
        let mut style = gpui::Style::default();
        let side = gpui::Length::Definite(gpui::DefiniteLength::Absolute(
            gpui::AbsoluteLength::Pixels(px(self.em)),
        ));
        style.size.width = side.clone();
        style.size.height = side;
        (window.request_layout(style, [], cx), ())
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
        child.prepaint_at(bounds.origin, window, cx);
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _state: &mut (),
        _prepaint: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        let matrix = combined_geometry::transform(
            bounds,
            self.natural,
            self.em,
            self.compress,
            window.scale_factor(),
        );
        let child = self.child.as_mut().unwrap();
        window.with_transformation(matrix, |window| child.paint(window, cx));
    }
}

impl IntoElement for CombinedUpright {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

thread_local! {
    /// Двухкадровый замер повёрнутого блока: ключ абзаца → фактическая
    /// высота содержимого при РЕШЁННОЙ длине строки (см. `prepaint`).
    /// Первый кадр заявляет ширину по свободному замеру, второй — по факту;
    /// стенд и так ждёт устоявшийся кадр (как пробы ячеек).
    pub(crate) static VT_MEASURED: std::cell::RefCell<std::collections::HashMap<u64, Pixels>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
    /// Счётчик ВХОЖДЕНИЙ базового ключа за кадр: два вертикальных абзаца с
    /// одинаковым текстом и числом узлов (повторяющиеся ячейки) делили один
    /// ключ, и замер одного применялся к другому. Порядок обхода кадра
    /// детерминирован — порядковый номер вхождения стабилен между кадрами.
    pub(crate) static VT_SEQ: std::cell::RefCell<std::collections::HashMap<u64, u64>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
    /// Замер ЛЕВОГО края коробки корня (ключ — соль документа): фон холста
    /// позиционируется от коробки корня (CSS 2.2 §14.2), а она при
    /// `vertical-rl` по содержимому и прижата к правому краю окна — её край
    /// известен только после раскладки. Пишет подготовка тела, читает
    /// отрисовка холста того же кадра (подготовка всего дерева идёт раньше
    /// отрисовки); прошлое значение — запасное.
    pub(crate) static ROOT_LEFT: std::cell::RefCell<std::collections::HashMap<u64, f32>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
}

/// Левый край коробки корня (см. `ROOT_LEFT`).
pub fn root_left_prev(key: u64) -> Option<f32> {
    ROOT_LEFT.with(|c| c.borrow().get(&key).copied())
}

/// Обёртка, которая на подготовке записывает левый край своей коробки минус
/// `offset` (поля тела и рамка/отбивка корня) в `ROOT_LEFT` — раскладку не
/// меняет: узел раскладки — сам ребёнок.
pub struct RecordRootLeft {
    pub(crate) child: AnyElement,
    pub(crate) key: u64,
    pub(crate) offset: f32,
}

pub fn record_root_left(child: AnyElement, key: u64, offset: f32) -> AnyElement {
    RecordRootLeft { child, key, offset }.into_any_element()
}

impl Element for RecordRootLeft {
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
        (self.child.request_layout(window, cx), ())
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
        let left = f32::from(bounds.origin.x) - self.offset;
        ROOT_LEFT.with(|c| c.borrow_mut().insert(self.key, left));
        self.child.prepaint(window, cx);
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _state: &mut (),
        _prepaint: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.child.paint(window, cx);
    }
}

impl IntoElement for RecordRootLeft {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

/// Санитария начала кадра (`render`/`render_block`): счётчик вхождений
/// VT-ключей обнуляется, а НЕЗАКРЫТЫЕ слои позиционированных выбрасываются —
/// пойманная паника прошлого кадра оставляла слой навсегда, и `late_close`
/// следующей страницы отдавал чужие элементы.
/// Забыть двухкадровые замеры вертикальных абзацев — при смене документа:
/// ключи солятся документом, но мусор копился бы бесконечно.
pub fn forget_vt_measures() {
    VT_MEASURED.with(|c| c.borrow_mut().clear());
    ROOT_LEFT.with(|c| c.borrow_mut().clear());
    // Рамка повёрнутого абзаца живёт только на время его подготовки; если
    // подготовка оборвалась паникой, `set(prev)` не выполнится, и рамка
    // протекла бы в следующие страницы — щупы горизонтальных абсолютов
    // считались бы повёрнутыми (`abspos-*` из CSS2 уходили в красное).
    VT_FRAME.with(|c| c.set(None));
}

/// Ключ с порядковым номером вхождения базового ключа в этом кадре.
pub fn vt_seq_key(base: u64) -> u64 {
    VT_SEQ.with(|c| {
        let mut m = c.borrow_mut();
        let n = m.entry(base).or_insert(0);
        *n += 1;
        base ^ n.wrapping_mul(0x517C_C1B7_2722_0A95)
    })
}

thread_local! {
    /// Сборщик внутреннего строчного размера повёрнутого текста (`None` —
    /// закрыт): наибольшая длина строки всех `VerticalText`, разложенных,
    /// пока он открыт.
    pub static VT_INLINE_MAX: std::cell::Cell<Option<f32>> = const { std::cell::Cell::new(None) };
}

pub struct VerticalText {
    pub(crate) child: Option<AnyElement>,
    /// Естественный размер содержимого до поворота.
    pub(crate) natural: gpui::Size<Pixels>,
    /// Потолок заявляемой высоты (см. `claiming_height`).
    pub(crate) claim_cap: Option<Pixels>,
    /// Ключ двухкадрового замера (текст абзаца + соль документа).
    pub(crate) key: Option<u64>,
    /// Предел строки от родителя: если строка УЖЕ помещается, высота
    /// заявляется честно — иначе гибкая ячейка считает коробку нулевой и
    /// `justify-content` уводит рисунок из виду (table-cell-align-005).
    pub(crate) fit_limit: Option<Pixels>,
    pub(crate) inline_constraint: Option<crate::computed::orthogonal::InlineConstraint>,
    pub(crate) inline_keyword: Option<crate::computed::orthogonal::InlineKeyword>,
    /// `writing-mode: sideways-lr` — поворот ПРОТИВ часовой стрелки.
    /// css-writing-modes-4, таблица Abstract-Physical Mapping: у `sideways-lr`
    /// line-left = НИЗ, line-right = ВЕРХ, over = ЛЕВО (у всех остальных
    /// вертикальных письмён line-left = верх, over = право). Blink различает
    /// эти два случая ровно так же — `paint/line_relative_rect.cc:69-75`:
    /// `AffineTransform(0, 1, -1, 0, …)` против `AffineTransform(0, -1, 1, 0, …)`.
    pub(crate) ccw: bool,
    /// Ячейка вертикальной таблицы: мерить содержимое по МИНИМАЛЬНОМУ
    /// вдоль строки, а не по максимальному. Тогда заявленная высота
    /// повёрнутой коробки — вклад ячейки в меру её КОЛОНКИ (css-tables-3
    /// §computing-column-measures), и дорожку считает решётка, а не
    /// инлайн-размер всего стола. Ставится из `render.rs` (`col_min`).
    pub(crate) col_min: bool,
    /// `vertical-lr` при повороте по часовой: строки поданы снизу вверх, и
    /// первая строка — ЛЕВАЯ колонка (у `vertical-rl` — правая).
    pub(crate) lr: bool,
    /// Первая строка для базовой по оси x: шрифт, кегль, высота строки
    /// (`None` — `normal`) и центральная ли доминантная базовая.
    pub(crate) first_line: Option<(gpui::Font, Pixels, Option<Pixels>, bool)>,
}

impl Element for VerticalText {
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
        // Собственный корень раскладки: размер текста нужен ЗДЕСЬ, чтобы
        // отдать его родителю перевёрнутым.
        //
        // ПРОБОВАЛИ И ОТКАТИЛИ: замер по запросу родителя
        // (`request_measured_layout`), чтобы ограничение доходило до
        // содержимого и вертикальный текст переносился. Не работает: на этом
        // шаге раскладочный движок недоступен — повёрнутый блок сам живёт
        // внутри чужого замера, и вызов падает на `layout_engine.unwrap()`
        // (vendor/gpui/src/window.rs). Ограничение придётся доводить другим
        // путём — например, осью потока в самом стиле.
        //
        // Ячейка вертикальной таблицы меряется по МИНИМАЛЬНОМУ содержимому
        // вдоль строки: её вклад в дорожку колонки — это min-content
        // («the outer min-content width of each cell that spans the column»,
        // css-tables-3 §computing-column-measures), а не длина всей строки.
        // Обёртка до поворота стоит при этом БЕЗ жёсткой ширины
        // (`render.rs`, `col_min`), поэтому запрос доходит до самого абзаца:
        // `Paragraph` на `AvailableSpace::MinContent` отвечает
        // `probe.min_content(window)` (`lines.rs`). Дальше `natural.width` —
        // инлайн-мера колонки (её заявит высотой `fit_within`), а
        // `natural.height` — число строк при этой мере, то есть блочная
        // толщина ряда.
        let space = gpui::size(
            if self.col_min {
                gpui::AvailableSpace::MinContent
            } else {
                gpui::AvailableSpace::MaxContent
            },
            gpui::AvailableSpace::MaxContent,
        );
        self.natural = if let Some(constraint) = self.inline_constraint {
            orthogonal_measure::measure(self.child.as_mut().unwrap(), constraint, self.inline_keyword, window, cx)
        } else {
            self.child.as_mut().unwrap().layout_as_root_unrounded(space, window, cx)
        };
        // Внутренний строчный размер повёрнутого текста — длина его самой
        // длинной строки (`natural.width` горизонтального абзаца до
        // поворота). Наружу высотой он не заявляется (см. ниже), и пробе
        // флоата хоста полос (`band_flow::intrinsic`, письмо вертикальное)
        // его взять неоткуда — сборщик кладёт его сюда, когда открыт.
        VT_INLINE_MAX.with(|c| {
            if let Some(v) = c.get() {
                c.set(Some(v.max(f32::from(self.natural.width))));
            }
        });
        let mut style = gpui::Style::default();
        // Ordinary orthogonal blocks claim the independently measured inline size;
        // other contexts let their existing parent-sizing contract decide height.
        // Факт прошлого кадра сильнее свободного замера: перенос строк при
        // решённой длине меняет число колонок, а свободный замер его не
        // видит (text-combine-upright-line-breaking-rules-001).
        let claim = if self.inline_keyword.is_some() {
            self.natural.height
        } else { self
            .key
            .and_then(|k| VT_MEASURED.with(|c| c.borrow().get(&k).copied()))
            .unwrap_or(self.natural.height) };
        style.size.width = gpui::Length::Definite(gpui::DefiniteLength::Absolute(
            gpui::AbsoluteLength::Pixels(claim),
        ));
        vertical_line_baseline::apply(
            &mut style,
            self.child.as_mut().unwrap(),
            self.natural,
            self.ccw,
            self.lr,
            self.first_line.as_ref(),
            window,
            cx,
        );
        if self.inline_constraint.is_some() {
            style.size.height = self.natural.width.into();
        } else if let Some(cap) = self.claim_cap
            && self.natural.width >= cap
        {
            style.size.height = gpui::Length::Definite(gpui::DefiniteLength::Absolute(
                gpui::AbsoluteLength::Pixels(cap),
            ));
        } else if let Some(limit) = self.fit_limit
            // ★ ЗАМЕРЕНО И ОТКАЧЕНО (02.10): допуск +1 на привязку к пикселю
            // (`natural.width <= limit + 1`, заявка `min(natural, limit)`).
            // Обёртка до поворота стоит ЖЁСТКОЙ шириной в предел, и при пределе
            // 394 (рамка 3) замер возвращает 394.4 — строгое `<=` заявку
            // отклоняет, коробка схлопывается в рамку. С допуском:
            // `sizing-orthog-v{lr,rl}-in-htb-007/010/019/022` 0.68-0.83 → 0.00
            // (+8), но `-008/009/011/020/021/023/024` (обе стороны) 0.00-0.20 →
            // 0.55-0.75 (−14): у них эталон — абсолютная вертикальная коробка с
            // `height: auto`, и она схлопывается так же; короткой строке нужна
            // её длина, а жёсткая ширина даёт всегда предел. Возвращать только
            // вместе с замером длины строки и правкой абсолютного эталона.
            && self.natural.width <= limit
        {
            style.size.height = gpui::Length::Definite(gpui::DefiniteLength::Absolute(
                gpui::AbsoluteLength::Pixels(self.natural.width),
            ));
        }
        (window.request_layout(style, [], cx), ())
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
        // Ограничение доводится ЗДЕСЬ: на замере размеры коробки ещё
        // неизвестны, а на подготовке они уже решены раскладкой. Оси при этом
        // переставлены: то, что для родителя высота, для повёрнутого
        // содержимого — длина строки. Без этого шага вертикальный текст
        // мерился «по максимуму содержимого» и не переносился никогда.
        // ПРОБОВАЛИ И ОТКАТИЛИ: ограничивать длину строки высотой области
        // просмотра, когда родитель своей не задал (так велит CSS для
        // ортогональных потоков). Замерено: writing-modes 195 → 194,
        // `available-size-020/021` не чинятся, а `slr-alongside-vlr-floats`
        // ломается. Значит предел приходит откуда-то ещё.
        let along = bounds.size.height;
        let child = self.child.as_mut().unwrap();
        if along > gpui::px(0.) {
            let space = gpui::size(
                gpui::AvailableSpace::Definite(along),
                gpui::AvailableSpace::Definite(bounds.size.width),
            );
            let sized = child.layout_as_root_unrounded(space, window, cx);
            // Факт для следующего кадра: высота содержимого при решённой
            // длине строки — она и есть настоящая ширина повёрнутого блока.
            if let Some(k) = self.key
                && sized.height > gpui::px(0.)
            {
                VT_MEASURED.with(|c| {
                    let mut map = c.borrow_mut();
                    if map.len() >= 256 {
                        map.clear();
                    }
                    map.insert(k, sized.height);
                });
            }
        }
        // Щупу статической позиции нужна и СТОРОНА поворота: отображение
        // до-поворотной точки в экранную у `sideways-lr` зеркально (см. `vt_map`).
        let prev_ccw = VT_CCW.with(|c| c.replace(self.ccw));
        let prev = VT_FRAME.with(|c| c.replace(Some(bounds)));
        child.prepaint_at(bounds.origin, window, cx);
        VT_FRAME.with(|c| c.set(prev));
        VT_CCW.with(|c| c.set(prev_ccw));
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _state: &mut (),
        _prepaint: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        // ЗАМЕРЕНО И ОТКАЧЕНО (01.09): статическая позиция абсолюта внутри
        // ВЕРТИКАЛЬНОЙ строки. Корень виден числом: щуп пишет дырку в
        // ДО-поворотных координатах (матрица ниже применяется только на
        // отрисовке), а стиль строки собран горизонтальным
        // (`render.rs`: `horizontal.vertical = None`), поэтому вынесенный
        // абсолют считается горизонтальным — печать дала `vert=false`,
        // коробку 80×16 вместо 16×80 и сдвиг по поперечной оси.
        // Пробовал вдвоём: (1) отображать дырку через тот же поворот,
        // (2) возвращать вынесенному абсолюту письмо. Срез из 24 пар
        // `static-position/v{lr,rl}-*` — 0/24 и до, и после, а две пары
        // (`vrl-rtl-*-in-multicol`) ушли 4.32 → «красное видно».
        // Осталось незакрытым: коробка растягивается на всю строку (16×400
        // вместо 16×80), и `display: inline` под абсолютом доходит сюда
        // блочным (`line=Some(16)` во всех шести местах документа), поэтому
        // берётся блочный рукав и смещение вдоль строки теряется.
        let scale_factor = window.scale_factor();
        let dev = |v: Pixels| v.scale(scale_factor);
        // Поворот на четверть по часовой стрелке вокруг левого верхнего угла
        // уводит содержимое влево от коробки; сдвиг на её ширину возвращает
        // его на место.
        //
        // `sideways-lr` (css-writing-modes-4, Abstract-Physical Mapping):
        // строчная ось идёт СНИЗУ ВВЕРХ, ascender смотрит ВЛЕВО — значит
        // поворот ПРОТИВ часовой. Он уводит содержимое ВВЕРХ от коробки,
        // поэтому возвращает его сдвиг на ВЫСОТУ, а не на ширину.
        // После такого поворота первая горизонтальная строка сама оказывается
        // ЛЕВОЙ колонкой, а её начало — у нижнего края: подача строк снизу
        // вверх (`lines_reversed`) больше не нужна, см. `render.rs`.
        let (shift, angle) = if self.ccw {
            (
                gpui::point(
                    dev(bounds.origin.x),
                    dev(bounds.origin.y + bounds.size.height),
                ),
                -std::f32::consts::FRAC_PI_2,
            )
        } else {
            (
                gpui::point(
                    dev(bounds.origin.x + bounds.size.width),
                    dev(bounds.origin.y),
                ),
                std::f32::consts::FRAC_PI_2,
            )
        };
        let matrix = gpui::TransformationMatrix::unit()
            .translate(shift)
            .rotate(gpui::Radians(angle))
            .translate(gpui::point(dev(-bounds.origin.x), dev(-bounds.origin.y)));
        let child = self.child.as_mut().unwrap();
        // Свой слой с ПОСЛЕ-поворотными границами: порядок отрисовки сцена
        // считает по границам примитивов, а у повёрнутого текста они
        // ДО-трансформные — лежат вне своей коробки, перекрытие с фоном не
        // видно, и градиент соседа красился ПОВЕРХ глифа
        // (table-cell-align-005: с третьей ячейки текст пропадал под фоном).
        window.paint_layer(bounds, |window| {
            window.with_transformation(matrix, |window| child.paint(window, cx));
        });
    }
}

impl IntoElement for VerticalText {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

/// Где на экране оказалась распорка — и где оказался её заместитель.
///
/// Позиционированный элемент по CSS рисуется ПОВЕРХ обычного содержимого, а
/// порядок отрисовки у нас — порядок детей. Отложенная отрисовка для этого не
/// годится (вложенная в GPUI запрещена, а без вложенности рушится раскладка),
/// поэтому элемент уходит последним ребёнком и возвращается на место сдвигом:
/// щуп запоминает, где стояла распорка, заместитель — где встал сам, разница
/// и есть нужный сдвиг.
thread_local! {
    /// Рамка `VerticalText`, внутри которого сейчас идёт подготовка.
    ///
    /// Повёрнутый абзац подготавливается в ДО-ПОВОРОТНОЙ системе, а матрица
    /// поворота живёт только в `paint`. Щуп статической позиции пишет дырку
    /// именно в подготовке, поэтому без этой рамки `LatePlace` читает
    /// до-поворотную точку как экранную, и коробка уезжает на колонку.
    pub(crate) static VT_FRAME: std::cell::Cell<Option<Bounds<Pixels>>> =
        const { std::cell::Cell::new(None) };
    /// Сторона поворота этой рамки: `sideways-lr` вертится против часовой.
    pub(crate) static VT_CCW: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Экранная точка для до-поворотной, если мы внутри повёрнутого абзаца.
/// Mapping follows the actual clockwise/counter-clockwise text frame.
/// Идёт ли сейчас подготовка ПОВЁРНУТОГО абзаца (`VerticalText`).
pub fn in_rotated_frame() -> bool {
    VT_FRAME.with(|c| c.get()).is_some()
}

pub(crate) fn vt_map(hole: Bounds<Pixels>, thickness: Pixels) -> Bounds<Pixels> {
    let Some(vt) = VT_FRAME.with(|c| c.get()) else {
        return hole;
    };
    let pre_x = hole.origin.x - vt.origin.x;
    let pre_y = hole.origin.y - vt.origin.y;
    // `sideways-lr` вертится ПРОТИВ часовой (см. `VerticalText::paint`):
    // до-поворотная `(px, py)` от угла рамки становится экранной
    // `(x + py, y + h - px - thickness)` — зеркало обычного случая по обеим
    // осям (css-writing-modes-4: строчная ось снизу вверх, over слева).
    if VT_CCW.with(|c| c.get()) {
        return Bounds {
            origin: gpui::point(
                vt.origin.x + pre_y,
                vt.origin.y + vt.size.height - pre_x - thickness,
            ),
            size: hole.size,
        };
    }
    Bounds {
        origin: gpui::point(
            vt.origin.x + vt.size.width - pre_y - thickness,
            vt.origin.y + pre_x,
        ),
        size: hole.size,
    }
}
