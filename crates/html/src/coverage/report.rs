//! CSS coverage report checks.

use super::*;

#[test]
fn print_coverage() {
    let total = PROPERTIES.len();
    let mapped = PROPERTIES
        .iter()
        .filter(|p| p.support == Support::Mapped)
        .count();
    let noop = PROPERTIES
        .iter()
        .filter(|p| matches!(p.support, Support::NoOp(_)))
        .count();
    let partial = PROPERTIES
        .iter()
        .filter(|p| matches!(p.support, Support::Partial(_)))
        .count();
    println!(
        "ПОКРЫТИЕ: всего {total}, полностью {mapped}, частично {partial}, пустышек {noop}, \
             невозможно {}, итого {:.1}%",
        total - mapped - noop - partial,
        mapped_pct()
    );
}
