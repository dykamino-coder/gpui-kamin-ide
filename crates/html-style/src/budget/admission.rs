//! Общий fallible контракт ранних проходов; policy принадлежит ревизии.

mod buffer;
mod text;

pub use buffer::Buffer;
pub use text::Text;

/// Отказ операции отличается от невалидного синтаксиса входа.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Budget(super::BudgetError),
    Allocation,
    CapacityOverflow,
}

/// Отсутствие ledger используется внутренними consumers без состояния документа.
#[derive(Clone, Default)]
pub struct Admission(Option<super::RevisionBudget>);

impl Admission {
    pub fn consume(&self, kind: super::BudgetKind, amount: usize) -> Result<(), Error> {
        if let Some(revision) = &self.0 {
            revision
                .consume(
                    kind,
                    u64::try_from(amount).map_err(|_| Error::CapacityOverflow)?,
                )
                .map_err(Error::Budget)?;
        }
        Ok(())
    }

    pub fn maximum(&self, kind: super::BudgetKind, amount: usize) -> Result<(), Error> {
        if let Some(revision) = &self.0 {
            revision
                .check_maximum(
                    kind,
                    u64::try_from(amount).map_err(|_| Error::CapacityOverflow)?,
                )
                .map_err(Error::Budget)?;
        }
        Ok(())
    }

    pub fn new(revision: super::RevisionBudget) -> Self {
        Self(Some(revision))
    }

    pub fn work(&self, amount: usize) -> Result<(), Error> {
        if let Some(revision) = &self.0 {
            let amount = u64::try_from(amount).map_err(|_| Error::CapacityOverflow)?;
            revision
                .consume(super::BudgetKind::TraversalVisits, amount)
                .map_err(Error::Budget)?;
        }
        Ok(())
    }

    pub fn storage(&self, bytes: usize) -> Result<Option<super::LiveReservation>, Error> {
        self.0
            .as_ref()
            .map(|revision| {
                let bytes = u64::try_from(bytes).map_err(|_| Error::CapacityOverflow)?;
                revision
                    .document()
                    .reserve(super::BudgetKind::LiveBytes, bytes)
                    .map_err(Error::Budget)
            })
            .transpose()
    }
}
