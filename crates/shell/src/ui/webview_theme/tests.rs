//! Тесты генерации тема-блока вебвью.

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

#[test]
fn menu_surface_matches_native_menu() {
    // Нативное меню: bg_surface + рамка text_primary@0.06 (ui/context_menu.rs)
    let dark = css_vars(&kamin_theme::DARK, false);
    assert!(dark.contains("--kamin-menu-background:#3d3f51;"));
    assert!(dark.contains("--kamin-menu-border:rgba(207,212,226,0.06);"));
    let light = css_vars(&kamin_theme::LIGHT, true);
    assert!(light.contains("--kamin-menu-background:#e6e1d4;"));
    assert!(light.contains("--kamin-menu-border:rgba(50,46,40,0.06);"));
}

#[test]
fn contributed_menu_keys_do_not_override_native_menu_surface() {
    set_contrib_colors(Some(vec![("menu.background".into(), "#ff0000".into())]));
    let vars = css_vars(&kamin_theme::DARK, false);
    set_contrib_colors(None);
    assert!(vars.contains("--vscode-menu-background:#ff0000"));
    assert!(vars.contains("--kamin-menu-background:#3d3f51;"));
}
