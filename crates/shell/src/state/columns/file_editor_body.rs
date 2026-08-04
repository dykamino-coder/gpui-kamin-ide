//! Тело редактора файла: sticky-scroll, breadcrumb, минимапа и скроллбар.
//!
//! Вынесено из `file_editor.rs` без изменения поведения
//! (`plan/100-refactor-250.md`).

use crate::colors::rgba;
use crate::state::model::RootView;
use gpui::prelude::*;
use gpui::{Context, Entity, div, px};
use gpui_component::Sizable as _;
use gpui_component::input::InputState;
use kamin_metrics as m;
use kamin_theme::Palette;

impl RootView {
    /// `input` и `apath` берутся у активного таба ДО вызова: держать `&mut self`
    /// и ссылку внутрь `self.ed.editor_tabs` одновременно нельзя.
    pub(crate) fn file_editor_body(
        &mut self,
        input: &Entity<InputState>,
        apath: String,
        cx: &mut Context<Self>,
        p: &'static Palette,
    ) -> gpui::Div {
        // Sticky-scroll: первая видимая строка из scroll-оффсета
        // (vendored gpui-component: pub scroll_handle)
        let off_y = f32::from(input.read(cx).scroll_handle.offset().y);
        let first_visible = ((-off_y) / crate::ui::sticky_scroll::EDITOR_LINE_H)
            .floor()
            .max(0.0) as usize;
        if self.ed.sticky_cache.0 != apath || self.ed.sticky_cache.1 != first_visible {
            let text = input.read(cx).value().to_string();
            let lines: Vec<&str> = text.lines().collect();
            let rows = crate::ui::sticky_scroll::compute(&lines, first_visible)
                .into_iter()
                .map(|i| (i, lines[i].to_string()))
                .collect();
            self.ed.sticky_cache = (apath.clone(), first_visible, rows);
        }
        let sticky =
            crate::ui::sticky_scroll::sticky_overlay(&self.ed.sticky_cache.2, &apath, p, &self.tx);
        // Breadcrumb Zed-стиля ВНУТРИ рамки редактора +
        // единственный (встроенный) скроллбар; самопальный
        // минимап-полосы удалён (юзер: «изобрёл своё гавно»)
        let home = std::env::var("USERPROFILE").unwrap_or_default();
        let disp = {
            let norm = apath.replace('/', "\\");
            if !home.is_empty() && norm.to_lowercase().starts_with(&home.to_lowercase()) {
                format!("~{}", &norm[home.len()..])
            } else {
                norm
            }
        };
        // `.error` вместо редактора, если файл не прочитался
        // (`MonacoEditor.tsx:345-347`): flex 1, центр, padding
        // space-5, accent-red, моно, fs-sm
        let read_error = self.ed.editor_errors.get(&apath).cloned();
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h(px(0.))
            // `.body { background: editor-bg; border-radius:
            // radius-md; overflow: hidden; padding: 8px 0 10px }`
            // — инсеты карты живут на `.viewer` выше
            .pt(px(8.0))
            .pb(px(10.0))
            .relative()
            .rounded(px(m::RADIUS_MD))
            .overflow_hidden()
            .when_some(read_error, |d, err| {
                d.child(
                    div()
                        .absolute()
                        .inset_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .p(px(m::SPACE_5))
                        .bg(rgba(p.editor_bg))
                        .font_family("JetBrains Mono")
                        .text_size(px(m::FS_SM))
                        .text_color(rgba(p.accent_red))
                        .child(gpui::SharedString::from(format!("Failed to open: {err}"))),
                )
            })
            .bg(
                if false
                /* KAMIN_VWV_PAINTDBG законстанчен OFF (диагностика plan/97) */
                {
                    // ДИАГ: жёлтый = рамка редактора file-режима
                    gpui::Rgba {
                        r: 1.0,
                        g: 1.0,
                        b: 0.0,
                        a: 1.0,
                    }
                } else {
                    rgba(p.editor_bg)
                },
            )
            .child(
                div()
                    .flex_shrink_0()
                    .h(px(24.0))
                    .flex()
                    .items_center()
                    .gap(px(m::SPACE_2))
                    .px(px(m::SPACE_3))
                    .text_size(px(m::FS_XS))
                    .font_family("JetBrains Mono")
                    .text_color(rgba(p.text_muted))
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .overflow_hidden()
                            .text_ellipsis()
                            .whitespace_nowrap()
                            .child(disp),
                    )
                    // Иконки действий справа (Zed toolbar): только живые
                    // действия — поиск в буфере и Locate в дереве.
                    .child(crate::ui::file_tree::header::tool_btn(
                        "editor-find-btn",
                        crate::ui::icon::SEARCH,
                        "Find in file (Ctrl+F)",
                        false,
                        p,
                        {
                            let tx = self.tx.clone();
                            move || {
                                let _ = tx.try_send(crate::host_link::ShellEvent::Ed(
                                    crate::host::events::EdEvent::EditorFind,
                                ));
                            }
                        },
                    ))
                    .child(crate::ui::file_tree::header::tool_btn(
                        "editor-locate-btn",
                        "\u{ebf8}", // codicon-target (как Locate дерева)
                        "Locate in file tree",
                        false,
                        p,
                        {
                            let tx = self.tx.clone();
                            move || {
                                let _ = tx.try_send(crate::host_link::ShellEvent::Ed(
                                    crate::host::events::EdEvent::LocateSelectedFile,
                                ));
                            }
                        },
                    )),
            )
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h(px(0.))
                    // Без `min_w: 0` ряд не сжимается ниже
                    // min-content своих детей: минимапа и
                    // скроллбар `flex_shrink_0`, а текстовая
                    // колонка тянет ширину самой длинной
                    // строки — карта редактора выезжала за
                    // свою колонку вправо, в правую панель
                    // (баг найден юзером)
                    .min_w(px(0.))
                    .overflow_hidden()
                    .child(
                        div()
                            .relative()
                            .flex_1()
                            .min_w(px(0.))
                            .h_full()
                            .font_family("JetBrains Mono")
                            // Monaco fontSize 13 (оригинал);
                            // Input берёт кегль из окна
                            .text_size(px(m::EDITOR_FONT_SIZE))
                            .child(
                                // appearance(false): без рамки/
                                // фона компонента
                                gpui_component::input::Input::new(input)
                                    .h_full()
                                    .appearance(false)
                                    // Monaco fontSize 13: у
                                    // `Input` свой
                                    // `input_text_size(size)`,
                                    // который перекрывает
                                    // кегль обёртки, а
                                    // `Size::Size` внутри
                                    // умножается на 0.875
                                    .with_size(gpui_component::Size::Size(px(
                                        m::EDITOR_FONT_SIZE / 0.875
                                    )))
                                    // единственный скроллбар —
                                    // наш, правее глиф-бара
                                    .hide_scrollbar(),
                            )
                            .children(sticky),
                    )
                    // Порядок как в Zed: текст → глиф-бар
                    // (минимапа) → скроллбар. Ничего не
                    // накладывается: три колонки в ряд.
                    .child(crate::ui::editor_minimap::minimap(
                        input,
                        self.ed.minimap_input.as_ref(),
                        p,
                    ))
                    .child(crate::ui::editor_minimap::scrollbar(
                        input,
                        {
                            // Диагностики активного файла для маркеров трека
                            // (ключ diags — путь в форме LSP-uri/пути; равняем
                            // сепараторы и регистр как Problems)
                            let norm = |s: &str| s.replace('\\', "/").to_lowercase();
                            let ap = norm(&apath);
                            let mut ms: Vec<(u32, u8)> = self
                                .diags
                                .iter()
                                .filter(|((_, uri), _)| norm(uri) == ap)
                                .flat_map(|(_, v)| v.iter().map(|d| (d.line, d.severity)))
                                .collect();
                            ms.sort_unstable();
                            ms.dedup();
                            ms.truncate(500);
                            ms
                        },
                        p,
                    )),
            )
    }
}
