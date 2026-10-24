//! Политика доступа документа к ресурсам-картинкам: какие схемы адресов он вправе читать.
//! Решение принимает запрос документа ДО чтения байт; процессный кэш его не хранит и не
//! отменяет — общий растр не означает разрешения для другого документа (так проверяет
//! контекст запроса Gecko `imgLoader::ValidateEntry`).

use crate::resources::loader::ResourceUrl;

/// Разрешённые схемы. Сетевых схем нет: для них нет загрузчика.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct ResourcePolicy {
    pub(crate) data: bool,
    pub(crate) file: bool,
}

impl Default for ResourcePolicy {
    /// Как сейчас у движка: `data:` и локальные файлы.
    fn default() -> Self {
        Self {
            data: true,
            file: true,
        }
    }
}

impl ResourcePolicy {
    /// Только встроенные `data:`-ресурсы (документ без доступа к файлам).
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "Документы пока создаются с политикой по умолчанию"
        )
    )]
    pub(crate) fn data_only() -> Self {
        Self {
            data: true,
            file: false,
        }
    }

    /// Можно ли документу читать адрес. Неизвестная схема пропускается к загрузчику,
    /// чтобы ошибка называла настоящую причину (`UnsupportedScheme`).
    pub(crate) fn allows(&self, url: &ResourceUrl) -> bool {
        match url.scheme() {
            "data" => self.data,
            "file" => self.file,
            _ => true,
        }
    }
}
