//! Кэш SVG документа: полная разметка, параметры, шрифты и ревизии внешних картинок.

use super::ImageStore;
use crate::cache::{BoundedCache, MIB};
use crate::resources::images::{
    pixels::optional_bytes,
    svg_links,
    svg_raster::{self, ExternalImage, RasterRequest},
    types::CpuRaster,
};
use crate::resources::loader::{ResourceUrl, Revision};
use resvg::usvg::fontdb::Database;
use std::sync::Arc;
#[cfg(test)]
mod tests;

type RasterKey = (String, RasterRequest, Vec<ExternalImage>);
type BytesKey = (ResourceUrl, Revision);

pub(super) struct Rasters {
    fonts: Arc<Database>,
    cache: BoundedCache<RasterKey, Option<Arc<CpuRaster>>>,
    external: BoundedCache<BytesKey, Option<Arc<Vec<u8>>>>,
}
impl Default for Rasters {
    fn default() -> Self {
        // Прежний markup API usvg использовал пустую базу. Системные шрифты
        // подключаются только явным решением владельца документа.
        Self {
            fonts: Arc::new(Database::new()),
            cache: BoundedCache::new(128 * MIB, optional_bytes).with_key_weight(|k: &RasterKey| {
                k.0.len()
                    + k.2
                        .iter()
                        .map(|d| d.href.len() + d.bytes.as_ref().map_or(0, |b| b.len()))
                        .sum::<usize>()
            }),
            external: BoundedCache::new(32 * MIB, |b: &Option<Arc<Vec<u8>>>| {
                b.as_ref().map_or(0, |b| b.len())
            })
            .with_key_weight(|k: &BytesKey| k.0.as_str().len()),
        }
    }
}
impl Rasters {
    pub(super) fn clear(&mut self) {
        *self = Self {
            fonts: self.fonts.clone(),
            ..Self::default()
        };
    }
}
impl ImageStore {
    pub(crate) fn set_svg_fonts(&mut self, fonts: Arc<Database>) {
        if !Arc::ptr_eq(&self.rasters.fonts, &fonts) {
            self.rasters = Rasters {
                fonts,
                ..Rasters::default()
            };
            self.revision += 1;
        }
    }
    pub(crate) fn rasterize_svg(
        &self,
        markup: &str,
        request: RasterRequest,
    ) -> Option<Arc<CpuRaster>> {
        let images = svg_links::image_links(markup)
            .into_iter()
            .filter(|(href, _)| !href.starts_with('#') && !href.starts_with("data:"))
            .map(|(href, _)| {
                let (url, _) = self.resolve(&href);
                let allowed = self.policy.allows(&url);
                let revision = if allowed {
                    self.services.loader.revision(&url)
                } else {
                    Revision::Absent
                };
                let bytes = allowed
                    .then(|| {
                        self.rasters.external.get_or_insert_with(
                            (url.clone(), revision.clone()),
                            || {
                                self.services
                                    .loader
                                    .load(&url)
                                    .ok()
                                    .map(|loaded| Arc::new(loaded.bytes.to_vec()))
                            },
                        )
                    })
                    .flatten();
                ExternalImage {
                    href,
                    revision,
                    bytes,
                }
            })
            .collect();
        let key = (markup.to_string(), request, images);
        self.rasters.cache.get_or_insert_with(key.clone(), || {
            svg_raster::rasterize(
                markup,
                request,
                self.rasters.fonts.clone(),
                &key.2,
                self.policy.data,
            )
        })
    }
}
