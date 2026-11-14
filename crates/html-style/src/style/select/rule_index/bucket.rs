//! Корзина правил с кэшем отрицательного фильтра предков.

use super::*;

struct Candidates {
    rules: Vec<usize>,
    _lease: Option<crate::budget::LiveReservation>,
}

#[derive(Default)]
pub(super) struct Bucket {
    pub rules: Vec<usize>,
    cache: RefCell<HashMap<Bloom, Rc<Candidates>>>,
}

impl Bucket {
    pub fn clear(&self) {
        self.cache.borrow_mut().clear();
    }

    pub fn append(
        &self,
        bloom: Bloom,
        requirements: &[Bloom],
        out: &mut Buffer<usize>,
        admission: &Admission,
        bulk: Option<&super::super::bulk::Table>,
    ) -> Result<(), Error> {
        let cached = self.cache.borrow().get(&bloom).cloned();
        let candidates = if let Some(cached) = cached {
            cached
        } else {
            admission.work(
                self.rules
                    .len()
                    .checked_mul(2)
                    .ok_or(Error::CapacityOverflow)?,
            )?;
            let count = self
                .rules
                .iter()
                .filter(|&&i| {
                    !bulk.is_some_and(|b| b.handles(i)) && bloom.contains(requirements[i])
                })
                .count();
            let bytes = count
                .checked_mul(std::mem::size_of::<usize>())
                .and_then(|n| n.checked_add(192))
                .ok_or(Error::CapacityOverflow)?;
            let lease = admission.storage(bytes)?;
            let mut rules = Vec::new();
            rules
                .try_reserve_exact(count)
                .map_err(|_| Error::Allocation)?;
            for &i in &self.rules {
                if !bulk.is_some_and(|b| b.handles(i)) && bloom.contains(requirements[i]) {
                    rules.push(i);
                }
            }
            let candidates = Rc::new(Candidates {
                rules,
                _lease: lease,
            });
            self.cache
                .borrow_mut()
                .try_reserve(1)
                .map_err(|_| Error::Allocation)?;
            self.cache.borrow_mut().insert(bloom, candidates.clone());
            candidates
        };
        admission.work(candidates.rules.len())?;
        for &i in &candidates.rules {
            out.push(i, admission)?;
        }
        Ok(())
    }
}
