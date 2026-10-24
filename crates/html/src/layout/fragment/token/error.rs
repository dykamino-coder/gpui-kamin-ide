//! Ошибки исполнения продолжений: повреждённый курсор, отсутствие прогресса и ресурсы.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Error {
    InvalidToken,
    InvalidGeometry,
    NoProgress,
    ResourceExhausted,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "fragment continuation: {self:?}")
    }
}

impl std::error::Error for Error {}
