//! Параметры отрисовки приложения: контракт построения, который получает каждый бокс.
//!
//! `RenderOpts` несёт то, что задаёт хост (базовый текстовый стиль, viewport,
//! коэффициент `line-height: normal`), плюс контексты layout и фрагментации,
//! которые бокс передаёт боксам внутри себя. Точки входа render потребляют его;
//! домены layout, text и paint читают его, не завися от render.

use crate::defaults::MEDIUM_FONT_SIZE_PX;
use crate::style::computed::{BoxStyle, Computed};
use crate::style::values::value::Len;
use gpui::{TextStyle, px};

/// Настройки отрисовки: то, что задаёт приложение, а не документ.
#[derive(Clone)]
pub struct RenderOpts {
    /// Базовый стиль текста — от него считаются прогоны и наследование.
    pub text: TextStyle,
    /// Размер окна в точках — от него считаются `vh` и `vw`.
    pub viewport: (f32, f32),
    /// Множитель строки при `line-height: normal`.
    ///
    /// Браузер берёт его из метрик шрифта — у интерфейсных это около 1.31
    /// кегля. Умолчание GPUI — золотое сечение (1.618), и без своего значения
    /// КАЖДЫЙ блок текста выходил на четверть выше браузерного, а разница
    /// копилась вниз по документу.
    pub normal_line_height: f32,
    /// Контекст раскладки поддерева (`LayoutCx`): его задаёт коробка для
    /// коробок внутри. Хост передаёт `LayoutCx::default()`, а документ свой
    /// ставит `Document::render` (`Document::frame_opts`).
    pub cx: crate::layout::cx::LayoutCx,
    /// Контекст фрагментации сборки: его ставит код, строящий фрагменты
    /// (копии детей стопки колонок, вложенный многоколоночник рядами), а
    /// приложение передаёт `Default::default()`.
    pub frag: crate::layout::fragment::types::FragCx,
}

impl RenderOpts {
    /// Цвет подложки выделения.
    ///
    /// Отдельного поля в настройках нет, чтобы не ломать вызывающих: берём
    /// цвет текста и делаем из него полупрозрачную подложку — она читается
    /// и на светлой, и на тёмной теме.
    pub(crate) fn selection_color(&self) -> gpui::Hsla {
        let mut c = self.text.color;
        c.a = 0.25;
        c
    }

    pub(crate) fn base_size(&self) -> f32 {
        f32::from(self.text.font_size.to_pixels(px(MEDIUM_FONT_SIZE_PX)))
    }

    /// Корневой стиль документа.
    ///
    /// Высота строки тут НЕ задаётся: `normal` по CSS — метрика шрифта, и
    /// считает её `normal_fraction` по семейству элемента. Пока корень
    /// навязывал постоянную долю, она наследовалась ВСЕМ, и замер шрифта не
    /// работал ни разу: коробка с `line-height: normal` выходила выше коробки
    /// с `line-height: 1em` при одном и том же шрифте.
    pub(crate) fn root_style(&self) -> BoxStyle {
        BoxStyle::new(Computed {
            icb: true,
            ..Computed::default()
        })
    }
}

/// Свой кегль абзаца в точках — точка отсчёта для строки-опоры и для долей.
///
/// Отсчёт идёт от СВОЕГО кегля, а не от базового кегля документа: у коробки с
/// `font-size: 10px` строка обязана быть в 10 точек, а базовый (16) держал её
/// вдвое выше.
pub(crate) fn own_size(inherited: &Computed, opts: &RenderOpts) -> f32 {
    match inherited.font_size {
        Some(Len::Px(v)) => v,
        Some(Len::Em(k)) | Some(Len::Pct(k)) => k * opts.base_size(),
        _ => opts.base_size(),
    }
}
