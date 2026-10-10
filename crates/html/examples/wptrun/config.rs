//! Pair-list parsing and explicit animation snapshot configuration.

use std::time::Duration;

pub(super) fn pairs(list: &str) -> Vec<(String, String)> {
    std::fs::read_to_string(list)
        .unwrap_or_default()
        .lines()
        .filter_map(|line| line.split_once('|'))
        // Report rows may carry a verdict after the reference filename.
        .map(|(a, b)| {
            let b = b.split('|').next().unwrap_or(b).trim();
            (a.trim().to_string(), b.to_string())
        })
        .collect()
}

pub(super) fn animation_elapsed() -> Option<Duration> {
    let value = match std::env::var("WPT_ANIMATION_TIME_MS") {
        Ok(value) => value,
        Err(std::env::VarError::NotPresent) => return None,
        Err(error) => {
            eprintln!("Invalid WPT_ANIMATION_TIME_MS: {error}");
            std::process::exit(2);
        }
    };
    match value.parse::<u64>() {
        Ok(ms) => Some(Duration::from_millis(ms)),
        Err(error) => {
            eprintln!("WPT_ANIMATION_TIME_MS must be a nonnegative integer: {error}");
            std::process::exit(2);
        }
    }
}
