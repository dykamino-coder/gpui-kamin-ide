//! Вертикальный текст.
mod combined_upright;
mod root_left;
mod vertical_text;
pub use crate::text::vertical::combined_upright::CombinedUpright;
pub use crate::text::vertical::root_left::RecordRootLeft;
pub use crate::text::vertical::root_left::record_root_left;
pub use crate::text::vertical::root_left::root_left_prev;
pub use crate::text::vertical::vertical_text::VerticalText;

// owner: A

use gpui::{Bounds, Pixels};

mod combined_geometry;
pub(crate) mod combined_text;
mod vertical_line_baseline;
mod vertical_style;

thread_local! {
    /// Двухкадровый замер повёрнутого блока: ключ абзаца → фактическая
    /// высота содержимого при РЕШЁННОЙ длине строки (см. `prepaint`).
    /// Первый кадр заявляет ширину по свободному замеру, второй — по факту;
    /// стенд и так ждёт устоявшийся кадр (как пробы ячеек).
    static VT_MEASURED: std::cell::RefCell<std::collections::HashMap<u64, Pixels>> =
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
    static ROOT_LEFT: std::cell::RefCell<std::collections::HashMap<u64, f32>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
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

// Где на экране оказалась распорка — и где оказался её заместитель.
//
// Позиционированный элемент по CSS рисуется ПОВЕРХ обычного содержимого, а
// порядок отрисовки у нас — порядок детей. Отложенная отрисовка для этого не
// годится (вложенная в GPUI запрещена, а без вложенности рушится раскладка),
// поэтому элемент уходит последним ребёнком и возвращается на место сдвигом:
// щуп запоминает, где стояла распорка, заместитель — где встал сам, разница
// и есть нужный сдвиг.
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
