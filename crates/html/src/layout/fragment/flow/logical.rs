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
    if recorded && c.style.display == Some(crate::style::computed::Display::None) {
        return Some(c.clone());
    }
    if c.style.vertical_decl() == Some(false)
        || c.style.rl_decl().is_some_and(|v| v != rl)
        || (!recorded && c.style.wm().rtl())
    {
        return None;
    }
    let turn = |s: &crate::style::computed::Sides| crate::style::computed::Sides {
        top: if rl { s.right } else { s.left },
        bottom: if rl { s.left } else { s.right },
        left: if c.style.wm().rtl() { s.bottom } else { s.top },
        right: if c.style.wm().rtl() { s.top } else { s.bottom },
    };
    let mut t = c.clone();
    std::mem::swap(&mut t.style.width, &mut t.style.height);
    std::mem::swap(&mut t.style.min_width, &mut t.style.min_height);
    std::mem::swap(&mut t.style.max_width, &mut t.style.max_height);
    t.style.padding = turn(&c.style.padding);
    t.style.margin = turn(&c.style.margin);
    t.style.border_width = turn(&c.style.border_width);
    t.style.inset = turn(&c.style.inset);
    // Видимость рамки — `[верх, право, низ, лево]` (`Computed::borders`).
    let v = c.style.border_visible;
    t.style.border_visible = [
        if rl { v[1] } else { v[3] },
        if c.style.wm().rtl() { v[0] } else { v[2] },
        if rl { v[3] } else { v[1] },
        if c.style.wm().rtl() { v[2] } else { v[0] },
    ];
    // Обрезка ПО ОСИ ПОТОКА: в вертикальном письме это `overflow-x`.
    t.style.overflow_y = c.style.overflow_x;
    t.style.overflow_x = c.style.overflow_y;
    // `border-spacing` физическое (`horizontal vertical`), ряды таблицы идут
    // по оси потока: между рядами в вертикали — ГОРИЗОНТАЛЬНАЯ составляющая.
    if let Some((x, y)) = c.style.border_spacing {
        t.style.border_spacing = Some((y, x));
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
