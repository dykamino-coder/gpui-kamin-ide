//! Умолчания user-agent: базовый и браузерный листы, quirks и метрики HTML-форм.

pub mod forms;

mod browser;
mod stylesheet;
pub use browser::BROWSER_CSS;
pub use stylesheet::{QUIRKS_CSS, user_agent_css};
