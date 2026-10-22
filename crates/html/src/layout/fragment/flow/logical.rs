//! Метаданные физического дерева в логической блочной оси рекордера.
//!
//! Копия меняет только роли размеров и сторон: реальные координаты берутся
//! из layout исходного дерева. Ортогональные блочные потоки потомков
//! требуют отдельных продолжений (ADR P-19 F2-3).

use crate::dom::{Element, Node};

/// CSS Writing Modes 3 §6.4: block-start становится верхом, inline-start —
/// левым краем при direction:ltr. Направление RTL здесь ещё не поддержано.
/// Физическая копия для отрисовки не транспонируется.
pub(crate) fn transpose_tree(c: &Element, rl: bool) -> Option<Element> {
    transpose(c, rl, false)
}

/// Native recorder читает блочную ось независимо от inline direction.
/// Старой оценке строк RTL по-прежнему недоступно.
pub(super) fn recorded_tree(c: &Element, rl: bool) -> Option<Element> {
    transpose(c, rl, true)
}

fn transpose(c: &Element, rl: bool, recorded: bool) -> Option<Element> {
    if recorded && c.style.computed().display == Some(crate::style::computed::Display::None) {
        return Some(c.clone());
    }
    if c.style.cascaded().vertical_decl() == Some(false)
        || c.style.cascaded().rl_decl().is_some_and(|v| v != rl)
        || (!recorded && c.style.cascaded().wm().rtl())
    {
        return None;
    }
    let turn = |s: &crate::style::computed::Sides| crate::style::computed::Sides {
        top: if rl { s.right } else { s.left },
        bottom: if rl { s.left } else { s.right },
        left: if c.style.cascaded().wm().rtl() { s.bottom } else { s.top },
        right: if c.style.cascaded().wm().rtl() { s.top } else { s.bottom },
    };
    let mut t = c.clone();
    let source = c.style.computed();
    let used = &mut *t.style.used_mut();
    std::mem::swap(&mut used.width, &mut used.height);
    std::mem::swap(&mut used.min_width, &mut used.min_height);
    std::mem::swap(&mut used.max_width, &mut used.max_height);
    used.padding = turn(&source.padding);
    used.margin = turn(&source.margin);
    used.border_width = turn(&source.border_width);
    used.inset = turn(&source.inset);
    // Видимость рамки — `[верх, право, низ, лево]` (`Computed::borders`).
    let v = source.border_visible;
    used.border_visible = [
        if rl { v[1] } else { v[3] },
        if c.style.cascaded().wm().rtl() { v[0] } else { v[2] },
        if rl { v[3] } else { v[1] },
        if c.style.cascaded().wm().rtl() { v[2] } else { v[0] },
    ];
    // Обрезка ПО ОСИ ПОТОКА: в вертикальном письме это `overflow-x`.
    used.overflow_y = source.overflow_x;
    used.overflow_x = source.overflow_y;
    // `border-spacing` физическое (`horizontal vertical`), ряды таблицы идут
    // по оси потока: между рядами в вертикали — ГОРИЗОНТАЛЬНАЯ составляющая.
    if let Some((x, y)) = source.border_spacing {
        used.border_spacing = Some((y, x));
    }
    t.children = c
        .children
        .iter()
        .map(|n| match n {
            Node::Element(k) => transpose(k, rl, recorded).map(Node::Element),
            other => Some(other.clone()),
        })
        .collect::<Option<Vec<Node>>>()?;
    Some(t)
}
