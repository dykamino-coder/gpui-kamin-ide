//! Картинки icon-темы: файл, папка, ассет.
//!
//! Перенесено без изменения поведения (`plan/100-refactor-250.md`).

use crate::icon_theme::IconTheme;
use std::path::PathBuf;
use std::sync::{LazyLock, Mutex};

/// Активная contributed icon-тема (None = builtin Catppuccin).
static ACTIVE: LazyLock<Mutex<Option<IconTheme>>> = LazyLock::new(|| Mutex::new(None));
pub fn set_active(theme: Option<IconTheme>) {
    *ACTIVE.lock().unwrap() = theme;
    // Иконки живут в статике: кэшируемые панели узнают о смене только по
    // поколению оформления (`plan/102-components.md`).
    kamin_theme::bump_generation();
}
/// Ассет или диск → `Img` через СИНХРОННЫЙ пре-декод (`icon_raster`): готовая
/// текстура вместо асинхронной цепочки image_cache (пустые квадраты/краш).
/// Светлая тема — фильтр оригинала `saturate(3.2) brightness(0.7)`
/// (`TreeIcon.module.css:6`) применяется К ПИКСЕЛЯМ растра внутри
/// icon_raster: переписывание hex в SVG-исходнике ломало `url(#id)`-ссылки
/// и дефы — иконки светлой темы разваливались (скрин юзера). Ключ кэша
/// включает вариант темы.
fn img_of(path: PathBuf) -> gpui::Img {
    let light = kamin_theme::current_is_light();
    // Пре-декод умеет только SVG; растровые иконки contributed-тем (png)
    // остаются на старом пути.
    if path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("svg"))
    {
        let key = format!("{}#{}", path.to_string_lossy(), u8::from(light));
        if let Some(im) = crate::icon_raster::raster_svg(&key, light, || std::fs::read(&path).ok())
        {
            return gpui::img(im);
        }
    }
    gpui::img(path)
}
fn asset_img(asset: &'static str) -> gpui::Img {
    let light = kamin_theme::current_is_light();
    let key = format!("{asset}#{}", u8::from(light));
    if let Some(im) = crate::icon_raster::raster_svg(&key, light, || {
        crate::ui::icons::CAT_ICONS
            .iter()
            .find(|(p, _)| *p == asset)
            .map(|(_, b)| b.to_vec())
    }) {
        return gpui::img(im);
    }
    gpui::img(gpui::SharedString::from(asset))
}
/// Иконка файла: contributed-тема или Catppuccin-ассет.
pub fn file_img(name: &str) -> gpui::Img {
    let hit = ACTIVE
        .lock()
        .unwrap()
        .as_ref()
        .and_then(|t| t.resolve_file(name, kamin_theme::current_is_light()));
    match hit {
        Some(p) => img_of(p),
        None => asset_img(crate::ui::icons::file_icon(name)),
    }
}
/// Иконка папки: contributed-тема или Catppuccin-ассет. `is_root` — корневая
/// папка воркспейса (карты `rootFolder*`).
pub fn folder_img(name: &str, open: bool, is_root: bool) -> gpui::Img {
    let hit = ACTIVE
        .lock()
        .unwrap()
        .as_ref()
        .and_then(|t| t.resolve_folder(name, open, is_root, kamin_theme::current_is_light()));
    match hit {
        Some(p) => img_of(p),
        None => asset_img(crate::ui::icons::folder_icon(name, open)),
    }
}
