//! Нейтральные запросы и факты шрифтов; модуль не владеет загрузкой или CSS-резолвером.
//!
//! Запрос описывает существующий consumer корневых единиц. Полный face matching
//! требует отдельного контракта выбора лица: имя семейства не является face identity.
//! Provider не назначает CSS fallback отсутствующей метрике.

use std::rc::Rc;

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

/// Явная ревизия окружения. Старую ревизию backend обязан отвергнуть.
/// Это stamp доступа, а не копия ещё не перенесённого реестра дескрипторов.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FontRegistrySnapshot {
    pub environment: FontEnvironmentId,
    pub generation: u64,
}

/// Запрос существующих корневых метрик в CSS px.
/// `family_list` сохраняет порядок и синтаксис текущего CSS-списка без перепарсинга.
/// Размер передаётся без корректировки: разделение computed/adjusted — задача резолвера.
#[derive(Clone, Copy, Debug)]
pub struct FontRequest<'a> {
    pub family_list: &'a str,
    pub size_px: f32,
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
}

/// Backend вызывается с явной ревизией; style не загружает и не перечисляет шрифты.
/// Смена ревизии не превращает старый запрос в запрос к новому реестру.
pub trait FontFactsProvider {
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
