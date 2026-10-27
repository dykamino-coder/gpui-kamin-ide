//! Загрузчик байт ресурса по разрешённому адресу: `ResourceUrl` (канонический адрес без
//! фрагмента), интерфейс `ResourceLoader` и его синхронный бэкенд `SyncLoader` — `data:`,
//! `file:` и локальный путь. Смысла картинки, шрифта или листа здесь нет: загрузчик отдаёт
//! байты и ревизию, по которой владелец запроса решает, устарел ли прежний результат.
//!
//! Сетевого бэкенда нет: неизвестная схема — явная ошибка `UnsupportedScheme`, а не тайный
//! HTTP через GPUI. Фрагмент адреса (`#view`, `#id`) — ключ представления, а не второй
//! запрос тех же байт, поэтому он отделяется при разрешении и в ключ загрузки не входит.

pub(crate) mod data_url;
mod ingress;
mod limits;
#[cfg(test)]
mod tests;

pub(crate) use ingress::{Cancellation, Ingress, diagnostic, fetch};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;
use url::Url;

/// Где лежат байты разрешённого адреса.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum Locator {
    /// Адрес `data:` целиком: содержимое — в самом адресе.
    Data,
    /// Файл: `file:` с процентным декодированием или локальный путь как есть.
    /// Исправление URI-семантики: <https://www.rfc-editor.org/rfc/rfc8089.html#section-4>.
    File(PathBuf),
    /// Схема, для которой бэкенда нет (`http:`, `blob:` …).
    Unsupported(String),
}

/// Разрешённый адрес ресурса без фрагмента — ключ загрузки.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct ResourceUrl {
    text: String,
    locator: Locator,
}

impl ResourceUrl {
    /// Разрешить авторский адрес относительно базы документа и отделить фрагмент.
    ///
    /// Абсолютный локальный путь (`C:\…`, `/…`) остаётся путём: `Url::parse` прочёл бы
    /// букву диска как схему. Адрес `data:` не делится по `#`: его тело может содержать
    /// знак как есть, и прежний загрузчик фона его не делил.
    pub(crate) fn resolve(authored: &str, base: Option<&Url>) -> (Self, Option<String>) {
        if authored.starts_with("data:") {
            return (Self::data(authored), None);
        }
        let (body, fragment) = match authored.split_once('#') {
            Some((body, fragment)) => (body, Some(fragment.to_string())),
            None => (authored, None),
        };
        if std::path::Path::new(body).is_absolute() {
            return (Self::path(body), fragment);
        }
        let joined = base
            .and_then(|base| base.join(body).ok())
            .or_else(|| Url::parse(body).ok());
        let url = match joined {
            Some(url) => Self::url(url),
            // Ни базы, ни схемы: путь относительно рабочего каталога процесса.
            None => Self::path(body),
        };
        (url, fragment)
    }

    fn data(text: &str) -> Self {
        Self {
            text: text.to_string(),
            locator: Locator::Data,
        }
    }

    fn path(path: &str) -> Self {
        Self {
            text: path.to_string(),
            locator: Locator::File(PathBuf::from(path)),
        }
    }

    fn url(mut url: Url) -> Self {
        url.set_fragment(None);
        let locator = match url.scheme() {
            "data" => return Self::data(url.as_str()),
            "file" => match url.to_file_path() {
                Ok(path) => Locator::File(path),
                Err(()) => Locator::Unsupported("file".to_string()),
            },
            scheme => Locator::Unsupported(scheme.to_string()),
        };
        Self {
            text: url.to_string(),
            locator,
        }
    }

    /// Канонический текст адреса (без фрагмента).
    pub(crate) fn as_str(&self) -> &str {
        &self.text
    }

    /// База ссылок внутри ресурса; локальный путь превращается в `file:`.
    pub(crate) fn base_url(&self) -> Option<Url> {
        match &self.locator {
            Locator::File(path) => {
                let absolute = if path.is_absolute() {
                    path.clone()
                } else {
                    std::env::current_dir().ok()?.join(path)
                };
                Url::from_file_path(absolute).ok()
            }
            _ => Url::parse(&self.text).ok(),
        }
    }

