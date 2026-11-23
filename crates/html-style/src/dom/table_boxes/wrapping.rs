//! Группировка табличных братьев: пробелы между ними внутри, хвост снаружи.
use super::{anon_element, anon_role, is_blank};
use crate::budget::admission::{Admission, Error};
use crate::dom::normalization::Transform;
use crate::dom::{Children, Element, Node};
use crate::style::computed::Display;

pub(crate) struct InlineTables;

impl Transform for InlineTables {
    type Context = bool;

    fn enter(&self, e: &mut Element, _: &bool, _: &Admission) -> Result<bool, Error> {
        Ok(e.inline && e.style.used().reset_box_layout.display.is_none())
    }

    fn sequence(
        &self,
        nodes: &mut Children,
        &inline: &bool,
        admission: &Admission,
    ) -> Result<(), Error> {
        if inline && let Some(output) = tables(nodes, Display::InlineTable, admission)? {
            *nodes = output;
        }
        Ok(())
    }
}

pub(super) fn tables(
    nodes: &[Node],
    display: Display,
    admission: &Admission,
) -> Result<Option<Children>, Error> {
    admission.work(nodes.len())?;
    if !nodes.iter().any(|n| anon_role(n).is_some()) {
        return Ok(None);
    }
    // Один ряд заменяет один или несколько узлов: выход не длиннее входа.
    let lease = admission.storage(Children::storage_bytes(nodes.len())?)?;
    let mut out = Vec::new();
    out.try_reserve_exact(nodes.len())
        .map_err(|_| Error::Allocation)?;
    let mut cursor = 0;
    while cursor < nodes.len() {
        admission.work(1)?;
        if anon_role(&nodes[cursor]).is_none() {
            out.push(nodes[cursor].clone());
            cursor += 1;
            continue;
        }
        let start = cursor;
        let mut end = cursor + 1;
        cursor = end;
        while cursor < nodes.len() {
            admission.work(1)?;
            if anon_role(&nodes[cursor]).is_some() {
                end = cursor + 1;
            } else if !is_blank(&nodes[cursor]) {
                break;
            }
            cursor += 1;
        }
        // Пробельный хвост остаётся снаружи; промежуточные пробелы входят в ряд.
        let children = copy(&nodes[start..end], admission)?;
        let mut table = anon_element("table", Vec::new());
        table.children = children;
        table.style.cascaded_mut().reset_box_layout.display = Some(display);
        table.marks.anonymous_wrapper = true;
        // Сохраняем существующую идентичность буфера проб анонимной таблицы.
        table.node_id = match &nodes[start] {
            Node::Element(e) => e.node_id.rotate_left(1) ^ 0x7ab1_e000,
            _ => unreachable!("table role belongs to an element"),
        };
        out.push(Node::Element(table));
        out.extend_from_slice(&nodes[end..cursor]);
    }
    Ok(Some(Children::from_reserved(out, lease)))
}

fn copy(nodes: &[Node], admission: &Admission) -> Result<Children, Error> {
    admission.work(nodes.len())?;
    let lease = admission.storage(Children::storage_bytes(nodes.len())?)?;
    let mut out = Vec::new();
    out.try_reserve_exact(nodes.len())
        .map_err(|_| Error::Allocation)?;
    out.extend_from_slice(nodes);
    Ok(Children::from_reserved(out, lease))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::budget::{BudgetKind, DocumentBudget, DocumentLimits};

    #[test]
    fn anonymous_table_buffer_refusal_releases_staged_storage() {
        let raw = vec![Node::Element(super::super::anon_element("td", vec![])); 2];
        let cap = Children::storage_bytes(raw.len()).unwrap();
        let budget = DocumentBudget::new(
            DocumentLimits::default()
                .lower(BudgetKind::LiveBytes, cap as u64)
                .unwrap(),
            Default::default(),
        )
        .unwrap();
        assert!(matches!(
            tables(&raw, Display::Table, &Admission::new(budget.revision())),
            Err(Error::Budget(_))
        ));
        assert_eq!(budget.used(BudgetKind::LiveBytes), 0);
        assert!(matches!(&raw[0], Node::Element(e) if e.tag == "td"));
    }
}
