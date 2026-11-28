//! Font family fallbacks and raw metric normalization of the selected face.

use std::{
    hash::{Hash, Hasher},
    sync::Arc,
};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The fallback fonts that can be configured for a given font.
/// Fallback fonts family names are stored here.
#[derive(Default, Clone, Eq, PartialEq, Hash, Debug, Deserialize, Serialize, JsonSchema)]
pub struct FontFallbacks(Arc<FallbackConfig>);

// Unadjusted lists retain their existing JSON array representation.
#[derive(Clone, Eq, PartialEq, Hash, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(untagged)]
enum FallbackConfig {
    Names(Vec<String>),
    Adjusted {
        fonts: Vec<String>,
        adjustment: FontSizeAdjustment,
    },
}

impl Default for FallbackConfig {
    fn default() -> Self {
        Self::Names(Vec::new())
    }
}

/// A raw face metric from CSS Fonts 5 font-size-adjust.
#[derive(Clone, Copy, Eq, PartialEq, Hash, Debug, Deserialize, Serialize, JsonSchema)]
pub enum FontSizeMetric {
    XHeight,
    CapHeight,
    ZeroAdvance,
    IdeographicWidth,
    IdeographicHeight,
}

/// Target metric relative to the run em, already adjusted for the primary face.
/// Native fallback divides it by the raw metric of each selected face.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, JsonSchema)]
pub struct FontSizeAdjustment {
    pub metric: FontSizeMetric,
    pub aspect: f32,
}

impl PartialEq for FontSizeAdjustment {
    fn eq(&self, other: &Self) -> bool {
        self.metric == other.metric && self.aspect.to_bits() == other.aspect.to_bits()
    }
}
impl Eq for FontSizeAdjustment {}
impl Hash for FontSizeAdjustment {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.metric.hash(state);
        self.aspect.to_bits().hash(state);
    }
}

impl FontFallbacks {
    /// Get the fallback fonts family names
    pub fn fallback_list(&self) -> &[String] {
        match &*self.0 {
            FallbackConfig::Names(fonts) | FallbackConfig::Adjusted { fonts, .. } => fonts,
        }
    }

    /// Create a font fallback from a list of strings
    pub fn from_fonts(fonts: Vec<String>) -> Self {
        FontFallbacks(Arc::new(FallbackConfig::Names(fonts)))
    }

    /// Normalize each fallback face before shaping, including system fallback.
    pub fn with_size_adjustment(mut self, adjustment: FontSizeAdjustment) -> Self {
        let config = Arc::make_mut(&mut self.0);
        match config {
            FallbackConfig::Names(fonts) => {
                *config = FallbackConfig::Adjusted {
                    fonts: std::mem::take(fonts),
                    adjustment,
                };
            }
            FallbackConfig::Adjusted {
                adjustment: target, ..
            } => *target = adjustment,
        }
        self
    }

    /// Size normalization for native face selection.
    pub fn size_adjustment(&self) -> Option<FontSizeAdjustment> {
        match &*self.0 {
            FallbackConfig::Names(_) => None,
            FallbackConfig::Adjusted { adjustment, .. } => Some(*adjustment),
        }
    }

    /// Preserve the family list when a CSS run resets font-size-adjust.
    pub fn without_size_adjustment(mut self) -> Self {
        if self.size_adjustment().is_none() {
            return self;
        }
        let config = Arc::make_mut(&mut self.0);
        if let FallbackConfig::Adjusted { fonts, .. } = config {
            *config = FallbackConfig::Names(std::mem::take(fonts));
        }
        self
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
