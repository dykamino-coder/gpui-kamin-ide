//! Содержащий блок абсолютных коробок (поздняя расстановка).
// owner: A

use gpui::{AnyElement, Pixels};
mod spot;
pub use spot::Spot;
mod late;
pub use late::LatePlace;
pub use late::spot_place;
pub use late::spot_probe;
mod hang;
pub use hang::InlineStartHang;
mod source_paint;
pub(crate) use source_paint::split as split_source_paint;
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

thread_local! {
    static LINE_BOX: std::cell::Cell<Option<(Pixels, Pixels, Pixels)>> = const { std::cell::Cell::new(None) };
}

/// Prepaint a static-position probe placed in a text line with that line's
/// box `(top, height, start x)`; the probe takes it (`take_line_box`).
pub fn with_line_box<R>(line: (Pixels, Pixels, Pixels), f: impl FnOnce() -> R) -> R {
    let prev = LINE_BOX.with(|c| c.replace(Some(line)));
    let out = f();
    LINE_BOX.with(|c| c.set(prev));
    out
}

pub(crate) fn take_line_box() -> Option<(Pixels, Pixels, Pixels)> {
    LINE_BOX.with(|c| c.take())
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
    /// Параллельно `CB`: какие коробки забирает слой — `CB_ABS` (только
    /// абсолютные), `CB_ABS | CB_FIX` (и `position: fixed`) или `CB_FIX`
    /// (только фиксированные, см. `cb_open_fixed_only`).
    pub(crate) static CB_FIXED: std::cell::RefCell<Vec<u8>> = const { std::cell::RefCell::new(Vec::new()) };
    /// Parallel to `CB`: `Some(index)` for a strict layer (`cb_open_strict`),
    /// which takes only boxes whose NEAREST containing block is its owner —
    /// the CB-ancestor index of the owner (`Computed::cb_depth` + 1).
    pub(crate) static CB_STRICT: std::cell::RefCell<Vec<Option<u16>>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// The CB-ancestor index of the nearest containing block of an absolute box
/// whose parent's computed style is `parent` (see `Computed::cb_depth`).
pub(crate) fn nearest_cb_index(parent: &crate::style::computed::Computed) -> u16 {
    parent.cb_depth + u16::from(crate::text::inline::establishes_cb(parent))
}

/// Open a strict containing-block layer around the children of a box whose
/// own style is `own`: an absolute descendant whose nearest containing block
/// is a box without a layer below it (a table part, a positioned inline)
/// passes this layer by and keeps its previous route.
pub fn cb_open_strict(own: &crate::style::computed::Computed, fixed_cb: bool) {
    cb_open_with(fixed_cb);
    CB_STRICT.with(|s| {
        if let Some(last) = s.borrow_mut().last_mut() {
            *last = Some(own.cb_depth + 1);
        }
    });
}

/// Открыть слой содержащего блока; `fixed_cb` — коробка содержит и
/// `position: fixed` (трансформ, `contain: layout|paint`, css-transforms-1
/// §transform-rendering: «establishes a containing block for all
/// descendants»). Такой слой забирает фиксированных потомков в обход
/// промежуточных позиционированных предков (`cb_push_fixed`).
pub fn cb_open_with(fixed_cb: bool) {
    CB.with(|s| s.borrow_mut().push(Vec::new()));
    CB_STRICT.with(|s| s.borrow_mut().push(None));
    CB_FIXED.with(|s| {
        s.borrow_mut()
            .push(CB_ABS | if fixed_cb { CB_FIX } else { 0 })
    });
}

/// Is a containing-block layer available for this positioned box?
pub(crate) fn cb_active(fixed: bool) -> bool {
    let mask = if fixed { CB_FIX } else { CB_ABS };
    CB_FIXED.with(|flags| flags.borrow().iter().any(|f| f & mask != 0))
}

const CB_ABS: u8 = 1;
const CB_FIX: u8 = 2;

/// Открыть слой, который забирает ТОЛЬКО `position: fixed`: коробка
/// многоколоночника с трансформом (или `contain: layout|paint`) — содержащий
/// блок фиксированных потомков (css-transforms-1 §transform-rendering), но
/// абсолютные потомки копий колонок остаются в своих копиях и режутся по
/// колонкам (css-position-3 §abspos-breaking).
pub fn cb_open_fixed_only() {
    CB.with(|s| s.borrow_mut().push(Vec::new()));
    CB_STRICT.with(|s| s.borrow_mut().push(None));
    CB_FIXED.with(|s| s.borrow_mut().push(CB_FIX));
}

/// Отдать `position: fixed` слою ближайшего предка, содержащего `fixed`
/// (не ближайшего позиционированного, CSS 2.1 §10.1 п.3 + css-transforms-1).
/// Слоя нет — элемент возвращается, рисовать на месте.
pub fn cb_push_fixed(spot: SpotCell, el: AnyElement) -> Option<AnyElement> {
    let at = CB_FIXED.with(|f| f.borrow().iter().rposition(|x| x & CB_FIX != 0));
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
    CB_STRICT.with(|s| s.borrow_mut().pop());
    CB.with(|s| s.borrow_mut().pop())
        .unwrap_or_default()
        .into_iter()
        .map(|(spot, el)| icb_place(spot, el))
        .collect()
}

/// Отдать элемент слою ближайшего содержащего блока. Слоя нет — элемент
/// возвращается, рисовать на месте.
pub fn cb_push(spot: SpotCell, el: AnyElement, nearest: u16) -> Option<AnyElement> {
    let at = CB_FIXED.with(|f| {
        CB_STRICT.with(|s| {
            let s = s.borrow();
            f.borrow().iter().enumerate().rposition(|(i, x)| {
                x & CB_ABS != 0 && s.get(i).copied().flatten().is_none_or(|d| d == nearest)
            })
        })
    });
    let Some(i) = at else {
        return Some(el);
    };
    CB.with(|s| match s.borrow_mut().get_mut(i) {
        Some(layer) => {
            layer.push((spot, el));
            None
        }
        None => Some(el),
    })
}
