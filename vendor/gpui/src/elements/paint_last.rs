// KaminIDE patch: краска ребёнка ПОСЛЕ потока без смены раскладки.
//
// Позиционированный элемент с `z-index: auto | 0` рисуется на шаге 8
// приложения E CSS 2.1 — после всего обычного содержимого БЛИЖАЙШЕГО
// контекста наложения и в порядке разметки, — а место в раскладке держит
// своё (относительный сдвиг, абсолют с одной свободной осью). Порядок краски
// в GPUI — порядок детей, и переставить ребёнка значило бы сдвинуть
// раскладку. Обёртка ничего не меняет в раскладке и prepaint (узел раскладки
// — узел ребёнка).
//
// Краска — в два режима:
// - собиратель открыт (`PaintCollect` корня документа, `Div` с прозрачностью
//   < 1): ребёнок со снимком контекста краски (`Window::paint_ctx`) уходит в
//   собиратель и рисуется в его конце по возрастанию ключа — номера элемента
//   в разметке. Через обычные `Div` перенос проходит насквозь; любой другой
//   элемент (преобразование, маска, подложка, фрагменты) собиратель
//   закрывает для своего поддерева (`hoist_boundary`), пока он сам не
//   откроет его снова (`paint_reopen`).
// - собирателя нет: `Div::paint` рисует такие обёртки вторым проходом, после
//   прочих детей.
//
// Отложенная краска (`deferred`) тут не годится: она выносит поддерево из
// масок и вложенной не бывает.
use crate::{
    AnyElement, App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId,
    IntoElement, LayoutId, Pixels, Style, Window,
    window::{PaintCtx, TextPaintCtx},
};
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;

struct Hoisted {
    key: u64,
    el: AnyElement,
    ctx: PaintCtx,
}

/// Строчное содержимое, отложенное до конца фонов потока (`PaintInline`).
struct HoistedInline {
    el: AnyElement,
    ctx: TextPaintCtx,
}

/// Собиратель: строчное содержимое (шаг 7, в порядке прихода — это порядок
/// дерева) и позиционированные (шаг 8, по ключу).
#[derive(Default)]
struct Frame {
    inline: VecDeque<HoistedInline>,
    positioned: Vec<Hoisted>,
}

thread_local! {
    /// Открыт ли собиратель для текущего места краски.
    static OPEN: Cell<bool> = const { Cell::new(false) };
    /// Значение `OPEN` до каждой вложенной границы (`hoist_boundary`).
    static SAVED: RefCell<Vec<bool>> = const { RefCell::new(Vec::new()) };
    /// Стопка собирателей: перенесённые дети с ключами и контекстом.
    static FRAMES: RefCell<Vec<Frame>> = const { RefCell::new(Vec::new()) };
}

/// Открыт ли собиратель.
pub(crate) fn hoist_open() -> bool {
    OPEN.with(|o| o.get())
}

/// Закрыть собиратель на время краски чужого элемента: его контекст краски
/// может быть особым (повторная краска фрагментов, подложка, преобразование).
pub(crate) fn hoist_boundary<R>(f: impl FnOnce() -> R) -> R {
    let prev = OPEN.with(|o| o.replace(false));
    SAVED.with(|s| s.borrow_mut().push(prev));
    let r = f();
    SAVED.with(|s| s.borrow_mut().pop());
    OPEN.with(|o| o.set(prev));
    r
}

/// Сквозной элемент (заместитель без своего контекста краски) возвращает
/// ребёнку собиратель, закрытый на нём границей.
pub fn paint_reopen<R>(f: impl FnOnce() -> R) -> R {
    let outer = SAVED.with(|s| s.borrow().last().copied()).unwrap_or(false);
    let prev = OPEN.with(|o| o.replace(outer));
    let r = f();
    OPEN.with(|o| o.set(prev));
    r
}

/// Сбросить собиратели. Зовётся при сборке документа (вне краски): пойманная
/// паника кадра оставила бы открытый собиратель, и следующий кадр терял бы
/// перенесённое.
pub fn paint_collect_reset() {
    OPEN.with(|o| o.set(false));
    SAVED.with(|s| s.borrow_mut().clear());
    FRAMES.with(|f| f.borrow_mut().clear());
}

