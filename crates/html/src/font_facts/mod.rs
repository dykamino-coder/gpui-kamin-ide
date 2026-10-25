//! Нейтральные запросы и факты шрифтов; модуль не владеет загрузкой или CSS-резолвером.
//!
//! Снимок принадлежит документу, ранжирование дескрипторов не зависит от backend.
//! Provider не назначает CSS fallback отсутствующей метрике.

use std::rc::Rc;

/// Маркер CSS-подбора веса для адаптера платформы.
pub const CSS_WEIGHT_TAG: (&str, u32) = ("cssw", 1);

pub(crate) mod matching;
mod registry;
mod request;
pub(crate) use registry::{FeatureDefinitions, Registry};
pub use registry::{FontRegistrySnapshot, GENERIC_SANS, generic_family};
pub use request::{FontOrientation, FontRequest, FontStyle};

/// Identity окружения одного документа без процессного счётчика или реестра.
#[derive(Clone, Debug)]
pub struct FontEnvironmentId(Rc<()>);

impl Default for FontEnvironmentId {
    fn default() -> Self {
        Self(Rc::new(()))
    }
}

impl PartialEq for FontEnvironmentId {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for FontEnvironmentId {}

/// Нативная identity выбранного лица в конкретной ревизии документа.
/// Ключ выдаёт backend; строка семейства не может создать identity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FaceId {
    pub(crate) environment: FontEnvironmentId,
    pub(crate) generation: u64,
    pub(crate) backend_key: usize,
}

/// Факты нативного выбора; не утверждают наличие конкретного глифа.
#[derive(Clone, Debug, PartialEq)]
pub struct SelectedFaceFacts {
    pub face: FaceId,
    pub units_per_em: u32,
    pub size_adjust: f32,
    pub instance: FontInstanceFacts,
}

/// Backend пока не сообщает выбранные оси и фактический синтез.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FontInstanceFacts {
    Absent,
    Unsupported {
        variations: bool,
        optical_size: bool,
        script: bool,
    },
}

/// Достоверность факта не выводится из числового значения (ноль допустим).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MetricSource {
    /// Backend уже разрешил overrides/fallback; это не raw/design метрика лица.
    BackendResolved,
}

/// Эффективная метрика в CSS px; отсутствие передаётся через `Option`, не нулём.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FontMetric {
    pub px: f32,
    pub source: MetricSource,
}

impl FontMetric {
    pub fn backend_resolved(px: f32) -> Self {
        Self {
            px,
            source: MetricSource::BackendResolved,
        }
    }
}

/// Факты для `cap/ex/ch/ic` текущего корневого consumer.
/// Продвижения относятся к U+0030/U+6C34; backend пока не сообщает face identity
/// или ориентацию, поэтому эти факты нельзя выдавать за результат полного matching.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FontMetricFacts {
    /// Ascent нужен CSS-резолверу как fallback отсутствующего cap-height.
    pub ascent: FontMetric,
    pub cap_height: Option<FontMetric>,
    pub x_height: Option<FontMetric>,
    pub zero_advance: Option<FontMetric>,
    pub ideographic_advance: Option<FontMetric>,
}

/// Теоретическая `normal`-строка шрифта, не высота строки с её inline-потомками.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct NormalLineFacts {
    pub height: Option<FontMetric>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FontFactsError {
    WrongEnvironment,
    StaleSnapshot,
    Unavailable,
    UnsupportedRequest,
}

/// Backend вызывается с явной ревизией; style не загружает и не перечисляет шрифты.
/// Смена ревизии не превращает старый запрос в запрос к новому реестру.
pub trait FontFactsProvider {
    /// Нативный выбор для уже разрешённого CSS-семейства. Не glyph fallback.
    /// Backend без поддержки выбора сообщает отсутствие явно.
    fn select(
        &self,
        _request: FontRequest<'_>,
        _snapshot: &FontRegistrySnapshot,
    ) -> Result<SelectedFaceFacts, FontFactsError> {
        Err(FontFactsError::Unavailable)
    }

    fn metrics(
        &self,
        request: FontRequest<'_>,
        snapshot: &FontRegistrySnapshot,
    ) -> Result<FontMetricFacts, FontFactsError>;

    fn normal_line(
        &self,
        request: FontRequest<'_>,
        snapshot: &FontRegistrySnapshot,
    ) -> Result<NormalLineFacts, FontFactsError>;
}
