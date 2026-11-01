//! Стиль анонимной группы потока с краевыми фактами исходных коробок.

use crate::dom::SharedStyle;
use crate::style::computed::Computed;

pub fn group((before, after): (bool, bool)) -> SharedStyle {
    // CSS Break 3 §3.1: служебная группа сохраняет вынужденные разрывы
    // исходного пробега, но не добавляет прочие свойства его коробок.
    Computed {
        reset_fragmentation: crate::style::computed::StyleGroup::new(
            crate::style::computed::ResetFragmentation {
                break_before_force: before,
                break_after_force: after,
                ..Default::default()
            },
        ),
        ..Computed::default()
    }
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anonymous_group_constructs_only_its_boundary_facts() {
        for facts in [(false, false), (true, false), (false, true), (true, true)] {
            let style = group(facts);
            let mut expected = Computed::default();
            expected.reset_fragmentation.break_before_force = facts.0;
            expected.reset_fragmentation.break_after_force = facts.1;
            assert_eq!(**style.used(), expected);
        }
    }
}
