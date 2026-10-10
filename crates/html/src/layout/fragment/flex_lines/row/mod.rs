//! Флекс-строки ряда: перенос по строкам и их формы.

use super::column::flex_gap_rules;
use super::{constrained_inside, row_item_width};
use crate::dom::{Element, Node};
use crate::layout::fragment::fragment_size::shape_full;
use crate::layout::fragment::{Shape, ShapeCx};
use crate::layout::page::paged::visible_overflow;
use crate::layout::positioned::predicates::carries_abspos;
use crate::render::{is_blank, out_of_flow};
use crate::style::computed::FlexDir;
use crate::style::computed::{Computed, Display};
use crate::style::values::value::Len;
mod shape;
pub(super) use shape::shape_flex_lines;

/// Строки многострочного РЯДА flex для `split_flex_lines`: по строке —
/// `(сдвиг по x, элемент, мера)` каждого элемента. Строки — по css-flexbox-1
/// §9.3 шаг 5 (главная ось — ширина), поперечный размер строки — наибольшая
/// внешняя высота её элементов; элемент `height: auto` при `align-items:
/// normal` тянется на строку (§9.4 шаг 11) — полом `min-height`, чтобы рост
/// от фрагментации (`grow_pushed`) коробку не обрезал (Blink: «expansion past
/// the block-end of each row», `flex_layout_algorithm.cc:2560-2575`).
/// `break-before` любого элемента строки — разрыв перед строкой, `break-after`
/// — после неё (`:1898-1906`): на первого и последнего элемента строки. Гейт —
/// как у колонки, плюс высота контейнера `auto` и хотя бы одна строка из
/// нескольких элементов: ряд «элемент на строку» прежний путь уже знает.
#[allow(clippy::type_complexity)]
pub(super) fn flex_row_lines_of(
    c: &Element,
    col_w: Option<f32>,
) -> Option<Vec<Vec<(f32, Element, Shape)>>> {
    if flex_gap_rules(&c.style) {
        return None;
    }
    let zero = |l: &Option<Len>| match l {
        None => true,
        Some(Len::Px(v)) => v.abs() < 0.01,
        _ => false,
    };
    let s = &c.style;
    let b = s.borders();
    if let Some(value) = inline_flex_lines(c, zero, s, b) {
        return value;
    }
    let main = match s.width {
        Some(Len::Px(w)) => w,
        None | Some(Len::Auto) => col_w?,
        _ => return None,
    };
    let (row_gap, col_gap) = match s.gap {
        None => (0.0, 0.0),
        Some((r, g)) => {
            let px = |l: &Option<Len>| match l {
                None => Some(0.0),
                Some(Len::Px(v)) => Some(v.max(0.0)),
                _ => None,
            };
            (px(&r)?, px(&g)?)
        }
    };
    let side = |l: &Option<Len>| match l {
        None => Some(0.0),
        Some(Len::Px(v)) => Some(*v),
        _ => None,
    };
    let mut items: Vec<&Element> = Vec::new();
    for n in c.children.iter().filter(|n| !is_blank(n)) {
        let Node::Element(k) = n else { return None };
        let ks = &k.style;
        if k.inline
            || out_of_flow(ks)
            || !matches!(
                ks.position,
                None | Some(crate::style::computed::Position::Relative)
            )
            || ks.float.unwrap_or(0) != 0
            || ks.flex_grow.is_some_and(|g| g > 0.0)
            || ks.align_self.is_some()
            || ks.align_self_normal
            || ks.min_height.is_some()
            || !zero(&ks.margin.left)
            || !zero(&ks.margin.right)
            || row_item_width(ks, main).is_none()
        {
            return None;
        }
        items.push(k);
    }
    if items.is_empty() {
        return None;
    }
    items.sort_by_key(|k| k.style.order.unwrap_or(0));
    // Строки по внешней ширине элементов (главная ось).
    let mut lines: Vec<Vec<(f32, Element, Shape)>> = Vec::new();
    let mut used = 0.0f32;
    // Строка из одного элемента и размер в процентах/`flex-basis` — шире
    // прежнего гейта: такой ряд прежде шёл целым контейнером, и его мера
    // (`shape_full` контейнера) уже знала рост строки от разрыва внутри
    // элемента, растяжение соседей на выросшую строку, статическое место
    // абсолютного потомка и вложенный параллельный поток. Раскрытые элементы
    // этого не выражают (`grow_pushed` растит лишь сам элемент): замерено
    // −3 (`multi-line-row-flex-fragmentation-053/060/062`). Такие элементы —
    // прежним путём.
    let widened = items
        .iter()
        .any(|k| !matches!(k.style.width, Some(Len::Px(_))) || k.style.flex_basis.is_some());
    let mut single = true;
    let mut risky = false;
    for k in &items {
        risky |= carries_abspos(k, 4) || constrained_inside(k, 4);
    }
    for k in items {
        let kb = k.style.borders();
        // Гипотетический главный размер (css-flexbox-1 §9.2 шаг 3):
        // `flex-basis` в точках/процентах, иначе `width`; проценты — от
        // главного размера контейнера (§9.2 «percentage … against the flex
        // container's inner main size»). Копия элемента несёт его в точках:
        // в стопке он рисуется блоком в колонке.
        let w = row_item_width(&k.style, main)?;
        let mut k = k.clone();
        k.style.width = Some(Len::Px(w));
        k.style.flex_basis = None;
        let k = &k;
        let w = if k.style.border_box == Some(true) {
            w
        } else {
            w + side(&k.style.padding.left)?
                + side(&k.style.padding.right)?
                + side(&kb.left)?
                + side(&kb.right)?
        };
        let sh = shape_full(k, 4, ShapeCx::COLUMNS)?;
        match lines.last_mut() {
            Some(line) if used + col_gap + w <= main + 0.01 => {
                single = false;
                line.push((used + col_gap, k.clone(), sh));
                used += col_gap + w;
            }
            _ => {
                lines.push(vec![(0.0, k.clone(), sh)]);
                used = w;
            }
        }
    }
    if (single || widened) && (risky || lines.iter().flatten().any(|x| !x.2.4.is_empty())) {
        return None;
    }
    shape_flex_lines(row_gap, side, &mut lines)?;
    Some(lines)
}