    /// Схема адреса для политики доступа: `data`, `file` или иная.
    pub(crate) fn scheme(&self) -> &str {
        match &self.locator {
            Locator::Data => "data",
            Locator::File(_) => "file",
            Locator::Unsupported(scheme) => scheme,
        }
    }
}

/// Версия содержимого по адресу: по ней видно, что файл появился, исчез или изменился.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Revision {
    /// Содержимое задано самим адресом (`data:`) и не меняется.
    Immutable,
    /// Файл: длина и время изменения на момент чтения.
    File {
        len: u64,
        modified: Option<SystemTime>,
    },
    /// По адресу ничего нет или прочитать нельзя.
    Absent,
}

/// Прочитанные байты и ревизия, с которой они прочитаны.
#[derive(Clone, Debug)]
pub(crate) struct Loaded {
    pub(crate) bytes: Arc<[u8]>,
    pub(crate) revision: Revision,
}

/// Почему байты не получены.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum LoadError {
    /// Бэкенда для схемы нет.
    UnsupportedScheme(String),
    /// Адрес `data:` без запятой или с битым `base64`.
    InvalidUrl,
    /// Файла нет.
    NotFound,
    /// Иная ошибка ввода-вывода.
    Io(std::io::ErrorKind),
    /// Входной ресурс превысил предел одного ресурса (S6) до декодирования содержимого.
    TooLarge { limit: usize },
    /// Ревизия отказала в запросе или байтах: сумма документа, а не один ресурс.
    Budget(crate::budget::BudgetError),
    /// Задание отменено вместе с устаревшим поколением документа.
    Cancelled,
}

/// Источник байт документа. Реализация не хранит документ и не знает о картинках.
pub(crate) trait ResourceLoader: Send + Sync {
    /// Прочитать байты по адресу; каждую порцию допустить через `ingress` до копирования.
    fn load(&self, url: &ResourceUrl, ingress: &Ingress<'_>) -> Result<Loaded, LoadError>;
    /// Текущая ревизия без чтения содержимого — для перепроверки прежних результатов.
    fn revision(&self, url: &ResourceUrl) -> Revision;
}

/// Синхронный бэкенд: `data:` из адреса, файлы с диска.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct SyncLoader;

impl ResourceLoader for SyncLoader {
    fn load(&self, url: &ResourceUrl, ingress: &Ingress<'_>) -> Result<Loaded, LoadError> {
        match &url.locator {
            Locator::Data => {
                let rest = url
                    .text
                    .strip_prefix("data:")
                    .ok_or(LoadError::InvalidUrl)?;
                let bytes = data_url::decode(rest, |count| ingress.admit(count))?;
                Ok(Loaded {
                    bytes: bytes.into(),
                    revision: Revision::Immutable,
                })
            }
            Locator::File(path) => {
                // Ревизия снимается ДО чтения: изменение между ними даст при следующей
                // перепроверке другую ревизию, а не молча закреплённые новые байты.
                let revision = file_revision(path);
                let file = std::fs::File::open(path).map_err(limits::io_error)?;
                if file.metadata().map_err(limits::io_error)?.len() > limits::RESOURCE_BYTES as u64
                {
                    return Err(LoadError::TooLarge {
                        limit: limits::RESOURCE_BYTES,
                    });
                }
                let bytes = limits::read(file, limits::RESOURCE_BYTES, |n| ingress.admit(n))?;
                Ok(Loaded {
                    bytes: bytes.into(),
                    revision,
                })
            }
            Locator::Unsupported(scheme) => Err(LoadError::UnsupportedScheme(scheme.clone())),
        }
    }

    fn revision(&self, url: &ResourceUrl) -> Revision {
        match &url.locator {
            Locator::Data => Revision::Immutable,
            Locator::File(path) => file_revision(path),
            Locator::Unsupported(_) => Revision::Absent,
        }
    }
}

fn file_revision(path: &std::path::Path) -> Revision {
    match std::fs::metadata(path) {
        Ok(meta) if meta.is_file() => Revision::File {
            len: meta.len(),
            modified: meta.modified().ok(),
        },
        _ => Revision::Absent,
    }
}
