//! Кэш SVG документа: параметры, шрифты и ревизии всех запрошенных ресурсов.

use super::ImageStore;
use crate::cache::{BoundedCache, MIB};
use crate::resources::images::{
    pixels::optional_bytes,
    svg_raster::{self, RasterRequest},
    types::CpuRaster,
};
use crate::resources::loader::{ResourceUrl, Revision, fetch};
use resvg::usvg::fontdb::Database;
use std::sync::{Arc, Mutex};
mod identity;
use identity::Identity;
#[cfg(test)]
mod static_tests;
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
type SourceKey = (Identity, RasterRequest);
type RasterKey = (Identity, RasterRequest, Vec<ExternalResource>);
type BytesKey = (ResourceUrl, Revision);

pub(super) struct Rasters {
    fonts: Arc<Database>,
    identities: BoundedCache<String, Identity>,
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
            identities: BoundedCache::new(4 * MIB, |id: &Identity| id.markup.len())
                .with_key_weight(String::len),
            cache: BoundedCache::new(128 * MIB, optional_bytes).with_key_weight(|k: &RasterKey| {
                k.0.markup.len()
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
            .with_key_weight(|k: &SourceKey| k.0.markup.len()),
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
                        let loader = &*self.services.loader;
                        fetch(loader, &url, &self.budget, &self.cancel)
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
    pub fn set_svg_fonts(&mut self, fonts: Arc<Database>) {
        if !Arc::ptr_eq(&self.rasters.fonts, &fonts) {
            self.rasters = Rasters {
                fonts,
                ..Rasters::default()
            };
            self.revision += 1;
        }
    }
    /// Неизменяемая база шрифтов для синхронного запроса, без копии кэшей.
    pub fn svg_fonts(&self) -> Arc<Database> {
        self.rasters.fonts.clone()
    }
    /// Свежий запрос SVG-геометрии: не изменяет растровый кэш или зависимости.
    pub fn query_svg(
        &self,
        markup: &str,
        fonts: Arc<Database>,
        trace: impl resvg::usvg::GeometryTrace,
    ) -> Option<resvg::usvg::Tree> {
        let request = RasterRequest::new(1., 1., 1., 0.);
        svg_raster::parse_tree(
            markup,
            fonts,
            &|href| self.svg_resource(href, request).bytes,
            self.policy.data,
            trace,
        )
    }
    pub fn rasterize_svg(&self, markup: &str, request: RasterRequest) -> Option<Arc<CpuRaster>> {
        let identity = self.rasters.identities.get(markup).unwrap_or_else(|| {
            let identity = Identity::new(markup);
            self.rasters
                .identities
                .insert(markup.to_owned(), identity.clone());
            identity
        });
        let source = (identity, request);
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
                crate::work::add!(SvgRasterCacheHits, 1);
                return hit;
            }
        }
        crate::work::add!(SvgRasterCacheMisses, 1);
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
