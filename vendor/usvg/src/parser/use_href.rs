//! KaminIDE patch: внедрённая загрузка внешних use без доступа парсера к файлам.
use std::sync::Arc;

type Resolver<'a> = dyn Fn(&str) -> Option<Arc<Vec<u8>>> + Send + Sync + 'a;

/// Resolves an absolute external use URL without its fragment.
///
/// Returning `None` rejects the resource; no filesystem fallback is attempted.
pub struct UseHrefResolver<'a>(pub Box<Resolver<'a>>);

impl std::fmt::Debug for UseHrefResolver<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("UseHrefResolver(..)")
    }
}
