//! Синхронизация темы gpui-component с палитрой KaminIDE: Theme::change
//! (полный dark/light сет, включая highlight-тему редактора) + наши
//! оверрайды (скроллбары, инпуты, сплиттеры). Вызывается на старте и при
//! смене темы (Appearance-поповер).
//!
//! Contributed-маппер — ПОЛНЫЙ порт contributed-theme-resolve.ts:
//! elevation-ramp из авторских нейтральных поверхностей (backdrop → panel →
//! card → overlay, якорь на editor.background, light-темы инвертируют),
//! accent = самый НАСЫЩЕННЫЙ кандидат, muted/disabled — blend при отсутствии.

use crate::theme::resolve::resolve_palette;
use gpui::App;
use gpui_component::theme::{Theme, ThemeMode};
use kamin_theme::{Color, ThemeKind};

use crate::colors;

// Блендинг отсутствующих ключей (доли пути fg→bg) — как в оригинале
pub(crate) const MUTED_T: f32 = 0.42;
pub(crate) const DISABLED_T: f32 = 0.62;
pub(crate) const NEUTRAL_MAX_CHROMA: f32 = 0.25;
pub(crate) const MID_L: f32 = 0.5;
pub(crate) const MIN_SEP: f32 = 0.03;
pub(crate) const BACKDROP_NUDGE: f32 = 0.4;
pub(crate) const PANEL_NUDGE: f32 = 0.12;
pub(crate) const SURFACE_MAX_STEP: f32 = 0.09;
pub(crate) const OVERLAY_MAX_STEP: f32 = 0.13;

pub(crate) const BLACK: Color = Color::hex(0x000000);
pub(crate) const WHITE: Color = Color::hex(0xffffff);

/// Файл-кэш последней применённой contributed-темы: применяется СИНХРОННО на
/// старте, до загрузки расширения-поставщика — иначе бут шёл на дефолтной
/// теме до прихода реестра (жалоба юзера).
pub fn theme_cache_path() -> std::path::PathBuf {
    let (_, cache) = crate::host_link::data_dirs();
    cache.join("theme-cache.json")
}

/// Применить закэшированную contributed-тему на буте. true — применена;
/// false — кэша нет/бит (бут остаётся на builtin-выборе).
pub fn apply_cached_contributed(cx: &mut App) -> bool {
    let id = crate::layout_store::load_raw_key("contributedThemeId")
        .and_then(|v| v.as_str().map(str::to_string));
    if id.is_none() {
        return false;
    }
    let dark_ui = crate::layout_store::load_raw_key("contributedThemeDarkUi")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let path = theme_cache_path();
    if !path.exists() {
        return false;
    }
    apply_contributed(&path.to_string_lossy(), dark_ui, cx)
}

/// Оверлей SyntaxColors из tokenColors активной contributed-темы (JSON по
/// serde-ключам gpui-component). None — тема не contributed / без tokenColors:
/// редактор остаётся на builtin-подсветке. Чистится в SetThemeChoice.
static CONTRIB_SYNTAX: std::sync::Mutex<Option<serde_json::Value>> = std::sync::Mutex::new(None);

pub fn set_contrib_syntax(v: Option<serde_json::Value>) {
    *CONTRIB_SYNTAX.lock().unwrap() = v;
}

