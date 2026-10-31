//! Ограничение `:visited` (Selectors 4 §8.1 «:visited privacy»): правило,
//! совпавшее с элементом лишь при посещённом состоянии его ссылки, меняет
//! только цвета, по которым нельзя измерить разметку. Список свойств —
//! Blink `css_properties.json5` (`visited_property_for`), он же Stylo
//! `VISITED_DEPENDENT_PROPERTIES`. Состояние ссылки при сопоставлении
//! (Stylo `VisitedHandlingMode::RelevantLinkVisited`): посещённой бывает
//! только «своя» ссылка элемента — он сам или ближайший предок-ссылка;
//! прочие ссылки сопоставляются непосещёнными.

/// Длинные свойства, которые задаёт посещённая ссылка.
pub const VISITED_DEPENDENT: &[&str] = &[
    "color",
    "background-color",
    "border-top-color",
    "border-right-color",
    "border-bottom-color",
    "border-left-color",
    "border-block-start-color",
    "border-block-end-color",
    "border-inline-start-color",
    "border-inline-end-color",
    "caret-color",
    "column-rule-color",
    "outline-color",
    "text-decoration-color",
    "text-emphasis-color",
    "fill",
    "stroke",
    "-webkit-text-fill-color",
    "-webkit-text-stroke-color",
];

/// Зависит ли свойство от посещённости ссылки.
pub fn visited_dependent(name: &str) -> bool {
    VISITED_DEPENDENT.contains(&name)
}

/// При каком состоянии ссылки правило совпало с элементом.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LinkMatch {
    /// При любом (или элемент вне посещённой ссылки): все объявления.
    #[default]
    Any,
    /// Только при посещённой своей ссылке: лишь зависимые от неё цвета.
    VisitedOnly,
    /// Только при непосещённой: всё, кроме этих цветов — их у посещённой
    /// ссылки задают правила посещённого состояния.
    UnvisitedOnly,
}

impl LinkMatch {
    /// Участвует ли объявление свойства `name` в каскаде.
    pub fn admits(self, name: &str) -> bool {
        match self {
            Self::Any => true,
            Self::VisitedOnly => visited_dependent(name),
            Self::UnvisitedOnly => !visited_dependent(name),
        }
    }

    /// Состояние по двум сопоставлениям; `None` — правило не совпало.
    pub fn of(unvisited: bool, visited: bool) -> Option<Self> {
        match (unvisited, visited) {
            (true, true) => Some(Self::Any),
            (true, false) => Some(Self::UnvisitedOnly),
            (false, true) => Some(Self::VisitedOnly),
            (false, false) => None,
        }
    }
}

/// Правило, совпавшее при состоянии `link`: копия с этим признаком. У
/// правила посещённого состояния `all` раскрывается в зависимые свойства
/// (css-cascade-5 §3.2: `all` — сокращение всех длинных свойств, из
/// которых посещённой ссылке доступны лишь эти).
pub fn restrict(rule: &super::Rule, link: LinkMatch) -> super::Rule {
    let mut out = rule.clone();
    out.link = link;
    if link == LinkMatch::VisitedOnly && rule.decls.iter().any(|d| d.name == "all") {
        let mut decls = super::DeclBlock::new();
        for d in &rule.decls {
            if d.name != "all" {
                decls.push_decl(d.clone());
                continue;
            }
            for name in VISITED_DEPENDENT {
                decls.push_decl(super::Declaration::new(
                    *name,
                    d.value.clone(),
                    d.important,
                    d.pos,
                ));
            }
        }
        out.decls = decls;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn link_state_filters_declarations() {
        assert!(LinkMatch::VisitedOnly.admits("color"));
        assert!(!LinkMatch::VisitedOnly.admits("accent-color"));
        assert!(!LinkMatch::VisitedOnly.admits("background-image"));
        assert!(LinkMatch::UnvisitedOnly.admits("accent-color"));
        assert!(!LinkMatch::UnvisitedOnly.admits("border-top-color"));
        assert_eq!(LinkMatch::of(true, true), Some(LinkMatch::Any));
        assert_eq!(LinkMatch::of(false, false), None);
    }
}
