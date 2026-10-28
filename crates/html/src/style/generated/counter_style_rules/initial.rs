//! Начальные системы счётчика и fallible порождение повторяющихся символов.

use super::*;
use crate::budget::admission::{Admission, Error};

/// Начальное представление по системе (§cyclic … §additive).
pub(super) fn initial(
    algo: &Algo,
    v: i128,
    context: Context,
    admission: &Admission,
) -> Result<Option<String>, Error> {
    let mut error = None;
    let value = (|| -> Option<String> {
        match algo {
            Algo::Builtin(name) => context.disclosure(name).map(str::to_owned).or_else(|| {
                let magnitude = if matches!(name.as_str(), "disc" | "circle" | "square") {
                    0 // Cyclic systems represent every integer, including negative values.
                } else {
                    u64::try_from(v).ok()?
                };
                builtin_initial(name, magnitude)
            }),
            Algo::Own(sys, syms, add) => {
                let n = syms.len() as i128;
                match sys {
                    System::Cyclic => {
                        Some(syms[(v.rem_euclid(n) + n - 1).rem_euclid(n) as usize].clone())
                    }
                    System::Fixed(first) => {
                        let i = v.checked_sub(i128::from(*first))?;
                        (0..n).contains(&i).then(|| syms[i as usize].clone())
                    }
                    System::Symbolic => {
                        if v < 1 {
                            return None;
                        }
                        let reps = ((v - 1) / n + 1) as usize;
                        Some(
                            repeat(&syms[((v - 1) % n) as usize], reps, admission)
                                .map_err(|e| error = Some(e))
                                .ok()?,
                        )
                    }
                    System::Alphabetic => {
                        if v < 1 {
                            return None;
                        }
                        let mut v = v;
                        let mut out = vec![];
                        while v > 0 {
                            v -= 1;
                            out.push(syms[(v % n) as usize].as_str());
                            v /= n;
                        }
                        out.reverse();
                        Some(out.concat())
                    }
                    System::Numeric => {
                        let mut v = v;
                        if v == 0 {
                            return Some(syms[0].clone());
                        }
                        let mut out = vec![];
                        while v > 0 {
                            out.push(syms[(v % n) as usize].as_str());
                            v /= n;
                        }
                        out.reverse();
                        Some(out.concat())
                    }
                    System::Additive => {
                        let mut v = v as u64;
                        if v == 0 {
                            return add.iter().find(|(w, _)| *w == 0).map(|(_, s)| s.clone());
                        }
                        let mut out = String::new();
                        for (w, s) in add {
                            if *w == 0 || v < *w {
                                continue;
                            }
                            let reps = v / w;
                            out.push_str(
                                &repeat(s, reps as usize, admission)
                                    .map_err(|e| error = Some(e))
                                    .ok()?,
                            );
                            v -= reps * w;
                            if v == 0 {
                                break;
                            }
                        }
                        (v == 0).then_some(out)
                    }
                    System::Extends(_) => None,
                }
            }
        }
    })();
    match error {
        Some(error) => Err(error),
        None => Ok(value),
    }
}

/// Размер и work проверяются до repeat; отказ не маскируется стилем fallback.
pub(super) fn repeat(value: &str, count: usize, admission: &Admission) -> Result<String, Error> {
    if value.is_empty() {
        return Ok(String::new());
    }
    let bytes = value
        .len()
        .checked_mul(count)
        .ok_or(Error::CapacityOverflow)?;
    admission.consume(crate::budget::BudgetKind::SubstitutionBytes, bytes)?;
    let _lease = admission.storage(bytes)?;
    let mut out = String::new();
    out.try_reserve_exact(bytes)
        .map_err(|_| Error::Allocation)?;
    for _ in 0..count {
        out.push_str(value);
    }
    Ok(out)
}
