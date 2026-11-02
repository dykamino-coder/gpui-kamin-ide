//! Метаданные физического дерева в логической блочной оси рекордера.
//!
//! Копия меняет только роли размеров и сторон: реальные координаты берутся
//! из layout исходного дерева. Ортогональные блочные потоки потомков
//! требуют отдельных продолжений (ADR P-19 F2-3).

use crate::dom::{Element, Node};

/// Native recorder читает блочную ось независимо от inline direction.
pub(super) fn recorded_tree(c: &Element, rl: bool) -> Option<Element> {
    if c.style.used().reset_box_layout.display == Some(crate::style::computed::Display::None) {
        return Some(c.clone());
    }
    // TRANSITIONAL(F2-3): Transpose отказывает ортогональным потокам и оценочным RTL строкам; удалить отказ в P10 после child logical Space и собственного resume.
    if c.style.cascaded().vertical_decl() == Some(false)
        || c.style.cascaded().rl_decl().is_some_and(|v| v != rl)
    {
        return None;
    }
    let turn = |s: &crate::style::computed::Sides| crate::style::computed::Sides {
        top: if rl { s.right } else { s.left },
        bottom: if rl { s.left } else { s.right },
        left: if c.style.cascaded().wm().rtl() {
            s.bottom
        } else {
            s.top
        },
        right: if c.style.cascaded().wm().rtl() {
            s.top
        } else {
            s.bottom
        },
    };
    let mut t = c.clone();
    let source = c.style.used();
    let used = &mut *t.style.used_mut();
    {
        let first_value = used.reset_box_layout.width;
        let second_value = used.reset_box_layout.height;
        let first = &mut used.reset_box_layout.width;
        *first = second_value;
        let second = &mut used.reset_box_layout.height;
        *second = first_value;
    }
    {
        let first_value = used.reset_box_layout.min_width;
        let second_value = used.reset_box_layout.min_height;
        let first = &mut used.reset_box_layout.min_width;
        *first = second_value;
        let second = &mut used.reset_box_layout.min_height;
        *second = first_value;
    }
    {
        let first_value = used.reset_box_layout.max_width;
        let second_value = used.reset_box_layout.max_height;
        let first = &mut used.reset_box_layout.max_width;
        *first = second_value;
        let second = &mut used.reset_box_layout.max_height;
        *second = first_value;
    }
    used.reset_box_layout.padding = turn(&source.reset_box_layout.padding);
    used.reset_box_layout.margin = turn(&source.reset_box_layout.margin);
    used.reset_border.border_width = turn(&source.reset_border.border_width);
    used.reset_positioned.inset = turn(&source.reset_positioned.inset);
    // Видимость рамки — `[верх, право, низ, лево]` (`Computed::borders`).
    let v = source.reset_border.border_visible;
    used.reset_border.border_visible = [
        if rl { v[1] } else { v[3] },
        if c.style.cascaded().wm().rtl() {
            v[0]
        } else {
            v[2]
        },
        if rl { v[3] } else { v[1] },
        if c.style.cascaded().wm().rtl() {
            v[2]
        } else {
            v[0]
        },
    ];
    // Обрезка ПО ОСИ ПОТОКА: в вертикальном письме это `overflow-x`.
    used.reset_box_layout.overflow_y = source.reset_box_layout.overflow_x;
    used.reset_box_layout.overflow_x = source.reset_box_layout.overflow_y;
    // `border-spacing` физическое (`horizontal vertical`), ряды таблицы идут
    // по оси потока: между рядами в вертикали — ГОРИЗОНТАЛЬНАЯ составляющая.
    if let Some((x, y)) = source.inh.table_list.border_spacing {
        used.inh.table_list.border_spacing = Some((y, x));
    }
    t.children = c
        .children
        .iter()
        .map(|n| match n {
            Node::Element(k) => recorded_tree(k, rl).map(Node::Element),
            other => Some(other.clone()),
        })
        .collect::<Option<Vec<Node>>>()?
        .into();
    Some(t)
}
