//! Тесты счётчиков CSS (style::generated::counters): области видимости, сброс и приращение.

use super::*;

/// Область кончается там, где кончается родитель создателя.
#[test]
fn scope_ends_with_owners_parent() {
    let mut c = Counters::default();
    c.enter(); // корень
    c.enter(); // первый ребёнок — создатель
    c.reset("n", 0);
    c.update("n", 1, false);
    assert_eq!(c.value_of("n"), 1);
    c.leave();
    c.enter(); // следующий брат — внутри области
    c.update("n", 1, false);
    assert_eq!(c.value_of("n"), 2);
    c.leave();
    c.leave(); // вышли из родителя создателя
    c.enter();
    assert_eq!(c.value_of("n"), 0, "область кончилась вместе с родителем");
}

/// Вложенный сброс вкладывается, а сброс на своём уровне — заменяет.
#[test]
fn nesting_versus_replacement() {
    let mut c = Counters::default();
    c.enter();
    c.reset("n", 1);
    c.enter();
    c.reset("n", 7);
    assert_eq!(
        c.chain_of("n"),
        vec![1, 7],
        "вложенный счётчик рядом с внешним"
    );
    c.leave();
    c.enter();
    c.reset("n", 9);
    assert_eq!(c.chain_of("n"), vec![1, 9], "брат заменил запись брата");
}

#[test]
fn marker_reset_reaches_contents_but_not_next_item() {
    let mut c = Counters::default();
    c.enter();
    c.reset("n", 10);
    c.enter();
    c.enter_marker();
    c.reset("n", 1);
    c.leave();
    c.enter_pseudo(false);
    assert_eq!(c.chain_of("n"), vec![10, 1]);
    c.leave();
    c.leave();
    c.enter();
    assert_eq!(c.chain_of("n"), vec![10]);
}
