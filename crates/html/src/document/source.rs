//! Отложенный URL/path: пользовательская строка допускается до URL/IDNA.

use crate::budget::{
    BudgetKind,
    admission::{Admission, Error},
};
use std::path::Path;
use url::Url;

#[derive(Clone, Copy)]
pub(super) enum Source<'a> {
    Uri(&'a str),
    Path(&'a Path),
}

impl Source<'_> {
    pub(super) fn resolve(self, admission: &Admission) -> Result<Option<Url>, Error> {
        match self {
            Self::Uri(uri) => {
                admission.consume(BudgetKind::UrlBytes, uri.len())?;
                admission.work(uri.len())?;
                Ok(Url::parse(uri).ok())
            }
            Self::Path(path) => {
                admission.consume(BudgetKind::UrlBytes, path.as_os_str().len())?;
                admission.work(path.as_os_str().len())?;
                let Ok(path) = std::path::absolute(path) else {
                    return Ok(None);
                };
                admission.consume(BudgetKind::UrlBytes, path.as_os_str().len())?;
                admission.work(path.as_os_str().len())?;
                Ok(Url::from_file_path(path).ok())
            }
        }
    }
}

pub(super) fn join(base: &Url, value: &str, admission: &Admission) -> Result<Option<Url>, Error> {
    let bytes = value
        .len()
        .checked_add(base.as_str().len())
        .ok_or(Error::CapacityOverflow)?;
    admission.consume(BudgetKind::UrlBytes, bytes)?;
    admission.work(bytes)?;
    Ok(base.join(value).ok())
}

#[cfg(test)]
mod tests;
