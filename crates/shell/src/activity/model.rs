//! Модель активностей: слоты, пины, порядок, персист.
//!
//! Перенесено без изменения поведения (`plan/100-refactor-250.md`).

use crate::activity::PanelSlot;
use crate::activity::dyn_tools::dyn_has;
use crate::activity::registry::{is_singleton, lookup};
use std::collections::HashMap;

/// Состояние одного слота.
#[derive(Clone, Debug, Default)]
pub struct PanelState {
    pub pinned: Vec<String>,
    /// None → placeholder («Open Tool»).
    pub active: Option<String>,
}
/// Вся панельная модель.
pub struct ActivityModel {
    states: HashMap<PanelSlot, PanelState>,
}
impl ActivityModel {
    pub fn new() -> Self {
        let mut states: HashMap<PanelSlot, PanelState> = HashMap::new();
        for slot in PanelSlot::ALL {
            states.insert(slot, PanelState::default());
        }
        // Сиды повторяют текущую раскладку KaminIDE: projects в сайдбаре,
        // дерево в right-top (в оригинале сид только sidebar, right-top юзер
        // получает пином — у нас дерево уже там по факту раскладки).
        states.get_mut(&PanelSlot::Sidebar).unwrap().pinned = vec!["projects".into()];
        states.get_mut(&PanelSlot::Sidebar).unwrap().active = Some("projects".into());
        states.get_mut(&PanelSlot::RightTop).unwrap().pinned = vec!["tree".into()];
        let cb = states.get_mut(&PanelSlot::CentralBottom).unwrap();
        cb.pinned = vec!["claudeBridgeConsole".into()];
        cb.active = Some("claudeBridgeConsole".into());
        let mn = states.get_mut(&PanelSlot::Main).unwrap();
        mn.pinned = vec!["claudeBridge".into()];
        mn.active = Some("claudeBridge".into());
        states.get_mut(&PanelSlot::RightTop).unwrap().active = Some("tree".into());
        Self { states }
    }

    pub fn state(&self, slot: PanelSlot) -> &PanelState {
        &self.states[&slot]
    }

    /// Сериализация в layout.json (ключ activityModel): {slot:{pinned,active}}.
    pub fn to_json(&self) -> serde_json::Value {
        let mut map = serde_json::Map::new();
        for slot in PanelSlot::ALL {
            let st = &self.states[&slot];
            map.insert(
                slot.as_str().to_string(),
                serde_json::json!({ "pinned": st.pinned, "active": st.active }),
            );
        }
        serde_json::Value::Object(map)
    }

    /// Восстановление из layout.json; отсутствующие слоты — сиды new().
    pub fn from_json(v: &serde_json::Value) -> ActivityModel {
        let mut model = ActivityModel::new();
        let Some(obj) = v.as_object() else {
            return model;
        };
        for slot in PanelSlot::ALL {
            let Some(sv) = obj.get(slot.as_str()) else {
                continue;
            };
            let st = model.state_mut(slot);
            if let Some(p) = sv.get("pinned").and_then(|x| x.as_array()) {
                st.pinned = p
                    .iter()
                    .filter_map(|s| s.as_str().map(String::from))
                    .collect();
            }
            st.active = sv
                .get("active")
                .and_then(|x| x.as_str())
                .map(String::from)
                .filter(|a| st.pinned.iter().any(|p| p == a));
        }
        // Миграция: пустые слоты → дефолты; старые id chat/console →
        // contributed-контейнеры
        {
            for slot in PanelSlot::ALL {
                let st = model.state_mut(slot);
                let remap = |s: &mut String| {
                    if s == "chat" {
                        *s = "claudeBridge".into();
                    } else if s == "console" {
                        *s = "claudeBridgeConsole".into();
                    }
                };
                for pin in st.pinned.iter_mut() {
                    remap(pin);
                }
                if let Some(ac) = st.active.as_mut() {
                    remap(ac);
                }
            }
            let cb = model.state_mut(PanelSlot::CentralBottom);
            if cb.pinned.is_empty() {
                cb.pinned = vec!["claudeBridgeConsole".into()];
                cb.active = Some("claudeBridgeConsole".into());
            }
            let mn = model.state_mut(PanelSlot::Main);
            if mn.pinned.is_empty() {
                mn.pinned = vec!["claudeBridge".into()];
                mn.active = Some("claudeBridge".into());
            }
        }
        model
    }

