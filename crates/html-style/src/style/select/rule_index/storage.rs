//! Учёт удерживаемого AST, включая подготовленные функциональные аргументы.

use super::*;

pub(super) fn bytes(sel: &Selector, admission: &Admission) -> Result<usize, Error> {
    let mut stack = Buffer::default();
    stack.push(sel, admission)?;
    let mut bytes = 0usize;
    let mut add = |n: usize| -> Result<(), Error> {
        bytes = bytes.checked_add(n).ok_or(Error::CapacityOverflow)?;
        Ok(())
    };
    while let Some(sel) = stack.pop() {
        admission.work(1)?;
        add(std::mem::size_of::<Selector>())?;
        add(sel.classes.capacity() * std::mem::size_of::<String>())?;
        add(sel.also.capacity() * std::mem::size_of::<String>())?;
        add(sel.attrs.capacity() * std::mem::size_of::<crate::style::css::AttrSel>())?;
        add(sel.arguments.capacity() * std::mem::size_of::<usize>())?;
        for text in sel
            .tag
            .iter()
            .chain(&sel.id)
            .chain(&sel.pseudo)
            .chain(&sel.classes)
            .chain(&sel.also)
        {
            add(text.capacity())?;
            add(std::mem::size_of::<String>())?;
        }
        for attr in &sel.attrs {
            add(std::mem::size_of_val(attr))?;
            add(attr.name.capacity())?;
            if let Some((_, value)) = &attr.op {
                add(value.capacity())?;
            }
        }
        for argument in &sel.arguments {
            add(std::mem::size_of_val(argument.as_ref()) + std::mem::size_of::<usize>() * 4)?;
            add(argument.raw.capacity())?;
            add(argument.list.capacity() * std::mem::size_of::<Selector>())?;
            if let Some(nth) = &argument.nth {
                add(nth.capacity())?;
            }
            for nested in &argument.list {
                stack.push(nested, admission)?;
            }
        }
        if let Some((ancestor, _)) = sel.ancestor.as_deref() {
            stack.push(ancestor, admission)?;
        }
        if let Some((previous, _)) = sel.prev.as_deref() {
            stack.push(previous, admission)?;
        }
    }
    Ok(bytes)
}