pub(super) fn inline_flex_lines(
    c: &Element,
    zero: impl Fn(&Option<Len>) -> bool,
    s: &Computed,
    b: crate::style::computed::Sides,
) -> Option<
    Option<
        Vec<
            Vec<(
                f32,
                Element,
                (f32, f32, f32, Vec<(f32, f32)>, Vec<f32>, Vec<(f32, f32)>),
            )>,
        >,
    >,
> {
    if c.inline
        || s.display != Some(Display::Flex)
        || s.webkit_box == Some(true)
        || !matches!(s.flex_dir, None | Some(FlexDir::Row))
        || s.flex_wrap != Some(true)
        || s.flex_wrap_reverse == Some(true)
        || s.flex_balance == Some(true)
        || s.vertical == Some(true)
        || s.rtl == Some(true)
        || s.justify_content.is_some()
        || s.align_content.is_some()
        || s.align_items.is_some()
        || !matches!(s.height, None | Some(Len::Auto))
        || s.min_height.is_some()
        || s.max_height.is_some()
        || s.transform.is_some()
        || s.background.is_some()
        || s.bg_image.is_some()
        || !(s.position.is_none()
            || (s.position == Some(crate::style::computed::Position::Relative)
                && [&s.inset.top, &s.inset.right, &s.inset.bottom, &s.inset.left]
                    .into_iter()
                    .all(|l| matches!(l, None | Some(Len::Auto)))))
        || !visible_overflow(s)
        || ![
            &s.margin.top,
            &s.margin.bottom,
            &s.margin.left,
            &s.margin.right,
            &s.padding.top,
            &s.padding.bottom,
            &s.padding.left,
            &s.padding.right,
            &b.top,
            &b.bottom,
            &b.left,
            &b.right,
        ]
        .into_iter()
        .all(zero)
    {
        return Some(None);
    }
    None
}
