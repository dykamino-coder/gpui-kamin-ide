//! Тема-блок вебвью (--vscode-* переменные) — генерируется из АКТИВНОЙ
//! палитры kamin-theme (маппинг = vscode-colors.css оригинала). Блок
//! вставляется при ОТДАЧЕ страницы (`web/scheme.rs`), поэтому перезагрузка
//! вью всегда получает текущую тему; живым вью смена темы доставляется
//! `push_live()` (postMessage `__kaminTheme` — страница слушает его в
//! скрипте блока и переписывает `#__kaminThemeVars` без перезагрузки).

use std::sync::{LazyLock, Mutex};

use kamin_theme::{Color, Palette};

/// Сырые `colors` активной contributed-темы (ключ VS Code → значение).
/// Оригинал (contributed-theme.ts::applyVars) пробрасывает В ВЕБВЬЮ ВСЮ
/// colors-семью как `--vscode-<key dot→dash>` ПОВЕРХ дефолтов — вебвью моста
/// вяжет свои токены на эти vars (kamin-bridge.css: mantle→editor-background,
/// surface→menu/dropdown/editorWidget). Рамп-палитра gpui для чата давала
/// backdrop (#010409 у Claude Bridge Dark) вместо editor.background #0d1117 —
/// «чат весь тёмный» (скрин юзера vs Tauri 0.2.87).
static CONTRIB_COLORS: LazyLock<Mutex<Option<Vec<(String, String)>>>> =
    LazyLock::new(|| Mutex::new(None));

/// Задать/сбросить сырые colors contributed-темы (theme_sync).
pub fn set_contrib_colors(colors: Option<Vec<(String, String)>>) {
    *CONTRIB_COLORS.lock().unwrap() = colors;
}

/// `#rrggbb` (альфа игнорируется — для непрозрачных токенов палитры).
fn hx(c: Color) -> String {
    let q = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!("#{:02x}{:02x}{:02x}", q(c.r), q(c.g), q(c.b))
}

/// `rgba(r,g,b,a)` — токен палитры с заданной прозрачностью.
fn ra(c: Color, a: f32) -> String {
    let q = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!("rgba({},{},{},{a})", q(c.r), q(c.g), q(c.b))
}

