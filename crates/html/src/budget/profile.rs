//! Неизменяемый versioned profile; host может только уменьшить квоты.

const MIB: u64 = 1 << 20;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BudgetUnit {
    Bytes,
    Nodes,
    Requests,
    Visits,
    Pixels,
    Tasks,
    Documents,
    Names,
    Operations,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BudgetClass {
    Structural,
    Monotonic,
    Live,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(usize)]
pub enum BudgetKind {
    SourceBytes,
    ResourceBytes,
    ResourceRequests,
    TraversalVisits,
    SubstitutionBytes,
    DomNodes,
    TextBytes,
    AttributeBytes,
    LiveBytes,
    DecodedPixels,
    PendingTasks,
    Documents,
    UniqueNames,
    NameBytes,
    CssBytes,
    CssTokens,
    CssRules,
    CssSelectors,
    CssComponents,
    CssDeclarations,
    CssDepth,
    UrlBytes,
}

impl BudgetKind {
    pub const COUNT: usize = Self::UrlBytes as usize + 1;
    pub const ALL: [Self; Self::COUNT] = [
        Self::SourceBytes,
        Self::ResourceBytes,
        Self::ResourceRequests,
        Self::TraversalVisits,
        Self::SubstitutionBytes,
        Self::DomNodes,
        Self::TextBytes,
        Self::AttributeBytes,
        Self::LiveBytes,
        Self::DecodedPixels,
        Self::PendingTasks,
        Self::Documents,
        Self::UniqueNames,
        Self::NameBytes,
        Self::CssBytes,
        Self::CssTokens,
        Self::CssRules,
        Self::CssSelectors,
        Self::CssComponents,
        Self::CssDeclarations,
        Self::CssDepth,
        Self::UrlBytes,
    ];

    pub fn unit(self) -> BudgetUnit {
        match self {
            Self::DomNodes => BudgetUnit::Nodes,
            Self::ResourceRequests => BudgetUnit::Requests,
            Self::TraversalVisits => BudgetUnit::Visits,
            Self::DecodedPixels => BudgetUnit::Pixels,
            Self::PendingTasks => BudgetUnit::Tasks,
            Self::Documents => BudgetUnit::Documents,
            Self::UniqueNames => BudgetUnit::Names,
            Self::CssTokens
            | Self::CssRules
            | Self::CssSelectors
            | Self::CssComponents
            | Self::CssDeclarations
            | Self::CssDepth => BudgetUnit::Operations,
            _ => BudgetUnit::Bytes,
        }
    }

    pub fn class(self) -> BudgetClass {
        match self {
            Self::DomNodes | Self::TextBytes | Self::AttributeBytes | Self::CssDepth => {
                BudgetClass::Structural
            }
            Self::LiveBytes | Self::DecodedPixels | Self::PendingTasks | Self::Documents => {
                BudgetClass::Live
            }
            _ => BudgetClass::Monotonic,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvalidLimit {
    pub kind: BudgetKind,
    pub requested: u64,
    pub maximum: u64,
}

impl std::fmt::Display for InvalidLimit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:?}: requested {}, maximum {}",
            self.kind, self.requested, self.maximum
        )
    }
}
impl std::error::Error for InvalidLimit {}

/// Profile `kaminide-document/4`; значения калиброваны corpus B2.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DocumentLimits {
    pub(super) caps: [u64; BudgetKind::COUNT],
}

impl Default for DocumentLimits {
    fn default() -> Self {
        Self {
            caps: [
                64 * MIB,
                512 * MIB,
                1 << 12,
                1 << 26,
                256 * MIB,
                1 << 22,
                32 * MIB,
                32 * MIB,
                2048 * MIB,
                1 << 29,
                1 << 22,
                1,
                1 << 10,
                1 << 14,
                4 * MIB,
                4 * MIB,
                1 << 15,
                1 << 16,
                32 * MIB,
                1 << 18,
                Self::CSS_NESTING as u64,
                128 * MIB,
            ],
        }
    }
}

impl DocumentLimits {
    pub const PROFILE_ID: &'static str = "kaminide-document";
    pub const VERSION: u32 = 4;
    pub const BOX_DEPTH: usize = 2052;
    pub const LAYOUT_DEPTH: usize = 1026;
    pub const CSS_NESTING: usize = 128;
    pub const RESOURCE_BYTES: u64 = 256 * MIB;
    pub const DATA_URL_BYTES: u64 = 3 * Self::RESOURCE_BYTES + 4096;
    pub const VAR_OUTPUT_BYTES: u64 = MIB;
    pub const VAR_WORK_BYTES: u64 = 32 * MIB;

    pub fn limit(&self, kind: BudgetKind) -> u64 {
        self.caps[kind as usize]
    }

    /// Нулевая квота запрещает расход; unbounded sentinel не поддерживается.
    pub fn lower(&self, kind: BudgetKind, limit: u64) -> Result<Self, InvalidLimit> {
        if limit > self.limit(kind) {
            return Err(InvalidLimit {
                kind,
                requested: limit,
                maximum: self.limit(kind),
            });
        }
        let mut result = self.clone();
        result.caps[kind as usize] = limit;
        Ok(result)
    }
}

/// Конечный бюджет экземпляра хоста, совместный для его документов и cache/pool.
/// Четыре document-sized рабочих набора; не глобальный изменяемый singleton.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProcessLimits {
    pub(super) caps: [u64; BudgetKind::COUNT],
}

impl Default for ProcessLimits {
    fn default() -> Self {
        let document = DocumentLimits::default();
        let mut caps = [0; BudgetKind::COUNT];
        for kind in BudgetKind::ALL {
            if kind.class() == BudgetClass::Live {
                caps[kind as usize] = document.limit(kind) * 4;
            }
        }
        Self { caps }
    }
}

impl ProcessLimits {
    pub fn limit(&self, kind: BudgetKind) -> u64 {
        self.caps[kind as usize]
    }
    pub fn lower(&self, kind: BudgetKind, limit: u64) -> Result<Self, InvalidLimit> {
        if kind.class() != BudgetClass::Live || limit > self.limit(kind) {
            return Err(InvalidLimit {
                kind,
                requested: limit,
                maximum: self.limit(kind),
            });
        }
        let mut result = self.clone();
        result.caps[kind as usize] = limit;
        Ok(result)
    }
}
