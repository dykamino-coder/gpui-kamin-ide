//! Body of a style rule cut into declaration runs and nested blocks (css-nesting-1 §2).

/// Часть тела правила: серия объявлений или вложенный блок `(прелюдия, тело)`.
pub(super) enum Item<'a> {
    Decls(&'a str),
    Block(&'a str, &'a str),
}

/// Индекс закрывающей `}` для `{` на позиции `open` (или длина при обрыве).
fn matching_brace(b: &[u8], open: usize) -> usize {
    let mut depth = 0i32;
    let mut i = open;
    while i < b.len() {
        match b[i] {
            b'\\' => i += 1,
            q @ (b'"' | b'\'') => {
                i += 1;
                while i < b.len() && b[i] != q {
                    i += 1 + usize::from(b[i] == b'\\');
                }
            }
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return i;
                }
            }
            _ => {}
        }
        i += 1;
    }
    b.len()
}

/// Разрезать тело правила на серии объявлений и вложенные блоки.
/// Блок в значении пользовательского свойства (`--x: {a}`) — часть объявления.
pub(super) fn items(body: &str) -> Vec<Item<'_>> {
    let b = body.as_bytes();
    let mut out = vec![];
    let (mut run, mut stmt, mut depth, mut i) = (0usize, 0usize, 0i32, 0usize);
    while i < b.len() {
        match b[i] {
            b'\\' => i += 1,
            q @ (b'"' | b'\'') => {
                i += 1;
                while i < b.len() && b[i] != q {
                    i += 1 + usize::from(b[i] == b'\\');
                }
            }
            b'(' | b'[' => depth += 1,
            b')' | b']' => depth -= 1,
            b';' if depth <= 0 => stmt = i + 1,
            b'{' if depth <= 0 => {
                let close = matching_brace(b, i);
                let prelude = &body[stmt..i];
                if !(prelude.trim_start().starts_with("--") && prelude.contains(':')) {
                    if stmt > run {
                        out.push(Item::Decls(&body[run..stmt]));
                    }
                    out.push(Item::Block(prelude.trim(), &body[i + 1..close]));
                    run = (close + 1).min(b.len());
                    stmt = run;
                }
                i = close + 1;
                continue;
            }
            _ => {}
        }
        i += 1;
    }
    if run < b.len() {
        out.push(Item::Decls(&body[run..]));
    }
    out
}