fn open_frame() -> (usize, bool) {
    let depth = FRAMES.with(|f| {
        let mut f = f.borrow_mut();
        f.push(Frame::default());
        f.len() - 1
    });
    (depth, OPEN.with(|o| o.replace(true)))
}

fn close_frame(depth: usize, prev: bool, window: &mut Window, cx: &mut App) {
    loop {
        // Сначала строчное содержимое (шаг 7) в порядке прихода, потом
        // позиционированные (шаг 8): наименьший ключ; при равных — первый
        // пришедший. Строчное, пришедшее при краске позиционированного (его
        // собственные строки), рисуется сразу следом — до следующего ключа.
        enum Next {
            Inline(HoistedInline),
            Positioned(Hoisted),
        }
        let next = FRAMES.with(|f| {
            let mut f = f.borrow_mut();
            let frame = f.get_mut(depth)?;
            if let Some(item) = frame.inline.pop_front() {
                return Some(Next::Inline(item));
            }
            let list = &mut frame.positioned;
            let at = (0..list.len()).min_by_key(|&i| (list[i].key, i))?;
            Some(Next::Positioned(list.remove(at)))
        });
        match next {
            Some(Next::Inline(HoistedInline { mut el, ctx })) => {
                window.with_text_paint_ctx(ctx, |window| el.paint(window, cx));
            }
            Some(Next::Positioned(Hoisted { mut el, ctx, .. })) => {
                window.with_paint_ctx(ctx, |window| el.paint(window, cx));
            }
            None => break,
        }
    }
    FRAMES.with(|f| f.borrow_mut().truncate(depth));
    OPEN.with(|o| o.set(prev));
}

/// Paint `f` as an atomic box (inline-block, float, flex/grid item): the
/// inline content deferred inside it (step 7) is painted at its end, while
/// positioned descendants (step 8) still belong to the enclosing stacking
/// context and move to the outer collector (CSS 2.1 Appendix E: "positioned
/// descendants … should be considered part of the parent stacking context").
pub(crate) fn hoist_atomic(
    window: &mut Window,
    cx: &mut App,
    f: impl FnOnce(&mut Window, &mut App),
) {
    let (depth, prev) = open_frame();
    f(window, cx);
    loop {
        let next = FRAMES.with(|f| f.borrow_mut().get_mut(depth)?.inline.pop_front());
        let Some(HoistedInline { mut el, ctx }) = next else {
            break;
        };
        window.with_text_paint_ctx(ctx, |window| el.paint(window, cx));
    }
    let moved = FRAMES.with(|f| {
        let mut f = f.borrow_mut();
        let positioned = f
            .get_mut(depth)
            .map(|frame| std::mem::take(&mut frame.positioned))
            .unwrap_or_default();
        f.truncate(depth);
        match f.last_mut() {
            Some(outer) => {
                outer.positioned.extend(positioned);
                Vec::new()
            }
            None => positioned,
        }
    });
    OPEN.with(|o| o.set(prev));
    for Hoisted { mut el, ctx, .. } in moved {
        window.with_paint_ctx(ctx, |window| el.paint(window, cx));
    }
}

/// Нарисовать `f` собирателем: перенесённое внутри дорисовывается в конце.
pub(crate) fn hoist_collect(
    window: &mut Window,
    cx: &mut App,
    f: impl FnOnce(&mut Window, &mut App),
) {
    let (depth, prev) = open_frame();
    f(window, cx);
    close_frame(depth, prev, window, cx);
}

/// Ребёнок `Div`, который рисуется после потока.
pub struct PaintLast {
    child: Option<AnyElement>,
    key: u64,
}

impl PaintLast {
    /// Обернуть элемент: раскладка та же, краска — после потока.
    pub fn new(child: AnyElement) -> Self {
        PaintLast {
            child: Some(child),
            key: 0,
        }
    }

