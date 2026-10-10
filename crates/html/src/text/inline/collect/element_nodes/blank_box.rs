//! Blank box for element_nodes; split out to keep the owning module within 250 lines.

use crate::dom::{Element, Node};
use crate::style::computed::Computed;
use crate::style::values::value::Len;
use crate::text::inline::*;
use gpui::Styled;

pub(super) fn blank_box(
    e: &Element,
    merged: &Computed,
    inherited: &Computed,
    painted_bg: bool,
    lead: f32,
    trail: f32,
    out: &mut Vec<Piece>,
) -> bool {
    // ПУСТАЯ строчная коробка: прогона текста у неё нет, а
    // отступ, рамку и фон рисовать надо (§8.4). Распорка их не
    // красит (см. запись у `spacer_style`), поэтому коробка идёт
    // отдельным слоем — он места в строке не занимает (место
    // держат распорки) и высоту строки не меняет (§10.8: поля,
    // отступы и рамки строчного в неё не входят).
    let mut inner_text = String::new();
    crate::render::gather_text_public(&e.children, &mut inner_text);
    // Из одних СХЛОПЫВАЕМЫХ пробелов — тоже пустая: пробелы
    // схлопнутся в соседний или срежутся у края строки (§4.1.1,
    // §4.1.3), а рамка без фона прогоном не рисуется
    // (`line-edge-white-space-collapse-001/002`: зелёная рамка
    // `<span>  </span>` пропадала).
    // Коробка с элементами внутри (атомы, `<br>`) не пустая, даже
    // без текста: слой нарисовал бы её одной строкой поверх
    // настоящих фрагментов (`border-radius-012`).
    let only_text = e.children.iter().all(|n| matches!(n, Node::Text(_)));
    let blank = only_text
        && (inner_text.is_empty()
            || (merged.keep_spaces != Some(true)
                && !painted_bg
                && inner_text
                    .chars()
                    .all(|c| matches!(c, ' ' | '\t' | '\n' | '\r'))));
    if blank && (lead != 0.0 || trail != 0.0) {
        let px_of = |l: Option<Len>| match l {
            Some(Len::Px(v)) => v,
            _ => 0.0,
        };
        let size = match merged.font_size {
            Some(Len::Px(v)) => v,
            _ => 16.0,
        };
        let bs = e.style.borders();
        let padding = e.style.padding;
        let flat = physical_sides::project(
            inherited,
            [
                px_of(padding.top) + px_of(bs.top),
                px_of(padding.right) + px_of(bs.right),
                px_of(padding.bottom) + px_of(bs.bottom),
                px_of(padding.left) + px_of(bs.left),
            ],
        );
        let top = flat[0];
        // Высота области содержимого — подъём плюс спуск шрифта, как
        // у полосы непустого куска (`run_background_quad`); кегль
        // вместо неё оставлял под пустой коробкой светлую черту
        // рядом с полосой соседа (`word-spacing-characters-001`).
        let family = merged.font_family.clone().unwrap_or_else(|| {
            if merged.monospace == Some(true) {
                crate::text::metrics::mono_family_for(merged.lang.as_deref()).to_string()
            } else {
                String::new()
            }
        });
        let (asc, desc, _) = crate::text::metrics::vmetrics_px(&family, size);
        let content = if asc + desc > 0.0 { asc + desc } else { size };
        let line = match merged.line_height {
            Some(Len::Px(v)) => v,
            Some(Len::Em(k)) => k * size,
            _ => size * crate::text::metrics::normal_line(&family),
        };
        // Коробка стоит на области содержимого: она в середине
        // строки, а полулидинг делит остаток поровну (§10.8).
        let dy = ((line - content) / 2.0 - top).max(-line);
        let mut copy = e.clone();
        copy.style.width = Some(Len::Px(0.0));
        copy.style.height = Some(Len::Px(content));
        copy.style.margin = Default::default();
        copy.style.position = None;
        copy.style.display = None;
        // Размер коробки читается из СЛИТОГО стиля: без этого
        // заданные выше ширина и высота терялись, и коробка
        // выходила нулевой высоты — рамка `border-left` и фон
        // под отступом не рисовались вовсе.
        let mut sized = merged.clone();
        sized.width = copy.style.width;
        sized.height = copy.style.height;
        sized.margin = Default::default();
        sized.position = None;
        physical_sides::project_box(inherited, &mut copy.style);
        physical_sides::project_box(inherited, &mut sized);
        let boxel = crate::render::styled_div_with(&copy, &sized)
            .absolute()
            .top(gpui::px(dy));
        out.push(Piece::Overlay(
            boxel.into_any_element(),
            OverlayAt::default(),
        ));
    }
    blank
}
