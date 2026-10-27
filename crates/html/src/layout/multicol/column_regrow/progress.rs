//! Числовое состояние измеренной пробы без адресов временных layout-деревьев.

use super::ColumnStack;
use crate::layout::fragment::token::Error;
use std::collections::{BTreeMap, HashMap};
use std::fmt::{Debug, Write};

fn sorted<T: Debug>(output: &mut String, values: &HashMap<u64, T>) {
    let ordered: BTreeMap<_, _> = values.iter().collect();
    write!(output, "{ordered:?}").expect("writing a string");
}

pub(super) fn state(stack: &ColumnStack) -> Result<String, Error> {
    let kids = stack.kid_geoms();
    if kids.iter().any(|k| !k.h.is_finite() || !k.over.is_finite()) {
        return Err(Error::InvalidGeometry);
    }
    let mut output = format!("{kids:?}");
    for r in stack.children.iter().filter_map(|c| c.regrow.as_ref()) {
        let laid = &r.laid;
        if laid
            .boxes
            .values()
            .any(|(a, b)| !a.is_finite() || !b.is_finite())
        {
            return Err(Error::InvalidGeometry);
        }
        sorted(&mut output, &laid.boxes);
        sorted(&mut output, &laid.tables);
        sorted(&mut output, &laid.grids);
        sorted(&mut output, &laid.flexes);
        sorted(&mut output, &laid.content);
        sorted(&mut output, &laid.descendants);
        sorted(&mut output, &laid.lines);
        sorted(&mut output, &laid.margins);
        for (id, flow) in laid.flows.iter().collect::<BTreeMap<_, _>>() {
            write!(output, "{id}").expect("writing a string");
            // Внутренний HashMap тоже канонизируется; прочие поля — списки
            // исходных адресов, строк и float в порядке исходного потока.
            let mut geometry = flow.clone();
            sorted(&mut output, &geometry.boxes);
            geometry.boxes.clear();
            write!(output, "{geometry:?}").expect("writing a string");
        }
        sorted(&mut output, &r.opts.frag.float_floors);
        write!(output, "{:?}{:?}", r.floats, r.float_leads).expect("writing a string");
    }
    Ok(output)
}
