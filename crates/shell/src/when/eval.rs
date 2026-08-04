//! Вычисление разобранного выражения when.
//!
//! Перенесено без изменения поведения (`plan/100-refactor-250.md`).

use crate::when::lexer::tokenize;
use crate::when::values::{coerce_str, loose_eq, member_of};
use crate::when::{ContextValues, Node, Parser};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

fn as_num(v: &Value) -> f64 {
    match v {
        Value::Number(n) => n.as_f64().unwrap_or(f64::NAN),
        Value::String(s) => s.parse().unwrap_or(f64::NAN),
        Value::Bool(true) => 1.0,
        Value::Bool(false) | Value::Null => 0.0,
        _ => f64::NAN,
    }
}
fn to_bool(v: &Value) -> bool {
    !matches!(v, Value::Null | Value::Bool(false))
        && *v != Value::String(String::new())
        && *v != Value::String("false".into())
        && *v != Value::String("0".into())
        && as_num_bool(v)
}
fn as_num_bool(v: &Value) -> bool {
    // 0 (числом) — falsy, как в JS toBool оригинала
    if let Value::Number(n) = v {
        n.as_f64() != Some(0.0)
    } else {
        true
    }
}
fn eval_node(n: &Node, ctx: &ContextValues) -> Value {
    match n {
        Node::Or(l, r) => Value::Bool(to_bool(&eval_node(l, ctx)) || to_bool(&eval_node(r, ctx))),
        Node::And(l, r) => Value::Bool(to_bool(&eval_node(l, ctx)) && to_bool(&eval_node(r, ctx))),
        Node::Not(e) => Value::Bool(!to_bool(&eval_node(e, ctx))),
        Node::Key(name) => ctx.get(name).cloned().unwrap_or(Value::Null),
        Node::Lit(v) => v.clone(),
        Node::Match(l, rx) => Value::Bool(rx.is_match(&coerce_str(&eval_node(l, ctx)))),
        Node::Cmp(op, l, r) => {
            let a = eval_node(l, ctx);
            let b = eval_node(r, ctx);
            Value::Bool(match op.as_str() {
                "==" => loose_eq(&a, &b),
                "!=" => !loose_eq(&a, &b),
                "<" => as_num(&a) < as_num(&b),
                "<=" => as_num(&a) <= as_num(&b),
                ">" => as_num(&a) > as_num(&b),
                ">=" => as_num(&a) >= as_num(&b),
                "in" => member_of(&a, &b),
                _ => false,
            })
        }
    }
}
/// Compile-кэш: when-строка парсится ОДИН раз (как VS Code — parse при
/// регистрации); None кэширует ошибку парсинга (не пере-логировать).
fn compile(expr: &str) -> Option<Node> {
    static CACHE: OnceLock<Mutex<HashMap<String, Option<Node>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(Mutex::default);
    if let Some(cached) = cache.lock().unwrap().get(expr) {
        return cached.clone();
    }
    let ast = tokenize(expr)
        .and_then(|toks| Parser { toks, i: 0 }.parse())
        .map_err(|e| eprintln!("when-clause: failed to parse \"{expr}\": {e}"))
        .ok();
    cache.lock().unwrap().insert(expr.to_string(), ast.clone());
    ast
}
/// Пустое выражение = true (нет условия); битое = false (fail-closed).
pub fn evaluate_when(expr: &str, ctx: &ContextValues) -> bool {
    if expr.trim().is_empty() {
        return true;
    }
    match compile(expr) {
        Some(ast) => to_bool(&eval_node(&ast, ctx)),
        None => false,
    }
}