/// TextMate tokenColors → оверлей SyntaxColors (задача #71). Правила идут по
/// порядку, поздние переопределяют (семантика VS Code); селектор матчится
/// ДЛИННЕЙШИМ префиксом таблицы по границам точек. fontStyle: italic/underline
/// переносим, bold нет (формат font_weight у gpui-component другой).
fn token_colors_to_syntax(theme: &serde_json::Value) -> Option<serde_json::Value> {
    // TextMate-префикс → serde-ключи SyntaxColors gpui-component.
    const MAP: &[(&str, &[&str])] = &[
        ("comment.block.documentation", &["comment.doc"]),
        ("comment", &["comment"]),
        ("string.regexp", &["string.regex"]),
        ("constant.character.escape", &["string.escape"]),
        ("string", &["string"]),
        ("constant.numeric", &["number"]),
        ("constant.language", &["boolean", "constant"]),
        ("constant.other.symbol", &["string.special.symbol"]),
        ("constant", &["constant"]),
        ("keyword.operator", &["operator"]),
        ("keyword.control.directive", &["preproc"]),
        ("keyword", &["keyword"]),
        ("storage", &["keyword"]),
        ("entity.name.function", &["function"]),
        ("support.function", &["function"]),
        ("entity.name.type", &["type"]),
        ("entity.name.class", &["type"]),
        ("entity.other.inherited-class", &["type"]),
        ("support.type.property-name", &["property"]),
        ("support.type", &["type"]),
        ("support.class", &["type"]),
        ("entity.name.tag", &["tag"]),
        ("entity.other.attribute-name", &["attribute"]),
        ("entity.name.label", &["label"]),
        ("variable.other.property", &["property"]),
        ("support.variable.property", &["property"]),
        ("variable", &["variable"]),
        ("punctuation", &["punctuation"]),
        ("markup.heading", &["title"]),
        ("markup.italic", &["emphasis"]),
        ("markup.bold", &["emphasis.strong"]),
        ("markup.inline.raw", &["text.literal"]),
        ("markup.underline.link", &["link_uri"]),
        ("meta.preprocessor", &["preproc"]),
    ];
    let rules = theme.get("tokenColors")?.as_array()?;
    let mut out = serde_json::Map::new();
    for rule in rules {
        let Some(settings) = rule.get("settings") else {
            continue;
        };
        let fg = settings.get("foreground").and_then(|x| x.as_str());
        let fs = settings
            .get("fontStyle")
            .and_then(|x| x.as_str())
            .unwrap_or("");
        if fg.is_none() && fs.is_empty() {
            continue;
        }
        // scope: строка (возможно с запятыми) или массив строк. Правило БЕЗ
        // scope задаёт глобальный foreground редактора — не про подсветку.
        let mut scopes: Vec<String> = Vec::new();
        match rule.get("scope") {
            Some(serde_json::Value::String(s)) => {
                scopes.extend(s.split(',').map(|x| x.trim().to_string()));
            }
            Some(serde_json::Value::Array(a)) => {
                scopes.extend(a.iter().filter_map(|x| x.as_str()).map(str::to_string));
            }
            _ => continue,
        }
        for sel in &scopes {
            let mut best: Option<&(&str, &[&str])> = None;
            for e in MAP {
                let pfx = e.0;
                let is_pfx = sel == pfx
                    || (sel.starts_with(pfx) && sel.as_bytes().get(pfx.len()) == Some(&b'.'));
                if is_pfx && best.is_none_or(|b| pfx.len() > b.0.len()) {
                    best = Some(e);
                }
            }
            let Some((_, keys)) = best else { continue };
            let mut style = serde_json::Map::new();
            if let Some(c) = fg {
                style.insert("color".into(), serde_json::json!(c));
            }
            if fs.contains("italic") {
                style.insert("font_style".into(), serde_json::json!("italic"));
            } else if fs.contains("underline") {
                style.insert("font_style".into(), serde_json::json!("underline"));
            }
            if style.is_empty() {
                continue;
            }
            for k in *keys {
                out.insert((*k).to_string(), serde_json::Value::Object(style.clone()));
            }
        }
    }
    if out.is_empty() {
        None
    } else {
        Some(serde_json::Value::Object(out))
    }
}

/// Contributed VS Code-тема: JSON colors → Palette через полный resolve-порт.
/// Возвращает false если файл не прочитался.
pub fn apply_contributed(path: &str, dark_ui: bool, cx: &mut App) -> bool {
    let Ok(text) = std::fs::read_to_string(path) else {
        return false;
    };
    // Кэш для мгновенного применения на следующем буте (см. выше). Пишем
    // ИСХОДНЫЙ текст темы: применение на буте пройдёт тот же resolve-путь.
    {
        let cache = theme_cache_path();
        if cache.to_string_lossy() != path {
            if let Some(dir) = cache.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let _ = std::fs::write(&cache, &text);
        }
    }
    // Тем-JSON бывает с комментариями (jsonc) — срежем // построчно
    let clean: String = text
        .lines()
        .map(|l| {
            if let Some(i) = l.find("//")
                && (!l[..i].contains('"') || l[..i].matches('"').count() % 2 == 0)
            {
                return &l[..i];
            }
            l
        })
        .collect::<Vec<_>>()
        .join("\n");
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&clean) else {
        return false;
    };
    let colors = v.get("colors").cloned().unwrap_or_default();
    let base = if dark_ui {
        ThemeKind::Dark
    } else {
        ThemeKind::Light
    };
    let p = resolve_palette(&colors, dark_ui, *base.palette_base());
    kamin_theme::set_contributed(Some(p));
    // Вебвью получают СЫРУЮ colors-семью темы поверх дефолтов (как оригинал
    // пробрасывает --vscode-* в webview) — рамп-палитра тут не годится:
    // чат вязался на backdrop вместо editor.background (скрин юзера).
    crate::ui::webview_theme::set_contrib_colors(colors.as_object().map(|m| {
        m.iter()
            .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
            .collect()
    }));
    // tokenColors → подсветка gpui-редактора (#71); apply() ниже вольёт
    // оверлей в highlight-тему.
    set_contrib_syntax(token_colors_to_syntax(&v));
    apply(base, cx);
    true
}