/// Список `--vscode-*`/`--editor-*` переменных БЕЗ `color-scheme` (страница
/// дописывает его сама в обработчике `__kaminTheme`). База — маппинг палитры
/// (= vscode-colors.css); при contributed теме поверх — сырой проброс её
/// colors (как в оригинале).
pub fn css_vars(p: &Palette, light: bool) -> String {
    let mut vars: Vec<(String, String)> = Vec::with_capacity(140);
    let mut put = |k: &str, v: String| {
        vars.push((format!("--vscode-{k}"), v));
    };
    put("foreground", hx(p.text_primary));
    put("disabledForeground", hx(p.text_disabled));
    put("descriptionForeground", hx(p.text_muted));
    put("errorForeground", hx(p.accent_red));
    put("focusBorder", hx(p.accent_primary));
    put("contrastBorder", "transparent".into());
    put("contrastActiveBorder", hx(p.accent_primary));
    put("icon-foreground", hx(p.text_secondary));
    put("selection-background", ra(p.accent_primary, 0.30));
    put("editor-background", hx(p.bg_primary));
    put("editor-foreground", hx(p.text_primary));
    put("editor-selectionBackground", ra(p.accent_primary, 0.30));
    put(
        "editor-inactiveSelectionBackground",
        ra(p.accent_primary, 0.12),
    );
    put(
        "editor-lineHighlightBackground",
        ra(p.bg_surface, if light { 0.60 } else { 0.35 }),
    );
    put("editor-findMatchBackground", ra(p.accent_yellow, 0.30));
    put(
        "editor-findMatchHighlightBackground",
        ra(p.accent_yellow, 0.18),
    );
    put("editorCursor-foreground", hx(p.accent_primary));
    put("editorWhitespace-foreground", hx(p.text_disabled));
    put("editorLineNumber-foreground", hx(p.text_muted));
    put("editorLineNumber-activeForeground", hx(p.text_primary));
    put("editorIndentGuide-background", ra(p.bg_overlay, 0.30));
    put(
        "editorIndentGuide-activeBackground",
        ra(p.accent_primary, 0.40),
    );
    put("editorWidget-background", hx(p.bg_mantle));
    put("editorWidget-foreground", hx(p.text_primary));
    put("editorWidget-border", ra(p.bg_overlay, 0.50));
    put("editorHoverWidget-background", hx(p.bg_mantle));
    put("editorHoverWidget-border", ra(p.bg_overlay, 0.50));
    put("editorGroup-border", ra(p.bg_surface, 0.50));
    put("editorGroupHeader-tabsBackground", hx(p.bg_mantle));
    put("editorGroupHeader-tabsBorder", "transparent".into());
    put("tab-activeBackground", hx(p.bg_primary));
    put("tab-inactiveBackground", hx(p.bg_mantle));
    put("tab-activeForeground", hx(p.text_primary));
    put("tab-inactiveForeground", hx(p.text_muted));
    put("tab-border", "transparent".into());
    put("tab-activeBorderTop", hx(p.accent_primary));
    put("tab-hoverBackground", hx(p.bg_surface));
    put("sideBar-background", hx(p.bg_sidebar));
    put("sideBar-foreground", hx(p.text_primary));
    put("sideBar-border", "transparent".into());
    put("sideBarSectionHeader-background", "transparent".into());
    put("sideBarSectionHeader-foreground", hx(p.text_muted));
    put("sideBarTitle-foreground", hx(p.text_muted));
    put("activityBar-background", hx(p.bg_sidebar));
    put("activityBar-foreground", hx(p.text_primary));
    put("activityBar-inactiveForeground", hx(p.text_muted));
    put("activityBar-activeBorder", hx(p.accent_primary));
    put("titleBar-activeBackground", "transparent".into());
    put("titleBar-activeForeground", hx(p.text_primary));
    put("titleBar-inactiveBackground", "transparent".into());
    put("titleBar-inactiveForeground", hx(p.text_muted));
    put("statusBar-background", "transparent".into());
    put("statusBar-foreground", hx(p.text_muted));
    put("statusBar-border", "transparent".into());
    put("statusBarItem-hoverBackground", ra(p.bg_surface, 0.60));
    put("statusBarItem-prominentBackground", hx(p.accent_primary));
    put("statusBarItem-prominentForeground", hx(p.bg_primary));
    put("panel-background", hx(p.bg_mantle));
    put("panel-border", ra(p.bg_overlay, 0.30));
    put("panelTitle-activeForeground", hx(p.text_primary));
    put("panelTitle-activeBorder", hx(p.accent_primary));
    put("panelTitle-inactiveForeground", hx(p.text_muted));
    put("button-background", hx(p.accent_action));
    put("button-foreground", hx(p.accent_action_fg));
    put("button-hoverBackground", hx(p.accent_action_hover));
    put("button-secondaryBackground", hx(p.bg_surface));
    put("button-secondaryForeground", hx(p.text_primary));
    put("button-secondaryHoverBackground", hx(p.bg_overlay));
    put("input-background", hx(p.bg_surface));
    put("input-foreground", hx(p.text_primary));
    put("input-border", ra(p.bg_overlay, 0.40));
    put("input-placeholderForeground", hx(p.text_muted));
    put("inputOption-activeBackground", ra(p.accent_primary, 0.18));
    put("inputOption-activeBorder", hx(p.accent_primary));
    put("inputOption-activeForeground", hx(p.accent_primary));
    put("inputValidation-errorBorder", hx(p.accent_red));
    put("inputValidation-warningBorder", hx(p.accent_yellow));
    put("inputValidation-infoBorder", hx(p.accent_blue));
    put("list-activeSelectionBackground", ra(p.accent_primary, 0.18));
    put("list-activeSelectionForeground", hx(p.text_primary));
    put("list-inactiveSelectionBackground", ra(p.bg_surface, 0.60));
    put("list-inactiveSelectionForeground", hx(p.text_primary));
    put("list-hoverBackground", ra(p.bg_surface, 0.50));
    put("list-hoverForeground", hx(p.text_primary));
    put("list-focusOutline", hx(p.accent_primary));
    put("list-emptyBackground", "transparent".into());
    put("notifications-background", hx(p.bg_mantle));
    put("notifications-foreground", hx(p.text_primary));
    put("notifications-border", ra(p.bg_overlay, 0.50));
    put("notificationCenterHeader-background", hx(p.bg_sidebar));
    put("notificationLink-foreground", hx(p.accent_primary));
    put("quickInput-background", hx(p.bg_mantle));
    put("quickInput-foreground", hx(p.text_primary));
    put("quickInputTitle-background", hx(p.bg_mantle));
    put("pickerGroup-foreground", hx(p.text_muted));
    put("pickerGroup-border", ra(p.bg_surface, 0.60));
    put("dropdown-background", hx(p.bg_mantle));
    put("dropdown-foreground", hx(p.text_primary));
    put("dropdown-border", ra(p.bg_overlay, 0.50));
    put("scrollbar-shadow", "rgba(0, 0, 0, 0.3)".into());
    put("scrollbarSlider-background", hx(p.bg_overlay));
    put("scrollbarSlider-hoverBackground", hx(p.text_disabled));
    put("scrollbarSlider-activeBackground", hx(p.text_muted));
    put("textLink-foreground", hx(p.accent_blue));
    put("textLink-activeForeground", hx(p.accent_sapphire));
    put("textBlockQuote-background", ra(p.bg_surface, 0.40));
    put("textBlockQuote-border", hx(p.accent_primary));
    put("textCodeBlock-background", ra(p.bg_surface, 0.60));
    put("textPreformat-foreground", hx(p.accent_orange));
    put("editorError-foreground", hx(p.accent_red));
    put("editorWarning-foreground", hx(p.accent_yellow));
    put("editorInfo-foreground", hx(p.accent_primary));
    put("editorHint-foreground", hx(p.accent_sapphire));
    put("badge-background", hx(p.accent_primary));
    put("badge-foreground", hx(p.bg_primary));
    put("progressBar-background", hx(p.accent_primary));
    put("terminal-background", hx(p.bg_sidebar));
    put(
        "terminal-foreground",
        hx(if light { p.editor_fg } else { p.text_primary }),
    );
    put(
        "terminalCursor-foreground",
        hx(if light {
            p.accent_action_hover
        } else {
            p.accent_rosewater
        }),
    );
    // Токены xterm/Monaco моста — читаются страницами как есть
    vars.push(("--editor-bg".into(), hx(p.editor_bg)));
    vars.push(("--editor-fg".into(), hx(p.editor_fg)));
    vars.push(("--editor-cursor".into(), hx(p.editor_cursor)));
    vars.push((
        "--font-mono".into(),
        "'JetBrains Mono',Consolas,monospace".into(),
    ));
    vars.push((
        "--vscode-font-family".into(),
        "'Bricolage Grotesque',system-ui,sans-serif".into(),
    ));
    vars.push(("--vscode-font-size".into(), "13px".into()));
    // Contributed тема: сырой проброс ВСЕЙ colors-семьи поверх дефолтов
    // (заменяет совпавшие имена, добавляет отсутствующие в базе ключи —
    // menu.*, list.*, tab.* и прочие ~600 идентификаторов темы).
    if let Some(colors) = CONTRIB_COLORS.lock().unwrap().as_ref() {
        for (k, v) in colors {
            let name = format!("--vscode-{}", k.replace('.', "-"));
            match vars.iter_mut().find(|(n, _)| *n == name) {
                Some(e) => e.1 = v.clone(),
                None => vars.push((name, v.clone())),
            }
        }
    }
    let mut s = String::with_capacity(4096);
    for (i, (n, v)) in vars.iter().enumerate() {
        if i > 0 {
            s.push(';');
        }
        s.push_str(n);
        s.push(':');
        s.push_str(v);
    }
    s
}

