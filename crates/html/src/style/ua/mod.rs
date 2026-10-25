//! Умолчания user-agent: базовый и браузерный листы, quirks и метрики HTML-форм.

pub(crate) mod forms;

mod browser;
mod stylesheet;
pub use browser::BROWSER_CSS;
pub(crate) use stylesheet::{QUIRKS_CSS, user_agent_css};