    /// Номер элемента в разметке: порядок краски внутри собирателя.
    pub fn key(mut self, key: u64) -> Self {
        self.key = key;
        self
    }
}

impl Element for PaintLast {
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
        let child = self.child.as_mut().expect("PaintLast: ребёнок на месте");
        (child.request_layout(window, cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _state: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        if let Some(child) = self.child.as_mut() {
            child.prepaint(window, cx);
        }
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
        if hoist_open() && FRAMES.with(|f| !f.borrow().is_empty()) {
            if let Some(el) = self.child.take() {
                let ctx = window.paint_ctx();
                let key = self.key;
                FRAMES.with(|f| {
                    if let Some(frame) = f.borrow_mut().last_mut() {
                        frame.positioned.push(Hoisted { key, el, ctx });
                    }
                });
            }
            return;
        }
        if let Some(child) = self.child.as_mut() {
            child.paint(window, cx);
        }
    }
}

impl IntoElement for PaintLast {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

/// Пара меток собирателя без своей коробки: открывающая ставится первым
/// ребёнком корня документа, закрывающая — последним; между ними `PaintLast`
/// уходит в собиратель, а на закрывающей дорисовывается по ключам.
pub struct PaintCollect {
    open: bool,
    state: Rc<Cell<Option<(usize, bool)>>>,
}

impl PaintCollect {
    /// Пара меток: открывающая и закрывающая.
    pub fn pair() -> (PaintCollect, PaintCollect) {
        let state: Rc<Cell<Option<(usize, bool)>>> = Rc::default();
        (
            PaintCollect {
                open: true,
                state: state.clone(),
            },
            PaintCollect { open: false, state },
        )
    }
}

impl Element for PaintCollect {
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
        // Внепоточная коробка нулевого размера: соседей не двигает.
        let style = Style {
            position: crate::Position::Absolute,
            ..Default::default()
        };
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _state: &mut (),
        _window: &mut Window,
        _cx: &mut App,
    ) {
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
        if self.open {
            self.state.set(Some(open_frame()));
        } else if let Some((depth, prev)) = self.state.take() {
            close_frame(depth, prev, window, cx);
        }
    }
}

impl IntoElement for PaintCollect {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

/// KaminIDE patch: строчное содержимое блока (строки с текстом, строчными
/// коробками и атомами), которое рисуется ПОСЛЕ фонов и рамок всех блоков
/// потока своего контекста наложения.
///
/// CSS 2.1 прил. E: шаг 4 — фоны блочных потомков в порядке дерева, шаг 5 —
/// флоаты, шаг 7 — строчное содержимое (в порядке дерева, строка за
/// строкой), шаг 8 — позиционированные. Порядок краски в GPUI — порядок
/// детей, поэтому строки раннего блока ложились ПОД фон позднего блока,
/// наехавшего на них отрицательным полем (`inline-block-zorder-001`).
/// При открытом собирателе (`PaintCollect` корня, `Div` с прозрачностью)
/// обёртка уходит в него и рисуется в его конце раньше позиционированных;
/// без собирателя — на месте, как прежде. Раскладка не меняется.
pub struct PaintInline {
    child: Option<AnyElement>,
}

impl PaintInline {
    /// Обернуть строчное содержимое.
    pub fn new(child: AnyElement) -> Self {
        PaintInline { child: Some(child) }
    }
}

impl Element for PaintInline {
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
        let child = self.child.as_mut().expect("PaintInline: ребёнок на месте");
        (child.request_layout(window, cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _state: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        if let Some(child) = self.child.as_mut() {
            child.prepaint(window, cx);
        }
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
        if hoist_open() && FRAMES.with(|f| !f.borrow().is_empty()) {
            if let Some(el) = self.child.take() {
                let ctx = window.text_paint_ctx();
                FRAMES.with(|f| {
                    if let Some(frame) = f.borrow_mut().last_mut() {
                        frame.inline.push_back(HoistedInline { el, ctx });
                    }
                });
            }
            return;
        }
        if let Some(child) = self.child.as_mut() {
            child.paint(window, cx);
        }
    }
}

impl IntoElement for PaintInline {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}
