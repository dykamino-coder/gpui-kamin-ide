//! Range-comparison operators for media features and reversed operands.

/// Сравнение в диапазонной записи медиа-фичи (mediaqueries-5 §3.3).
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum MqCmp {
    Lt,
    Le,
    Gt,
    Ge,
    Eq,
}

impl MqCmp {
    /// Перевернуть на другую сторону: в `(100px < width)` фича СПРАВА, и
    /// отношение к ней обратное.
    pub(crate) fn flip(self) -> Self {
        match self {
            MqCmp::Lt => MqCmp::Gt,
            MqCmp::Le => MqCmp::Ge,
            MqCmp::Gt => MqCmp::Lt,
            MqCmp::Ge => MqCmp::Le,
            MqCmp::Eq => MqCmp::Eq,
        }
    }

    pub(crate) fn holds(self, a: f32, b: f32) -> bool {
        match self {
            MqCmp::Lt => a < b,
            MqCmp::Le => a <= b,
            MqCmp::Gt => a > b,
            MqCmp::Ge => a >= b,
            MqCmp::Eq => a == b,
        }
    }
}
