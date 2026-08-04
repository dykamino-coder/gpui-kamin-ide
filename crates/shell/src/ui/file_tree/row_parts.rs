//! Содержимое строки дерева: шеврон, иконка, лейбл, бейдж декорации.
//!
//! Вынесено из `row.rs` без изменения поведения (`plan/100-refactor-250.md`).

use crate::colors::rgba;
use crate::probe::registry::probe_area;
use crate::ui::file_tree::model::{DirEntry, TreeState, deco_color};
use crate::ui::icon::{CHEVRON_DOWN, CHEVRON_RIGHT, codicon};
use gpui::prelude::*;
use gpui::{AnyElement, Rgba, SharedString, div, px};
use kamin_metrics as m;
use kamin_theme::Palette;

/// Шеврон папки (или спиннер загрузки) — `.chevron` шириной 16.
/// У файла вместо него `.chevronSpacer` той же ширины.
pub(crate) fn chevron(
    e: &DirEntry,
    path: &str,
    expanded: bool,
    loading: bool,
    is_selected: bool,
    panel_key: &'static str,
    p: &Palette,
) -> gpui::Div {
    if !e.is_dir {
        // `.chevronSpacer` — та же ширина 16
        return div().flex_shrink_0().w(px(16.0)).h(px(16.0));
    }
    // `.chevron` — width 16, text-align center, font-size 13;
    // у выделенной строки цвет наследуется (`.rowSelected .chevron`)
    let glyph_color = if is_selected {
        rgba(p.text_primary)
    } else {
        rgba(p.text_muted)
    };
    let mut ch = div().flex_shrink_0().w(px(16.0)).flex().justify_center();
    ch = if loading {
        // `codicon-loading codicon-modifier-spin` — глиф КРУТИТСЯ
        // (ревью ц.24: рисовали статичный)
        ch.child(crate::ui::icon::spinner(
            SharedString::from(format!("spin:{panel_key}:{path}")),
            16.0,
            glyph_color,
        ))
    } else {
        ch.child(codicon(
            if expanded {
                CHEVRON_DOWN
            } else {
                CHEVRON_RIGHT
            },
            // `.chevron{font-size:13px}` (0,1,0) проигрывает базе
            // `.codicon[class*=codicon-]` (0,2,0) → 16 (ревью ц.13)
            16.0,
        ))
    };
    if !is_selected {
        ch = ch.text_color(rgba(p.text_muted));
    }
    ch
}

/// Иконка 16×16: contributed icon-тема или Catppuccin.
pub(crate) fn glyph(e: &DirEntry, expanded: bool) -> gpui::Img {
    if e.is_dir {
        crate::icon_theme::folder_img(&e.name, expanded, false)
    } else {
        crate::icon_theme::file_img(&e.name)
    }
    .flex_shrink_0()
    .w(px(16.0))
    .h(px(16.0))
}

/// Имя узла: цвет декорации, тултип и усечение по ширине панели.
#[allow(clippy::too_many_arguments)]
pub(crate) fn label(
    e: &DirEntry,
    path: &str,
    tree: &TreeState,
    depth: f32,
    panel_w: f32,
    panel_key: &'static str,
    row_group: SharedString,
    hover_fg: Rgba,
    p: &Palette,
) -> gpui::Stateful<gpui::Div> {
    let deco = tree.deco.get(path).and_then(|d| d.as_ref());
    let name_color = deco
        .and_then(|d| d.color.as_deref())
        .map(|id| rgba(deco_color(id, p)));
    let mut label = div()
        .id(SharedString::from(format!("lbl:{panel_key}:{path}")))
        // `.row:hover { color: text-primary }` красит лейбл;
        // ховер строки до вложенного текста не доходит (ц.21)
        .group_hover(row_group, move |st| st.text_color(hover_fg))
        .flex_1()
        .min_w(px(0.))
        .overflow_hidden()
        .text_ellipsis()
        .whitespace_nowrap()
        // `data-tooltip={deco?.tooltip ?? path}` — усечённое имя
        // иначе нечем прочитать; подсказка декорации приоритетнее
        // пути (ревью ц.14)
        .tooltip(crate::ui::tooltip::tooltip(
            deco.and_then(|d| d.tooltip.clone())
                .unwrap_or_else(|| path.to_string()),
        ))
        // `text-overflow: ellipsis`: многоточие в порте дописывает
        // `text_fit`, движок сам его не рисует. Ширину панели
        // берём из probe прошлого кадра, бюджет — минус индент,
        // шеврон 16, иконка 16, два гэпа 6 и pr 8 (ревью ц.21)
        .child(crate::ui::text_fit::fit_approx(
            &e.name,
            panel_w - (depth * 12.0 + 8.0) - 52.0,
            m::FS_SM,
        ));
    if let Some(c) = name_color {
        label = label.text_color(c);
    }
    label
}

/// Badge декорации (git: M/U/A…) справа, цветом декорации.
///
/// Без бейджа НЕ рендерим ничего: пустой `div` оставался flex-элементом и
/// съедал `gap 6` у лейбла (ревью ц.15).
pub(crate) fn badge(
    tree: &TreeState,
    path: &str,
    panel_key: &'static str,
    row_group: SharedString,
    hover_fg: Rgba,
    p: &Palette,
) -> Option<AnyElement> {
    let deco = tree.deco.get(path).and_then(|d| d.as_ref());
    let badge = deco.and_then(|d| d.badge.clone())?;
    let c = deco
        .and_then(|d| d.color.as_deref())
        .map(|id| rgba(deco_color(id, p)));
    Some(
        div()
            .id(SharedString::from(format!("badge:{panel_key}:{path}")))
            .relative()
            .child(probe_area("file-tree-row-badge"))
            .flex_shrink_0()
            .ml_auto()
            .pl(px(6.0))
            .text_size(px(m::FS_XS))
            .font_weight(gpui::FontWeight::SEMIBOLD)
            // `data-tooltip={deco.tooltip}` (ревью ц.14)
            .when_some(deco.and_then(|d| d.tooltip.clone()), |el, t| {
                el.tooltip(crate::ui::tooltip::tooltip(t))
            })
            // без ThemeColor цвет НАСЛЕДУЕТСЯ от строки, включая
            // `.row:hover { color: text-primary }` (ревью ц.25: ховер
            // до бейджа не доходил)
            .when(c.is_none(), |d| {
                d.group_hover(row_group, move |st| st.text_color(hover_fg))
            })
            .when_some(c, |d, c| d.text_color(c))
            .child(badge)
            .into_any_element(),
    )
}
