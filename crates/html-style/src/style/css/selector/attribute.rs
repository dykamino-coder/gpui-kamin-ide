//! Условия атрибутных селекторов и сравнение значений.

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
