//! Неизменяемые факты CSS-картинок одного документа: подготовка выполняет задания,
//! а style читает только снимок. Неизвестный URL остаётся Pending, не ошибкой.

use crate::resources::images::{
    store::{ImageRequest, ImageState, ImageStore},
    types::{ColourPolicy, CpuRaster, DecodeOptions, DecodedImage, OrientationMode, PixelFormat},
};
use crate::style::computed::flat_background::{BackgroundImageValues, ImageAvailability};
use crate::style::computed::{GradSpace, ImageSource};
use crate::style::values::value::Color;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

#[derive(Clone, Copy)]
struct Fact {
    availability: ImageAvailability,
    uniform: Option<Color>,
}

/// Авторские URL принадлежат этому документу и разрешаются заново после смены базы.
/// Процессный кэш декодера по-прежнему использует содержимое, а не эти ключи.
#[derive(Default)]
pub(crate) struct BackgroundImages {
    revision: u64,
    entries: HashMap<String, [Fact; 2]>,
}

impl BackgroundImages {
    /// Синхронный барьер перед проходом стиля; ранее известные URL также обновляются.
    pub(crate) fn prepare(
        previous: &Rc<Self>,
        store: &mut ImageStore,
        urls: impl IntoIterator<Item = String>,
    ) -> Rc<Self> {
        let mut urls: HashSet<_> = urls.into_iter().collect();
        urls.extend(previous.entries.keys().cloned());
        let requests: Vec<_> = urls
            .into_iter()
            .map(|raw| {
                let (url, _) = store.resolve(&raw);
                let ids = [OrientationMode::None, OrientationMode::FromImage].map(|orientation| {
                    store.request(ImageRequest {
                        url: url.clone(),
                        options: DecodeOptions {
                            orientation,
                            colour: ColourPolicy::Preserve,
                        },
                    })
                });
                (raw, ids)
            })
            .collect();
        store.complete_pending();
        let view = store.snapshot();
        if previous.revision == view.revision() && previous.entries.len() == requests.len() {
            return previous.clone();
        }
        let entries = requests
            .into_iter()
            .map(|(raw, ids)| {
                let facts = ids.map(|id| match view.state(id) {
                    Some(ImageState::Ready { image, .. }) => Fact {
                        availability: ImageAvailability::Ready,
                        uniform: match image {
                            DecodedImage::Raster(r) => uniform(&super::presentation::raster(r)),
                            DecodedImage::Vector(_) => None,
                        },
                    },
                    Some(ImageState::Failed(_)) => Fact {
                        availability: ImageAvailability::Failed,
                        uniform: None,
                    },
                    _ => Fact {
                        availability: ImageAvailability::Pending,
                        uniform: None,
                    },
                });
                (raw, facts)
            })
            .collect();
        Rc::new(Self {
            revision: view.revision(),
            entries,
        })
    }

    fn fact(&self, raw: &str, orient: bool) -> Fact {
        self.entries
            .get(raw)
            .map(|facts| facts[usize::from(orient)])
            .unwrap_or(Fact {
                availability: ImageAvailability::Pending,
                uniform: None,
            })
    }
}

impl BackgroundImageValues for Rc<BackgroundImages> {
    fn image_available(&self, image: &ImageSource, orient: bool) -> ImageAvailability {
        match image {
            ImageSource::Url(raw) => self.fact(raw, orient).availability,
            ImageSource::Fallback { url, color } => match self.fact(url, orient).availability {
                ImageAvailability::Failed if color.is_some() => ImageAvailability::Ready,
                state => state,
            },
            ImageSource::Filter { image, .. } => self.image_available(image, orient),
            ImageSource::Invalid => ImageAvailability::Failed,
            _ => ImageAvailability::Ready,
        }
    }

    fn uniform_image_color(&self, image: &ImageSource) -> Option<Color> {
        match image {
            ImageSource::Url(raw) | ImageSource::Fallback { url: raw, .. } => {
                self.fact(raw, true).uniform
            }
            ImageSource::Gradient { value: g, .. } => {
                let c = g.from;
                // Длинная дуга hue может менять даже два одинаковых полярных цвета.
                let constant = g.hue != 1
                    || matches!(
                        g.space,
                        GradSpace::Srgb | GradSpace::Linear | GradSpace::Oklab | GradSpace::Lab
                    );
                (constant
                    && c.a == 1.0
                    && g.stops.iter().all(|s| s.0 == c)
                    && !crate::style::values::color_space::out_of_gamut(c))
                .then_some(c)
            }
            _ => None,
        }
    }
}

fn uniform(raster: &CpuRaster) -> Option<Color> {
    let first = raster.pixel(0, 0)?;
    if first[3] != 255
        || !(0..raster.height).all(|y| (0..raster.width).all(|x| raster.pixel(x, y) == Some(first)))
    {
        return None;
    }
    let [r, g, b, _] = match raster.format {
        PixelFormat::Rgba8 => first,
        PixelFormat::Bgra8 => [first[2], first[1], first[0], first[3]],
    };
    Some(Color {
        r: r as f32 / 255.0,
        g: g as f32 / 255.0,
        b: b as f32 / 255.0,
        a: 1.0,
    })
}

#[cfg(test)]
mod tests;
