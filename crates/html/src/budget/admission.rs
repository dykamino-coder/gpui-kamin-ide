//! Общий fallible контракт ранних проходов; policy принадлежит ревизии.

mod buffer;
mod text;

pub(crate) use buffer::Buffer;
pub(crate) use text::Text;

/// Отказ операции отличается от невалидного синтаксиса входа.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Error {
    Budget(super::BudgetError),
    Allocation,
    CapacityOverflow,
}

/// Отсутствие ledger используется внутренними consumers без состояния документа.
#[derive(Clone, Default)]
pub(crate) struct Admission(Option<super::RevisionBudget>);

impl Admission {
    pub(crate) fn consume(&self, kind: super::BudgetKind, amount: usize) -> Result<(), Error> {
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

    pub(crate) fn maximum(&self, kind: super::BudgetKind, amount: usize) -> Result<(), Error> {
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

    pub(crate) fn new(revision: super::RevisionBudget) -> Self {
        Self(Some(revision))
    }

    pub(crate) fn work(&self, amount: usize) -> Result<(), Error> {
        if let Some(revision) = &self.0 {
            let amount = u64::try_from(amount).map_err(|_| Error::CapacityOverflow)?;
            revision
                .consume(super::BudgetKind::TraversalVisits, amount)
                .map_err(Error::Budget)?;
        }
        Ok(())
    }

    pub(crate) fn storage(&self, bytes: usize) -> Result<Option<super::LiveReservation>, Error> {
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
