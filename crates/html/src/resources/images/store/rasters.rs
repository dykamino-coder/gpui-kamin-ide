//! Кэш SVG документа: параметры, шрифты и ревизии всех запрошенных ресурсов.

use super::ImageStore;
use crate::cache::{BoundedCache, MIB};
use crate::resources::images::{
    pixels::optional_bytes,
    svg_raster::{self, RasterRequest},
    types::CpuRaster,
};
use crate::resources::loader::{ResourceUrl, Revision};
use resvg::usvg::fontdb::Database;
use std::sync::{Arc, Mutex};
#[cfg(test)]
mod tests;
#[cfg(test)]
mod use_tests;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct ExternalResource {
    href: String,
    revision: Revision,
    bytes: Option<Arc<Vec<u8>>>,
}
type SourceKey = (String, RasterRequest);
type RasterKey = (String, RasterRequest, Vec<ExternalResource>);
type BytesKey = (ResourceUrl, Revision);

pub(super) struct Rasters {
    fonts: Arc<Database>,
    cache: BoundedCache<RasterKey, Option<Arc<CpuRaster>>>,
    external: BoundedCache<BytesKey, Option<Arc<Vec<u8>>>>,
    dependencies: BoundedCache<SourceKey, Vec<String>>,
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
            dependencies: BoundedCache::new(4 * MIB, |urls: &Vec<String>| {
                urls.iter().map(|url| url.len() + size_of::<String>()).sum()
            })
            .with_key_weight(|k: &SourceKey| k.0.len()),
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
    fn svg_resource(&self, href: &str, request: RasterRequest) -> ExternalResource {
        // Контракт markup API не наследует базу документа. Корневой SVG
        // привязывается через with_base; usvg сохраняет базы вложенных SVG.
        let (url, _) = ResourceUrl::resolve(href, None);
        let allowed =
            self.policy.allows(&url) && (request.external_resources || url.scheme() == "data");
        let revision = if allowed {
            self.services.loader.revision(&url)
        } else {
            Revision::Absent
        };
        let bytes = allowed
            .then(|| {
                self.rasters
                    .external
                    .get_or_insert_with((url.clone(), revision.clone()), || {
                        self.services
                            .loader
                            .load(&url)
                            .ok()
                            .map(|loaded| Arc::new(loaded.bytes.to_vec()))
                    })
            })
            .flatten();
        ExternalResource {
            href: href.to_string(),
            revision,
            bytes,
        }
    }
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "База шрифтов пока не меняется после создания документа"
        )
    )]
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
        let source = (markup.to_string(), request);
        if let Some(hrefs) = self.rasters.dependencies.get(&source) {
            let resources = hrefs
                .iter()
                .map(|href| self.svg_resource(href, request))
                .collect();
            if let Some(hit) = self
                .rasters
                .cache
                .get(&(source.0.clone(), request, resources))
            {
                return hit;
            }
        }
        // Только парсер знает фактически запрошенные вложенные use/image.
        // Сохраняем и отказы: появившийся файл должен инвалидировать fallback.
        let resources = Mutex::new(Vec::<ExternalResource>::new());
        let load = |href: &str| {
            let mut resources = resources.lock().ok()?;
            if let Some(resource) = resources.iter().find(|r| r.href == href) {
                return resource.bytes.clone();
            }
            let resource = self.svg_resource(href, request);
            let bytes = resource.bytes.clone();
            resources.push(resource);
            bytes
        };
        let raster = svg_raster::rasterize(
            markup,
            request,
            self.rasters.fonts.clone(),
            &load,
            self.policy.data,
        );
        let resources = resources.into_inner().ok()?;
        self.rasters.dependencies.insert(
            source.clone(),
            resources.iter().map(|r| r.href.clone()).collect(),
        );
        self.rasters
            .cache
            .insert((source.0, request, resources), raster.clone());
        raster
    }
}
