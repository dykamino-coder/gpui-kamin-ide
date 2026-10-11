//! Корень постраничного режима: поля и имя первой страницы, группы строчных прогонов.

use crate::dom::Node;
use crate::render::is_blank;
use crate::style::cascade::inherit::inherit;
use crate::style::computed::{Computed, Display};
use crate::style::values::value::Len;

#[derive(PartialEq)]
pub(super) enum Run {
    None,
    Inline,
    Float,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn resolve_root_page(
    none: &mut bool,
    canvas: &mut Option<gpui::Hsla>,
    root: &mut Computed,
    nodes: &mut Vec<Node>,
    left: &mut f32,
    right: &mut f32,
    top: &mut f32,
    root_page: &mut String,
) {
    loop {
        let live: Vec<&Node> = nodes.iter().filter(|n| !is_blank(n)).collect();
        let [Node::Element(e)] = live.as_slice() else {
            break;
        };
        if !matches!(e.tag.as_str(), "html" | "body") {
            break;
        }
        // Корень без коробки: один пустой лист, и свойства `@page` к нему не
        // применяются (Blink `StyleForPage`: «The root is display:none. One
        // page box will still be created, but no properties should apply»;
        // `root-element-display-none-print` против `blank-print-ref`).
        if matches!(e.style.display, Some(Display::None)) {
            *none = true;
            *nodes = Vec::new();
            break;
        }
        let e = (*e).clone();
        let side = |l: &Option<Len>| match l {
            Some(Len::Px(v)) => *v,
            _ => 0.0,
        };
        let b = e.style.borders();
        // Обёртку с видимой рамкой или своим `display` (сетка, флекс) не
        // снимаем: снятая теряла рамку и раскладку (`page-box-011-print-ref`:
        // `body { border: 10px solid }` — чёрной рамки не было;
        // `page-box-000-print-ref`: `html { display: grid; border: 20px }`).
        // Она остаётся одним ребёнком стопки; фон всё равно уходит в канвас
        // (§painting: фон корня/тела красит канвас листа).
        let framed = [&b.top, &b.right, &b.bottom, &b.left]
            .iter()
            .any(|l| side(l) > 0.0);
        let boxy = matches!(
            e.style.display,
            Some(Display::Grid)
                | Some(Display::InlineGrid)
                | Some(Display::Flex)
                | Some(Display::InlineFlex)
                | Some(Display::GridLanes)
        );
        if framed || boxy {
            if let Some(c) = e.style.background.filter(|c| c.a > 0.0) {
                *canvas = Some(c.to_hsla());
            }
            break;
        }
        *left += side(&e.style.margin.left) + side(&b.left) + side(&e.style.padding.left);
        *right += side(&e.style.margin.right) + side(&b.right) + side(&e.style.padding.right);
        *top += side(&e.style.margin.top) + side(&b.top) + side(&e.style.padding.top);
        if let Some(c) = e.style.background.filter(|c| c.a > 0.0) {
            *canvas = Some(c.to_hsla());
        }
        if let Some(p) = &e.style.page {
            *root_page = p.clone();
        }
        *root = inherit(&*root, &e.style);
        *nodes = e.children;
    }
}

pub(super) fn group_runs(
    nodes: Vec<Node>,
    inline_level: impl Fn(&Node) -> bool,
    floated: impl Fn(&Node) -> bool,
    groups: &mut Vec<Vec<Node>>,
    mut run: Run,
) {
    for n in nodes.iter() {
        if is_blank(n) {
            // Пробел внутри строчного пробега — его часть, вне — пропуск.
            if run != Run::None
                && let Some(g) = groups.last_mut()
            {
                g.push(n.clone());
            }
            continue;
        }
        let (fl, il) = (floated(n), inline_level(n));
        let join = match run {
            Run::None => false,
            Run::Inline => il || fl,
            Run::Float => true,
        };
        match groups.last_mut() {
            Some(g) if join => g.push(n.clone()),
            _ => groups.push(vec![n.clone()]),
        }
        run = if fl || (run == Run::Float && il) {
            Run::Float
        } else if il {
            Run::Inline
        } else {
            Run::None
        };
    }
}