    fn state_mut(&mut self, slot: PanelSlot) -> &mut PanelState {
        self.states.get_mut(&slot).expect("slot exists")
    }

    /// Выкинуть из слотов id, которых нет ни среди builtin, ни в реестре
    /// расширений. Вызывать ТОЛЬКО когда реестр contributed уже приехал:
    /// иначе на старте выпадут легитимные тулы расширений. Возвращает true,
    /// если что-то удалили (тогда модель надо сохранить).
    pub fn prune_unknown(&mut self) -> bool {
        let mut changed = false;
        for slot in PanelSlot::ALL {
            let st = self.state_mut(slot);
            let before = st.pinned.len();
            st.pinned.retain(|id| lookup(id).is_some() || dyn_has(id));
            changed |= st.pinned.len() != before;
            self.repair(slot);
        }
        changed
    }

    /// Ремонт инварианта «active ∈ pinned». В persist мог остаться id из
    /// старых прогонов (например, id сессии, случайно запиненный отладочным
    /// эмитом): панель рисовала заглушку несуществующего тула, и ни одна
    /// плитка не подсвечивалась, потому что такого тула в баре нет.
    pub fn repair(&mut self, slot: PanelSlot) {
        let st = self.state_mut(slot);
        let ok = st
            .active
            .as_ref()
            .is_some_and(|a| st.pinned.iter().any(|p| p == a));
        if !ok {
            st.active = st.pinned.first().cloned();
        }
    }

    /// Слот, в котором тул сейчас закреплён (для тулов-одиночек — его дом).
    pub fn slot_of(&self, id: &str) -> Option<PanelSlot> {
        PanelSlot::ALL.into_iter().find(|s| self.is_pinned(*s, id))
    }

    pub fn is_pinned(&self, slot: PanelSlot, id: &str) -> bool {
        self.states[&slot].pinned.iter().any(|x| x == id)
    }

    /// pinToPanel: уже закреплён → активировать; иначе добавить + активировать.
    /// Тул-одиночка, живущий в ДРУГОМ слоте, не копируется — вызов no-op
    /// (переносить его можно только перетаскиванием).
    pub fn pin(&mut self, slot: PanelSlot, id: &str) {
        if is_singleton(id) && self.slot_of(id).is_some_and(|home| home != slot) {
            return;
        }
        let st = self.state_mut(slot);
        if !st.pinned.iter().any(|x| x == id) {
            st.pinned.push(id.to_string());
        }
        st.active = Some(id.to_string());
    }

    /// unpinFromPanel: убрать; если был активным — active ← pinned[0].
    pub fn unpin(&mut self, slot: PanelSlot, id: &str) {
        let st = self.state_mut(slot);
        st.pinned.retain(|x| x != id);
        if st.active.as_deref() == Some(id) {
            st.active = st.pinned.first().cloned();
        }
    }

    /// setActive: только если закреплён.
    pub fn set_active(&mut self, slot: PanelSlot, id: &str) {
        let st = self.state_mut(slot);
        if st.pinned.iter().any(|x| x == id) {
            st.active = Some(id.to_string());
        }
    }

    /// moveActivity: same-slot реордер (поправка индекса) / кросс-слот перенос
    /// (отказ при дубле в dst; active следует за тулом).
    pub fn move_activity(&mut self, src: PanelSlot, id: &str, dst: PanelSlot, dst_index: usize) {
        if !self.is_pinned(src, id) {
            return;
        }
        if src == dst {
            let st = self.state_mut(src);
            let from = st.pinned.iter().position(|x| x == id).unwrap();
            st.pinned.remove(from);
            let adjusted = if dst_index > from {
                dst_index - 1
            } else {
                dst_index
            }
            .min(st.pinned.len());
            st.pinned.insert(adjusted, id.to_string());
        } else {
            if self.is_pinned(dst, id) {
                return; // один тайл на (слот, id)
            }
            let src_st = self.state_mut(src);
            src_st.pinned.retain(|x| x != id);
            if src_st.active.as_deref() == Some(id) {
                src_st.active = src_st.pinned.first().cloned();
            }
            let dst_st = self.state_mut(dst);
            let idx = dst_index.min(dst_st.pinned.len());
            dst_st.pinned.insert(idx, id.to_string());
            dst_st.active = Some(id.to_string());
        }
    }
}
