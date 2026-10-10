//! Переклад темы gpui-component под ThemeKind: Theme::change (полный
//! dark/light сет, включая highlight-тему редактора) + наши оверрайды
//! (инпуты, скроллбары, панель поиска, сплиттеры) и слияние tokenColors
//! contributed-темы в подсветку редактора.

use gpui::App;
use gpui_component::theme::{Theme, ThemeMode};
use kamin_theme::ThemeKind;

use super::syntax::contrib_syntax;
use crate::colors;

/// Полный переклад темы компонентов под ThemeKind.
pub fn apply(kind: ThemeKind, cx: &mut App) {
    kamin_theme::set_current_kind(kind);
    let mode = match kind {
        ThemeKind::Dark => ThemeMode::Dark,
        ThemeKind::Light => ThemeMode::Light,
    };
    Theme::change(mode, None, cx);

    let p = kind.palette();
    // 0.7.1: только `Theme::update` сводит цвета в `tokens` (их читают Root/скроллбары).
    Theme::update(cx, |theme| {
        // Инпуты/редактор в цветах IDE
        // Фон Input-редактора = editor.background темы (was bg_primary — юзер
        // отметил неверный цвет фона эдитора)
        theme.colors.background = colors::rgba(p.editor_bg).into();
        theme.colors.input = {
            let mut c = colors::rgba(p.bg_overlay);
            c.a = 0.4;
            c.into()
        };
        // `.input { color: var(--text-primary) }` — цвет текста инпутов
        // gpui-component берёт отсюда; без синка оставался крейтовый #fafafa
        // (ревью ц.16)
        theme.colors.foreground = colors::rgba(p.text_primary).into();
        theme.colors.muted = colors::rgba(p.bg_surface).into();
        theme.colors.muted_foreground = colors::rgba(p.text_muted).into();
        theme.colors.ring = {
            let mut c = colors::rgba(p.accent_primary);
            c.a = 0.35;
            c.into()
        };
        // Highlight-тема редактора (gutter/фон/номера строк): дефолтная
        // default_dark чужая — перекладываем токены палитры IDE
        {
            let mut ht = (*theme.highlight_theme).clone();
            ht.style.editor_background = Some(colors::rgba(p.editor_bg).into());
            ht.style.editor_foreground = Some(colors::rgba(p.editor_fg).into());
            ht.style.editor_line_number = Some(colors::rgba(p.text_muted).into());
            ht.style.editor_active_line_number = Some(colors::rgba(p.text_primary).into());
            ht.style.editor_active_line = Some({
                let mut c = colors::rgba(p.bg_surface);
                c.a = 0.35;
                c.into()
            });
            // tokenColors contributed-темы поверх builtin-подсветки (#71):
            // merge через serde-roundtrip — SyntaxColors ~30 полей, пополе
            // мержить руками = дрейф при апгрейде вендора.
            if let Some(over) = contrib_syntax()
                && let Ok(mut base_v) = serde_json::to_value(&ht.style.syntax)
            {
                if let (Some(bo), Some(oo)) = (base_v.as_object_mut(), over.as_object()) {
                    for (k, val) in oo {
                        bo.insert(k.clone(), val.clone());
                    }
                }
                match serde_json::from_value(base_v) {
                    Ok(s) => ht.style.syntax = s,
                    Err(e) => eprintln!("[theme] contributed syntax merge failed: {e}"),
                }
            }
            theme.highlight_theme = std::sync::Arc::new(ht);
        }
        // Скроллбары (global.css 1:1): постоянный сплошной thumb bg-overlay,
        // hover text-disabled, трек прозрачный. В СВЕТЛОЙ палки вдвое светлее
        // (полу-альфа над светлым фоном) — сплошной bg-overlay читался слишком
        // тёмным (запрос юзера).
        theme.colors.scrollbar = gpui::transparent_black();
        theme.colors.scrollbar_thumb = {
            let mut c = colors::rgba(p.bg_overlay);
            if kind == ThemeKind::Light {
                c.a = 0.5;
            }
            c.into()
        };
        theme.colors.scrollbar_thumb_hover = {
            let mut c = colors::rgba(p.text_disabled);
            if kind == ThemeKind::Light {
                c.a = 0.5;
            }
            c.into()
        };
        theme.scrollbar_mode = gpui_component::scroll::ScrollbarMode::Always;
        // Панель поиска редактора (`input/search.rs`) рисует себя как
        // `bg(popover)` + `border_b_1(border)`, а кнопки замены/prev/next — как
        // `secondary`. Ни одно из этих полей мы не пробрасывали, поэтому бар
        // выглядел чужим: почти чёрный фон крейта, без рамок, кнопки не читались
        // (баг найден юзером). Кладём наши поверхности.
        theme.colors.popover = colors::rgba(p.bg_surface).into();
        theme.colors.popover_foreground = colors::rgba(p.text_primary).into();
        theme.colors.secondary = {
            let mut c = colors::rgba(p.text_primary);
            c.a = 0.06;
            c.into()
        };
        theme.colors.secondary_hover = {
            let mut c = colors::rgba(p.text_primary);
            c.a = 0.10;
            c.into()
        };
        theme.colors.secondary_active = {
            let mut c = colors::rgba(p.accent_primary);
            c.a = 0.22;
            c.into()
        };
        theme.colors.secondary_foreground = colors::rgba(p.text_secondary).into();
        theme.colors.accent = {
            let mut c = colors::rgba(p.accent_primary);
            c.a = 0.16;
            c.into()
        };
        theme.colors.accent_foreground = colors::rgba(p.text_primary).into();
        theme.colors.list_hover = {
            let mut c = colors::rgba(p.text_primary);
            c.a = 0.10;
            c.into()
        };
        // Сплиттеры: зазор чистый, drag = accent
        theme.colors.border = gpui::transparent_black();
        theme.colors.drag_border = colors::rgba(p.accent_primary).into();
    });
}