/// Полный тема-блок для вставки ПЕРЕД HTML страницы: переменные, базовые
/// стили страницы и скрипт (класс vscode-dark/light + приём `__kaminTheme`).
pub fn theme_block(p: &Palette, light: bool) -> String {
    let scheme = if light { "light" } else { "dark" };
    let kind = if light { "vscode-light" } else { "vscode-dark" };
    format!(
        concat!(
            r#"<style id="__kaminThemeVars">:root{{{vars};color-scheme:{scheme};}}</style>"#,
            "<style>html,body{{color:var(--vscode-foreground);",
            "background-color:var(--vscode-editor-background);",
            "font-family:var(--vscode-font-family,system-ui,sans-serif);",
            "font-size:var(--vscode-font-size,13px);margin:0;padding:0 20px;}}",
            "a{{color:var(--vscode-textLink-foreground);text-decoration:none;}}",
            "a:hover{{color:var(--vscode-textLink-activeForeground);text-decoration:underline;}}",
            "code,pre{{background-color:var(--vscode-textCodeBlock-background);}}</style>",
            "<script>var __k='{kind}';",
            "function __ab(k){{document.documentElement.classList.remove('vscode-dark','vscode-light');",
            "document.documentElement.classList.add(k);",
            "document.body&&document.body.classList.remove('vscode-dark','vscode-light');",
            "document.body&&document.body.classList.add(k);}}__ab(__k);",
            "document.addEventListener('DOMContentLoaded',function(){{__ab(__k)}});",
            "window.addEventListener('message',function(e){{var d=e.data;",
            "if(!d||!d.__kaminTheme)return;var s=document.getElementById('__kaminThemeVars');",
            "if(s)s.textContent=':root{{'+d.css+';color-scheme:'+(d.kind==='vscode-light'?'light':'dark')+';}}';",
            "__k=d.kind;__ab(d.kind);}});</script>"
        ),
        vars = css_vars(p, light),
        scheme = scheme,
        kind = kind,
    )
}

