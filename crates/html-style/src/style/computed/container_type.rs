//! Оси запросов размера задаёт container-type, независимо от contain.

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ContainerType {
    #[default]
    Normal,
    InlineSize,
    Size,
}

impl ContainerType {
    pub fn is_size_query(self) -> bool {
        self != Self::Normal
    }
}