/// Полный переклад темы компонентов под ThemeKind.
pub fn apply(kind: ThemeKind, cx: &mut App) {
    kamin_theme::set_current_kind(kind);
    let mode = match kind {
        ThemeKind::Dark => ThemeMode::Dark,
        ThemeKind::Light => ThemeMode::Light,
    };
    Theme::change(mode, None, cx);

    let p = kind.palette();
    let theme = Theme::global_mut(cx);
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
        if let Some(over) = CONTRIB_SYNTAX.lock().unwrap().clone()
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
    theme.scrollbar_show = gpui_component::scroll::ScrollbarShow::Always;
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::color_math::lightness;
    use crate::theme::resolve::theme_hex_opaque;
    use serde_json::json;

    #[test]
    fn token_colors_prefix_match_order_and_shape() {
        let theme = json!({
            "tokenColors": [
                {"scope": "comment", "settings": {"foreground": "#11aa22", "fontStyle": "italic"}},
                {"scope": ["keyword.operator", "string"], "settings": {"foreground": "#334455"}},
                {"scope": "keyword, storage.type", "settings": {"foreground": "#667788"}},
                // Позднее и БОЛЕЕ специфичное правило переопределяет comment
                {"scope": "comment.line.double-slash", "settings": {"foreground": "#99aabb"}},
                // Без scope = глобальный foreground, в подсветку не идёт
                {"settings": {"foreground": "#000000"}},
                // "keywords" НЕ префикс "keyword" по границе точки
                {"scope": "keywordsomething", "settings": {"foreground": "#ff0000"}}
            ]
        });
        let v = token_colors_to_syntax(&theme).expect("overlay");
        assert_eq!(v["comment"]["color"], "#99aabb");
        assert_eq!(v["operator"]["color"], "#334455");
        assert_eq!(v["string"]["color"], "#334455");
        assert_eq!(v["keyword"]["color"], "#667788");
        assert!(v.get("function").is_none());
        // Оверлей обязан валидно десериализоваться в SyntaxColors вендора
        // (hex-строки → Hsla через Rgba).
        let _sc: gpui_component::highlighter::SyntaxColors =
            serde_json::from_value(v).expect("SyntaxColors roundtrip");
    }

    #[test]
    fn token_colors_none_without_rules() {
        assert!(token_colors_to_syntax(&json!({})).is_none());
        assert!(token_colors_to_syntax(&json!({"tokenColors": []})).is_none());
    }

    fn l_hex(c: Color) -> f32 {
        lightness(c)
    }

    #[test]
    fn ramp_dark_theme_orders_surfaces() {
        // 3 нейтрала: sideBar темнее editor, widget светлее (типичный dark)
        let colors = json!({
            "editor.background": "#1e1e2e",
            "sideBar.background": "#181825",
            "editorWidget.background": "#313244",
            "foreground": "#cdd6f4",
            "activityBarBadge.background": "#f38ba8",
        });
        let p = resolve_palette(&colors, true, kamin_theme::DARK);
        // Backdrop = самый тёмный нейтрал (sideBar)
        assert!(l_hex(p.bg_sidebar) <= l_hex(p.bg_mantle));
        // Панель темнее редактора (код-канва остаётся самой яркой)
        assert!(l_hex(p.bg_mantle) < l_hex(p.editor_bg));
        // Карточка светлее панели
        assert!(l_hex(p.bg_surface) > l_hex(p.bg_mantle));
        // Accent = сильнее всех насыщенный кандидат (розовый бейдж)
        assert!((p.accent_primary.r - 0xf3 as f32 / 255.0).abs() < 0.01);
    }

    #[test]
    fn ramp_single_neutral_synthesises_backdrop() {
        // editor == sideBar (один нейтрал) → backdrop сдвигается к чёрному
        let colors = json!({
            "editor.background": "#222233",
            "sideBar.background": "#222233",
            "foreground": "#ccccdd",
        });
        let p = resolve_palette(&colors, true, kamin_theme::DARK);
        assert!(
            (l_hex(p.bg_sidebar) - l_hex(p.bg_mantle)).abs() >= MIN_SEP,
            "backdrop must be nudged apart from panel"
        );
    }

    #[test]
    fn translucent_surface_excluded_from_ramp() {
        // list.activeSelectionBackground с альфой — тинт, не поверхность
        assert!(!theme_hex_opaque("#ffffff80"));
        assert!(theme_hex_opaque("#ffffffff"));
        assert!(theme_hex_opaque("#ffffff"));
    }

    #[test]
    fn muted_blend_when_missing() {
        let colors = json!({
            "editor.background": "#101020",
            "foreground": "#e0e0f0",
        });
        let p = resolve_palette(&colors, true, kamin_theme::DARK);
        // muted между fg и bg
        let lm = l_hex(p.text_muted);
        assert!(lm < l_hex(p.text_primary) && lm > l_hex(p.editor_bg));
    }

    #[test]
    fn light_theme_inverts_ramp() {
        let colors = json!({
            "editor.background": "#ffffff",
            "sideBar.background": "#f3f3f3",
            "editorWidget.background": "#ececec",
            "foreground": "#333333",
        });
        let p = resolve_palette(&colors, false, kamin_theme::LIGHT);
        // Light: backdrop = САМЫЙ СВЕТЛЫЙ нейтрал, панели темнее
        assert!(l_hex(p.bg_sidebar) >= l_hex(p.bg_mantle));
    }
}