/// Доставить АКТИВНУЮ тему во все живые вью без перезагрузки: страница ловит
/// `__kaminTheme` (слушатель из [`theme_block`]) и переписывает переменные.
/// Звать после `theme_sync::apply` (палитра уже переключена).
pub fn push_live() {
    let p = kamin_theme::current_palette();
    let light = kamin_theme::current_is_light();
    let kind = if light { "vscode-light" } else { "vscode-dark" };
    let css = serde_json::Value::String(css_vars(p, light)).to_string();
    let js = format!("window.postMessage({{__kaminTheme:1,css:{css},kind:'{kind}'}},'*')");
    let ids = crate::web::all_view_ids();
    eprintln!("[theme] push_live {kind} → {ids:?} ({} байт js)", js.len());
    for id in ids {
        crate::web::execute_script(&id, &js);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dark_vars_match_palette() {
        let vars = css_vars(&kamin_theme::DARK, false);
        // Ключевые токены тёмной палитры (сгенерированный блок 1:1 со старым
        // статическим THEME_BLOCK_DARK)
        assert!(vars.contains("--vscode-foreground:#cfd4e2;"));
        assert!(vars.contains("--vscode-editor-background:#313240;"));
        assert!(vars.contains("--vscode-selection-background:rgba(137,180,250,0.3);"));
        assert!(vars.contains("--editor-bg:#1d1c25;"));
        // color-scheme страница дописывает сама — в vars его нет
        assert!(!vars.contains("color-scheme"));
    }

    #[test]
    fn block_carries_kind_and_listener() {
        let dark = theme_block(&kamin_theme::DARK, false);
        assert!(dark.contains("var __k='vscode-dark'"));
        assert!(dark.contains("color-scheme:dark"));
        assert!(dark.contains("__kaminTheme"));
        let light = theme_block(&kamin_theme::LIGHT, true);
        assert!(light.contains("var __k='vscode-light'"));
        assert!(light.contains("color-scheme:light"));
    }
}
