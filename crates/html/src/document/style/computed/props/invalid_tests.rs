//! Невалидные значения всех свойств реестра.

use crate::style::computed::Computed;

#[test]
fn invalid_value_preserves_computed_for_every_registered_property() {
    let st = crate::document::state::DocState::new(crate::style::css::Media::default());
    let mut seed = Computed::default();
    for prop in crate::coverage::PROPERTIES {
        seed.apply_one(prop.name, prop.sample, st.values());
    }
    let mut failures = Vec::new();
    for prop in crate::coverage::PROPERTIES {
        for invalid in [
            "invalid()",
            "invalid(0)",
            "0 invalid()",
            "invalid() 0",
            "initial invalid()",
        ] {
            if Computed::declaration_result(prop.name, invalid, st.values())
                == crate::style::computed::ParseResult::Accepted
            {
                failures.push(format!("@supports({}: {invalid})", prop.name));
            }
            for mut c in [Computed::default(), seed.clone()] {
                c.apply_one(prop.name, prop.sample, st.values());
                let before = format!("{c:?}");
                let seq = c.decl_order;
                c.apply_one(prop.name, invalid, st.values());
                // Счётчик входных деклараций сам по себе не является значением свойства.
                c.decl_order = seq;
                if before != format!("{c:?}") {
                    failures.push(format!("{}: {invalid}", prop.name));
                    break;
                }
            }
        }
    }
    assert!(
        failures.is_empty(),
        "invalid declaration changed Computed or was accepted: {failures:?}"
    );
}
