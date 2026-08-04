//! AssetSource: вшитые SVG-иконки (Phosphor d-строки из tool-icon-paths.ts,
//! сгенерированы в assets/icons/*.svg). gpui svg() красит currentColor
//! через .text_color() — как <ToolIcon> в kamin-ide.

use std::borrow::Cow;

use anyhow::Result;
use gpui::{AssetSource, SharedString};

pub struct Assets;

macro_rules! icon {
    ($name:literal) => {
        (
            concat!("icons/", $name, ".svg"),
            include_bytes!(concat!("../assets/icons/", $name, ".svg")).as_slice(),
        )
    };
}

const ICONS: &[(&str, &[u8])] = &[
    icon!("folders"),
    icon!("tree-view"),
    icon!("search"),
    icon!("warning"),
    icon!("terminal"),
    icon!("gear"),
    icon!("panel-left"),
    icon!("kaminoid"),
    icon!("codicon-loading"),
    icon!("spinner-arc"),
    icon!("panel-slot-main"),
    icon!("panel-slot-main-bottom"),
    icon!("panel-slot-center"),
    icon!("panel-slot-center-bottom"),
    icon!("panel-slot-right"),
    icon!("panel-slot-right-top"),
    icon!("panel-slot-right-bottom"),
    icon!("panel-slot-bottom"),
    // Панель поиска редактора рисуется ВЕНДОРНЫМ `Input` и просит иконки по
    // путям `IconName` (`gpui-component/src/icon.rs`). Крейт их не поставляет
    // — без этих файлов кнопки «замена», «дальше/назад», «закрыть» и «регистр»
    // не рисуются вовсе: пустой бар, найдено юзером
    icon!("chevron-left"),
    icon!("chevron-right"),
    icon!("chevron-down"),
    icon!("close"),
    icon!("replace"),
    icon!("case-sensitive"),
];

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        Ok(ICONS
            .iter()
            .chain(crate::ui::icons::CAT_ICONS.iter())
            .find(|(p, _)| *p == path)
            .map(|(_, bytes)| Cow::Borrowed(*bytes)))
    }

    fn list(&self, prefix: &str) -> Result<Vec<SharedString>> {
        Ok(ICONS
            .iter()
            .chain(crate::ui::icons::CAT_ICONS.iter())
            .filter(|(p, _)| p.starts_with(prefix))
            .map(|(p, _)| SharedString::from(*p))
            .collect())
    }
}
