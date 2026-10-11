use std::sync::Arc;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The fallback fonts that can be configured for a given font.
/// Fallback fonts family names are stored here.
#[derive(Default, Clone, Eq, PartialEq, Hash, Debug, Deserialize, Serialize, JsonSchema)]
pub struct FontFallbacks(pub Arc<Vec<String>>);

impl FontFallbacks {
    /// Get the fallback fonts family names
    pub fn fallback_list(&self) -> &[String] {
        self.0.as_slice()
    }

    /// Create a font fallback from a list of strings
    pub fn from_fonts(fonts: Vec<String>) -> Self {
        FontFallbacks(Arc::new(fonts))
    }

    /// KaminIDE patch: a fallback entry that applies only to the given
    /// inclusive code point ranges (`Family@3040-30FF,31F0-31FF`). Platforms
    /// that understand it map just those ranges to the family; characters
    /// outside them keep the system fallback.
    pub fn restricted(family: &str, ranges: &[(u32, u32)]) -> String {
        let list = ranges
            .iter()
            .map(|(lo, hi)| format!("{lo:X}-{hi:X}"))
            .collect::<Vec<_>>()
            .join(",");
        format!("{family}@{list}")
    }

    /// Splits an entry made by [`FontFallbacks::restricted`] into the family
    /// name and its ranges; a plain family name has no ranges.
    pub fn split_restricted(entry: &str) -> (&str, Option<Vec<(u32, u32)>>) {
        let Some((family, list)) = entry.rsplit_once('@') else {
            return (entry, None);
        };
        let ranges: Option<Vec<(u32, u32)>> = list
            .split(',')
            .map(|r| {
                let (lo, hi) = r.split_once('-')?;
                Some((
                    u32::from_str_radix(lo, 16).ok()?,
                    u32::from_str_radix(hi, 16).ok()?,
                ))
            })
            .collect();
        match ranges {
            Some(ranges) if !ranges.is_empty() => (family, Some(ranges)),
            _ => (entry, None),
        }
    }
}
