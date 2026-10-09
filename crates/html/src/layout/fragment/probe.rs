//! Пробы фрагментации.
// owner: A

use crate::render::*;

/// Коробка и всё её поддерево — ОБЫЧНЫЕ блоки: ни гибкого контейнера, ни
/// сетки, ни таблицы, ни вложенного многоколоночника. Только у такого
/// поддерева мера `shape_full` совпадает с тем, что рисует движок: у гибкого
/// контейнера с переносом она складывает элементы стопкой (`shape_full`,
/// ветка без `row_nowrap`), у вложенного многоколоночника — не знает про его
/// собственные колонки. Протяжённость параллельного потока, снятая с такого
/// приближения, была бы выдуманной, и зелёные `multicol-nested-026`,
/// `single-line-row-flex-fragmentation-039`,
/// `multi-line-row-flex-fragmentation-093`,
/// `single-line-column-flex-fragmentation-043/058` разъехались бы
/// (`target/scout-fragparallel-2026-09.md` §5). `display: flow-root` сюда
/// входит: он сводится к `Block` (`computed.rs:3057`).
pub(crate) fn plain_block_tree(c: &Element, depth: u8) -> bool {
    if c.style.column_count.is_some() || c.style.column_width.is_some() {
        return false;
    }
    if c.style.webkit_box == Some(true) {
        return false;
    }
    if !matches!(
        c.style.display,
        None | Some(Display::Block) | Some(Display::ListItem)
    ) {
        return false;
    }
    if depth == 0 {
        return true;
    }
    c.children.iter().all(|n| match n {
        Node::Element(k) => k.inline || plain_block_tree(k, depth - 1),
        _ => true,
    })
}

/// Поддерево обычных блоков (`plain_block_tree`), где допустим и flex-ряд с
/// переносом, каждый элемент которого занимает всю строку (`flex-basis` или
/// `width` 100%, без роста и боковых полей): строка = элемент, и элементы идут
/// блочной стопкой (css-flexbox-1 §9.3 шаг 5: следующий элемент в строку не
/// помещается). Такую стопку мера `shape_full` ведёт так же точно, как блок
/// (ветка «строка = элемент»), и её переполнение заданной высоты продолжается
/// в следующем фрагментаинере параллельным потоком (css-break-3 §3).
pub(crate) fn stacked_flex_tree(c: &Element, depth: u8) -> bool {
    use crate::computed::FlexDir;
    if c.style.column_count.is_some() || c.style.column_width.is_some() || c.style.webkit_box == Some(true) {
        return false;
    }
    let s = &c.style;
    let flex_stack = s.display == Some(Display::Flex)
        && matches!(s.flex_dir, None | Some(FlexDir::Row))
        && s.flex_wrap == Some(true)
        && s.flex_wrap_reverse != Some(true)
        && s.vertical != Some(true)
        && s.gap.is_none()
        && !flex_gap_rules(s);
    if !flex_stack && !matches!(s.display, None | Some(Display::Block) | Some(Display::ListItem)) {
        return false;
    }
    if depth == 0 {
        return !flex_stack;
    }
    c.children.iter().all(|n| match n {
        Node::Element(k) if flex_stack => {
            let ks = &k.style;
            let full = |l: &Option<Len>| matches!(l, Some(Len::Pct(p)) if (*p - 1.0).abs() < 1e-4);
            let zero = |l: &Option<Len>| match l {
                None => true,
                Some(Len::Px(v)) => v.abs() < 0.01,
                _ => false,
            };
            !k.inline
                && !out_of_flow(ks)
                && ks.position.is_none()
                && ks.flex_grow.is_none_or(|g| g == 0.0)
                && ks.basis_content != Some(true)
                && match ks.flex_basis {
                    None | Some(Len::Auto) => full(&ks.width),
                    _ => full(&ks.flex_basis),
                }
                && zero(&ks.margin.left)
                && zero(&ks.margin.right)
                && zero(&ks.padding.left)
                && zero(&ks.padding.right)
                && zero(&ks.borders().left)
                && zero(&ks.borders().right)
                && stacked_flex_tree(k, depth - 1)
        }
        Node::Element(k) => k.inline || stacked_flex_tree(k, depth - 1),
        _ => true,
    })
}

