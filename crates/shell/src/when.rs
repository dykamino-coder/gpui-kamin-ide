//! `when`-clause движок — порт when-clause.ts 1:1 (B4 exthost).
//! Операторы реальных расширений: `!`, `&&`, `||`, `==`, `!=`, `=~`, `<`,
//! `<=`, `>`, `>=`, `in`/`not in`, скобки, bare-key truthiness, литералы
//! `true`/`false`/число/'строка'/`/regex/`. Неизвестные ключи = undefined
//! (falsy); битое выражение = false (fail-closed, как VS Code).

mod parser;

pub(crate) use parser::Parser;
mod eval;

pub use eval::evaluate_when;
mod lexer;
mod values;

use std::collections::HashMap;

use serde_json::Value;

pub type ContextValues = HashMap<String, Value>;

#[derive(Clone, Debug)]
pub(crate) enum Node {
    Or(Box<Node>, Box<Node>),
    And(Box<Node>, Box<Node>),
    Not(Box<Node>),
    Cmp(String, Box<Node>, Box<Node>),
    Match(Box<Node>, regex::Regex),
    Key(String),
    Lit(Value),
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn ctx() -> ContextValues {
        let mut m = ContextValues::new();
        m.insert("editorFocus".into(), json!(true));
        m.insert("sidebarVisible".into(), json!(false));
        m.insert("resourceLangId".into(), json!("typescript"));
        m.insert("view".into(), json!("explorer"));
        m.insert("gitOpenRepoCount".into(), json!(2));
        m.insert("listMultiSelection".into(), json!(false));
        m.insert("resourceExtname".into(), json!(".ts"));
        m.insert("tags".into(), json!(["a", "b"]));
        m
    }

    #[test]
    fn basics() {
        let c = ctx();
        assert!(evaluate_when("", &c));
        assert!(evaluate_when("   ", &c));
        assert!(evaluate_when("editorFocus", &c));
        assert!(!evaluate_when("sidebarVisible", &c));
        assert!(!evaluate_when("nope", &c));
        assert!(evaluate_when("!sidebarVisible", &c));
        assert!(!evaluate_when("!editorFocus", &c));
        assert!(evaluate_when("!!editorFocus", &c));
    }

    #[test]
    fn logical_precedence() {
        let c = ctx();
        assert!(evaluate_when(
            "sidebarVisible && editorFocus || editorFocus",
            &c
        ));
        assert!(!evaluate_when("editorFocus && sidebarVisible", &c));
        assert!(evaluate_when("editorFocus && !sidebarVisible", &c));
        assert!(evaluate_when(
            "editorFocus && (sidebarVisible || editorFocus)",
            &c
        ));
        assert!(!evaluate_when(
            "(editorFocus || sidebarVisible) && sidebarVisible",
            &c
        ));
    }

    #[test]
    fn comparisons() {
        let c = ctx();
        assert!(evaluate_when("resourceLangId == typescript", &c));
        assert!(evaluate_when("resourceLangId == 'typescript'", &c));
        assert!(evaluate_when("resourceLangId != python", &c));
        assert!(evaluate_when("editorFocus == true", &c));
        assert!(evaluate_when("gitOpenRepoCount == 2", &c));
        assert!(evaluate_when("gitOpenRepoCount > 1", &c));
        assert!(evaluate_when("gitOpenRepoCount >= 2", &c));
        assert!(!evaluate_when("gitOpenRepoCount < 2", &c));
        assert!(evaluate_when("gitOpenRepoCount <= 2", &c));
    }

    #[test]
    fn regex_match() {
        let c = ctx();
        assert!(evaluate_when(r"resourceExtname =~ /\.(ts|js)$/", &c));
        assert!(evaluate_when("resourceLangId =~ /^type/", &c));
        assert!(!evaluate_when("resourceLangId =~ /^py/", &c));
        // флаги
        assert!(evaluate_when("resourceLangId =~ /^TYPE/i", &c));
        assert!(!evaluate_when("resourceLangId =~ /^TYPE/", &c));
    }

    #[test]
    fn in_not_in() {
        let c = ctx();
        assert!(evaluate_when("'a' in tags", &c));
        assert!(!evaluate_when("'z' in tags", &c));
        assert!(evaluate_when("'z' not in tags", &c));
        assert!(!evaluate_when("'a' in unknownKey", &c));
        assert!(evaluate_when("'a' not in unknownKey", &c));
        assert!(evaluate_when("resourceExtname in '.ts,.js'", &c));
        assert!(!evaluate_when("resourceExtname in '.py,.rb'", &c));
    }

    #[test]
    fn not_binds_tighter_than_cmp() {
        let c = ctx();
        assert!(!evaluate_when("!editorFocus == true", &c));
        assert!(evaluate_when("!sidebarVisible == true", &c));
    }

    #[test]
    fn fail_closed() {
        let c = ctx();
        assert!(!evaluate_when("resourceLangId == 'typ", &c));
        assert!(!evaluate_when("resourceLangId =~ /typ", &c));
        assert!(!evaluate_when("editorFocus &&", &c));
        assert!(!evaluate_when("( unbalanced", &c));
    }

    #[test]
    fn realistic_and_cache() {
        let c = ctx();
        assert!(evaluate_when("view == explorer && !listMultiSelection", &c));
        let expr = "editorFocus && resourceLangId == typescript";
        assert!(evaluate_when(expr, &c));
        let mut c2 = ctx();
        c2.insert("editorFocus".into(), json!(false));
        assert!(!evaluate_when(expr, &c2));
        let mut c3 = ctx();
        c3.insert("resourceLangId".into(), json!("python"));
        assert!(!evaluate_when(expr, &c3));
        assert!(evaluate_when(expr, &c));
    }
}
