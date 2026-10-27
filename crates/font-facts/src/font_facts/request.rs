//! Входы выбора лица без зависимости от вычисленного стиля или платформы.

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FontStyle {
    Normal,
    Italic,
    Oblique(f32),
}

/// Ориентация запроса; поворот строки не выдаётся за вертикальные метрики лица.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FontOrientation {
    #[default]
    Horizontal,
    VerticalMixed,
    VerticalUpright,
    Sideways,
}

/// Область запроса позволяет не запускать посторонние замеры и сохраняет порядок кэша.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FontMetricQuery {
    #[default]
    All,
    Horizontal,
    Vertical,
    Ideographic,
    Math,
    Alphabetic,
    /// Метрики выбранного лица до дескрипторов, без CSS-запасных значений.
    Adjustment,
}

/// Полный вход matching. Размер — computed CSS px, без size-adjust.
/// Список сохраняет авторский порядок и синтаксис; разбор CSS остаётся в style.
#[derive(Clone, Copy, Debug)]
pub struct FontRequest<'a> {
    pub family_list: &'a str,
    pub size_px: f32,
    pub weight: f32,
    pub width: f32,
    pub style: FontStyle,
    pub oblique_only: bool,
    pub monospace: bool,
    pub language: Option<&'a str>,
    pub script: Option<&'a str>,
    pub features: &'a [(String, u32)],
    pub feature_settings: &'a [(String, u32)],
    pub variations: &'a [(String, f32)],
    pub optical_size: Option<f32>,
    pub synthesis: (bool, bool, bool, bool),
    pub orientation: FontOrientation,
    pub metric_query: FontMetricQuery,
}

impl<'a> FontRequest<'a> {
    /// Начальные входы прежнего consumer метрик по одному семейству.
    pub fn new(family_list: &'a str, size_px: f32) -> Self {
        Self {
            family_list,
            size_px,
            weight: 400.0,
            width: 100.0,
            style: FontStyle::Normal,
            oblique_only: false,
            monospace: false,
            language: None,
            script: None,
            features: &[],
            feature_settings: &[],
            variations: &[],
            optical_size: None,
            synthesis: (true, true, true, true),
            orientation: FontOrientation::Horizontal,
            metric_query: FontMetricQuery::All,
        }
    }
}