/// Мера блочного ребёнка для укладки колонок: высота с
/// отбивками и рамками, поля и точки ЗАКОННОГО разреза
/// (css-break-3 §4.3, класс A) — границы вложенных
/// блочных детей, рекурсивно. Высота `auto` складывается
/// из тех же детей со схлопыванием полей (CSS 2.1
/// §8.3.1); строчное содержимое высоты не даёт — такой
/// ребёнок мерить нечем, и весь стек идёт другим путём.
pub(crate) fn shape(c: &Element, depth: u8) -> Option<(f32, f32, f32, Vec<(f32, f32)>)> {
    shape_full(c, depth, ShapeCx::COLUMNS).map(|s| (s.0, s.1, s.2, s.3))
}

/// `contain: size` — монолит везде, где есть фрагментация (Blink
/// `LayoutBox::IsMonolithic`, `layout_box.cc`: `ShouldApplySizeContainment()`
/// = `StyleRef().ContainsSize() && IsEligibleForSizeContainment()`; обе оси —
/// `size`/`strict`, не `inline-size`; `contain-intrinsic-size` не участвует;
/// таблица, группа, ряд и ячейка не годны — `layout_table*.h`
/// `IsEligibleForSizeContainment() … return false`). `content-visibility:
/// hidden` ставит тот же `contain_size`.
pub(crate) fn size_monolith(k: &Element) -> bool {
    k.style.contains_block_size()
        && !matches!(
            k.style.display,
            Some(Display::TableCell) | Some(Display::TableRow) | Some(Display::TableRowGroup)
        )
        && !matches!(
            k.tag.as_str(),
            "td" | "th" | "tr" | "thead" | "tbody" | "tfoot"
        )
}

/// Коробка, принудительные разрывы ВНУТРИ которой фрагментации не видны:
/// `contain: size` (`size_monolith`) и прокручиваемая коробка — css-break-4
/// §possible-breaks: «Any forced breaks within such boxes therefore cannot split
/// the box, and must therefore also be ignored by the box’s own fragmentation
/// context». `break-inside: avoid` сюда НЕ входит: там принудительный разрыв
/// сильнее запрета (§forced-breaks). Ряд и группа рядов `overflow` не берут
/// (css-overflow-3, «Applies to»), ячейка — берёт.
/// Есть ли в поддереве принудительный разрыв (`break-before/after` любого
/// потомка, кроме уходящих внутрь монолита, `forced_opaque`).
pub(crate) fn forced_inside(e: &Element, depth: u8) -> bool {
    if depth == 0 {
        return false;
    }
    e.children.iter().any(|n| match n {
        // Вне потока — свой поток (`edge_break`).
        Node::Element(k)
            if matches!(
                k.style.position,
                Some(crate::computed::Position::Absolute) | Some(crate::computed::Position::Fixed)
            ) =>
        {
            false
        }
        Node::Element(k) => {
            k.style.break_before_force
                || k.style.break_after_force
                || (!forced_opaque(k) && forced_inside(k, depth - 1))
        }
        _ => false,
    })
}

pub(crate) fn forced_opaque(k: &Element) -> bool {
    let scrolls = |o: Option<crate::computed::Overflow>| {
        matches!(o, Some(crate::computed::Overflow::Scroll))
    };
    size_monolith(k)
        || ((scrolls(k.style.overflow_x) || scrolls(k.style.overflow_y))
            && !matches!(
                k.style.display,
                Some(Display::TableRow) | Some(Display::TableRowGroup)
            )
            && !matches!(k.tag.as_str(), "tr" | "thead" | "tbody" | "tfoot"))
}
