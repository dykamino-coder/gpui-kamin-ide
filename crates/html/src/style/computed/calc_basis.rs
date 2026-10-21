//! Sizing keyword retained by calc-size until used-value resolution.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CalcSizeBasis {
    Auto,
    MinContent,
    MaxContent,
    FitContent,
    Stretch,
}

impl CalcSizeBasis {
    pub(super) fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "auto" | "content" => Self::Auto,
            "min-content" => Self::MinContent,
            "max-content" => Self::MaxContent,
            "fit-content" => Self::FitContent,
            "stretch" => Self::Stretch,
            _ => return None,
        })
    }
}
