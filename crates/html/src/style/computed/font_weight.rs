//! Вычисленный стиль: `font-weight` — разрешение
//! относительных `bolder`/`lighter` при наследовании по вычисленному числу
//! родителя, независимо от доступных начертаний (§2.2.1); потомки наследуют
//! уже разрешённое число, а не ещё один относительный шаг.

use super::Computed;

pub(crate) fn inherit(result: &mut Computed, parent: &Computed, own: &Computed) {
    // CSS Fonts 4 section 2.2.1: relative weights use the parent's computed
    // number, independently of which physical faces are available.
    let weight = parent.font_weight.unwrap_or(400);
    result.font_weight = match own.font_weight_step {
        1 => Some(match weight {
            0..350 => 400,
            350..550 => 700,
            550..900 => 900,
            _ => weight,
        }),
        -1 => Some(match weight {
            0..100 => weight,
            100..550 => 100,
            550..750 => 400,
            _ => 700,
        }),
        _ => own.font_weight.or(parent.font_weight),
    };
    // Descendants inherit the resolved number, not another relative step.
    result.font_weight_step = 0;
}
