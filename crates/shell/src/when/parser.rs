//! Парсер выражений when: приоритеты операций.
//!
//! Перенесено без изменения поведения (`plan/100-refactor-250.md`).

use crate::when::Node;
use crate::when::lexer::Tok;
use crate::when::values::{compile_regex, num};
use serde_json::Value;

const COMPARE_OPS: [&str; 6] = ["==", "!=", "<", "<=", ">", ">="];
pub(crate) struct Parser {
    pub(crate) toks: Vec<Tok>,
    pub(crate) i: usize,
}
impl Parser {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.i)
    }
    fn is_op(&self, v: &str) -> bool {
        matches!(self.peek(), Some(Tok::Op(o)) if o == v)
    }
    fn is_ident(&self, v: &str) -> bool {
        matches!(self.peek(), Some(Tok::Ident(o)) if o == v)
    }

    pub(crate) fn parse(mut self) -> Result<Node, String> {
        let n = self.or()?;
        if self.peek().is_some() {
            return Err("trailing tokens".into());
        }
        Ok(n)
    }
    fn or(&mut self) -> Result<Node, String> {
        let mut l = self.and()?;
        while self.is_op("||") {
            self.i += 1;
            l = Node::Or(Box::new(l), Box::new(self.and()?));
        }
        Ok(l)
    }
    fn and(&mut self) -> Result<Node, String> {
        let mut l = self.comparison()?;
        while self.is_op("&&") {
            self.i += 1;
            l = Node::And(Box::new(l), Box::new(self.comparison()?));
        }
        Ok(l)
    }
    // `!` крепче сравнений (BNF VS Code: || < && < ! < cmp):
    // `!key == v` = `(!key) == v`
    fn unary(&mut self) -> Result<Node, String> {
        if self.is_op("!") {
            self.i += 1;
            return Ok(Node::Not(Box::new(self.unary()?)));
        }
        self.primary()
    }
    fn comparison(&mut self) -> Result<Node, String> {
        let l = self.unary()?;
        if self.is_op("=~") {
            self.i += 1;
            let Some(Tok::Regex(pat, flags)) = self.peek().cloned() else {
                return Err("=~ expects a /regex/".into());
            };
            self.i += 1;
            let rx = compile_regex(&pat, &flags)?;
            return Ok(Node::Match(Box::new(l), rx));
        }
        if let Some(Tok::Op(op)) = self.peek()
            && COMPARE_OPS.contains(&op.as_str())
        {
            let op = op.clone();
            self.i += 1;
            let r = self.value()?;
            return Ok(Node::Cmp(op, Box::new(l), Box::new(r)));
        }
        if self.is_ident("in") {
            self.i += 1;
            let r = self.primary()?;
            return Ok(Node::Cmp("in".into(), Box::new(l), Box::new(r)));
        }
        if self.is_ident("not") {
            self.i += 1;
            if !self.is_ident("in") {
                return Err("expected 'in' after 'not'".into());
            }
            self.i += 1;
            let r = self.primary()?;
            return Ok(Node::Not(Box::new(Node::Cmp(
                "in".into(),
                Box::new(l),
                Box::new(r),
            ))));
        }
        Ok(l)
    }
    fn primary(&mut self) -> Result<Node, String> {
        let t = self.toks.get(self.i).cloned().ok_or("unexpected end")?;
        self.i += 1;
        match t {
            Tok::Op(ref o) if o == "(" => {
                let e = self.or()?;
                if !self.is_op(")") {
                    return Err("expected ')'".into());
                }
                self.i += 1;
                Ok(e)
            }
            Tok::Ident(v) => Ok(match v.as_str() {
                "true" => Node::Lit(Value::Bool(true)),
                "false" => Node::Lit(Value::Bool(false)),
                _ => Node::Key(v),
            }),
            Tok::Str(v) => Ok(Node::Lit(Value::String(v))),
            Tok::Num(v) => Ok(Node::Lit(num(v))),
            _ => Err("unexpected token".into()),
        }
    }
    /// RHS сравнения — ЛИТЕРАЛ, не контекст-ключ: `editorLangId == typescript`
    /// сравнивает со строкой "typescript" (семантика VS Code).
    fn value(&mut self) -> Result<Node, String> {
        let t = self.toks.get(self.i).cloned().ok_or("unexpected end")?;
        self.i += 1;
        match t {
            Tok::Ident(v) => Ok(match v.as_str() {
                "true" => Node::Lit(Value::Bool(true)),
                "false" => Node::Lit(Value::Bool(false)),
                _ => Node::Lit(Value::String(v)),
            }),
            Tok::Str(v) => Ok(Node::Lit(Value::String(v))),
            Tok::Num(v) => Ok(Node::Lit(num(v))),
            _ => Err("unexpected token".into()),
        }
    }
}
