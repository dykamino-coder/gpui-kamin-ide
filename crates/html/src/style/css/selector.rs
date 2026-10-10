//! Селекторы: разбор, специфичность, псевдоклассы и псевдоэлементы.

use crate::style::css::*;

mod compound;
mod pseudo;
use pseudo::{is_pseudo_element, known_pseudo, pseudo_specificity};
pub(crate) use pseudo::{nth_of_parts, split_selector_list};

/// Простой селектор — ровно то подмножество, которое встречается на практике.
#[derive(Clone, Debug, PartialEq)]
pub struct Selector {
    pub tag: Option<String>,
    pub id: Option<String>,
    pub classes: Vec<String>,
    /// Атрибутные условия: `[href]`, `[type="text"]`, `[lang|=en]`.
    pub attrs: Vec<AttrSel>,
    /// Псевдокласс `:hover` и т.п. — применяется отдельным слоем.
    pub pseudo: Option<String>,
    /// Остальные псевдоклассы компаунда: `li:first-child:last-child` несёт
    /// в `pseudo` последний, прочие копятся здесь и обязаны выполниться все.
    pub also: Vec<String>,
    /// Предок для `.a .b` и `.a > .b`. Прямой ли — во втором поле.
    pub ancestor: Option<Box<(Selector, bool)>>,
    /// Предыдущий сосед для `.a + .b` и `.a ~ .b`. Смежный ли — во втором
    /// поле (`+` — ровно предыдущий, `~` — любой раньше).
    pub prev: Option<Box<(Selector, bool)>>,
    /// Явный универсальный селектор `*` в компаунде: безликий хост тени он не
    /// берёт (selectors-4 §featureless; `*:host` — `featureless-002`).
    pub universal: bool,
}

/// Атрибутный селектор одного условия.
#[derive(Clone, Debug, PartialEq)]
pub struct AttrSel {
    pub name: String,
    /// Операция и значение: `=` 0, `~=` 1, `|=` 2, `^=` 3, `$=` 4, `*=` 5.
    /// Отсутствует — проверяется само наличие атрибута.
    pub op: Option<(u8, String)>,
    /// Регистронезависимое сравнение значений (` i` перед `]`).
    pub ci: bool,
}

impl AttrSel {
    /// Совпадает ли значение атрибута (None = атрибута нет).
    pub fn matches(&self, value: Option<&str>) -> bool {
        let Some(value) = value else { return false };
        let Some((op, want)) = &self.op else {
            return true;
        };
        let (v, w);
        let (value, want): (&str, &str) = if self.ci {
            v = value.to_ascii_lowercase();
            w = want.to_ascii_lowercase();
            (&v, &w)
        } else {
            (value, want)
        };
        match op {
            0 => value == want,
            1 => !want.is_empty() && value.split_whitespace().any(|t| t == want),
            2 => {
                value == want
                    || (value.len() > want.len()
                        && value.starts_with(want)
                        && value.as_bytes()[want.len()] == b'-')
            }
            3 => !want.is_empty() && value.starts_with(want),
            4 => !want.is_empty() && value.ends_with(want),
            _ => !want.is_empty() && value.contains(want),
        }
    }
}

impl Selector {
    /// Специфичность как в CSS: (id, класс+псевдо, тег). Сравнивается лексикографически.
    pub fn specificity(&self) -> (u32, u32, u32) {
        let mut s = (
            self.id.is_some() as u32,
            self.classes.len() as u32 + self.attrs.len() as u32,
            self.tag.is_some() as u32,
        );
        // Псевдокласс — +1 к классам, а `of S` добавляет ещё специфичность
        // самого специфичного селектора списка (селекторы-4 §specificity):
        // `:nth-child(even of .foo, #bar)` весит как псевдо + id.
        for p in self.pseudo.iter().chain(self.also.iter()) {
            let m = pseudo_specificity(p);
            s = (s.0 + m.0, s.1 + m.1, s.2 + m.2);
        }
        if let Some(anc) = &self.ancestor {
            let a = anc.0.specificity();
            s = (s.0 + a.0, s.1 + a.1, s.2 + a.2);
        }
        if let Some(prev) = &self.prev {
            let a = prev.0.specificity();
            s = (s.0 + a.0, s.1 + a.1, s.2 + a.2);
        }
        s
    }

    /// `.card > .title`, `.card .title`, `div.card` — всё сюда.
    pub fn parse(raw: &str) -> Option<Selector> {
        // Разбивка на составные части и комбинаторы МЕЖДУ ними. Внутри скобок
        // `+` и `~` — не комбинаторы, а часть записи `2n+1` в `:nth-child()`.
        // Пробел — комбинатор потомка, если рядом нет знакового.
        let mut tokens: Vec<Result<String, u8>> = vec![];
        let mut depth = 0i32;
        let mut cur = String::new();
        for ch in raw.chars() {
            match ch {
                '(' | '[' => {
                    depth += 1;
                    cur.push(ch);
                }
                ')' | ']' => {
                    depth -= 1;
                    cur.push(ch);
                }
                _ if depth > 0 => cur.push(ch),
                ' ' | '\t' | '\n' => {
                    // Пробел сразу после hex-экранирования — ограничитель
                    // кода, а не комбинатор: `.css\\0032 p` — это ОДИН
                    // класс `css2p`, unescape ограничитель поглотит.
                    if ends_with_open_escape(&cur) {
                        cur.push(' ');
                        continue;
                    }
                    if !cur.is_empty() {
                        tokens.push(Ok(std::mem::take(&mut cur)));
                    }
                    tokens.push(Err(0));
                }
                '>' | '+' | '~' => {
                    if !cur.is_empty() {
                        tokens.push(Ok(std::mem::take(&mut cur)));
                    }
                    tokens.push(Err(match ch {
                        '>' => 1,
                        '+' => 2,
                        _ => 3,
                    }));
                }
                _ => cur.push(ch),
            }
        }
        if !cur.is_empty() {
            tokens.push(Ok(cur));
        }
        let mut compounds: Vec<String> = vec![];
        let mut combs: Vec<u8> = vec![];
        let mut pending: Option<u8> = None;
        for t in tokens {
            match t {
                Err(k) => {
                    // Пробелы вокруг знакового комбинатора — не «потомок»:
                    // знак сильнее.
                    pending = Some(pending.unwrap_or(0).max(k));
                }
                Ok(c) => {
                    if let Some(k) = pending.take() {
                        if compounds.is_empty() {
                            // Комбинатор до первой части — мусор.
                            if k > 0 {
                                return None;
                            }
                        } else {
                            combs.push(k);
                        }
                    }
                    compounds.push(c);
                }
            }
        }
        if compounds.is_empty() || combs.len() + 1 != compounds.len() {
            return None;
        }
        // Сборка слева направо: у `.a > .b + .c` предметом остаётся `.c`,
        // его сосед — `.b`, а предок соседа — `.a`.
        let mut sel = Selector::parse_compound(&compounds[0])?;
        for (comp, comb) in compounds[1..].iter().zip(&combs) {
            // A pseudo-element must end the complex selector (Selectors §3.1).
            if sel.pseudo.as_deref().is_some_and(is_pseudo_element) {
                return None;
            }
            let mut next = Selector::parse_compound(comp)?;
            match comb {
                0 => next.ancestor = Some(Box::new((sel, false))),
                1 => next.ancestor = Some(Box::new((sel, true))),
                2 => next.prev = Some(Box::new((sel, true))),
                _ => next.prev = Some(Box::new((sel, false))),
            }
            sel = next;
        }
        Some(sel)
    }
}
