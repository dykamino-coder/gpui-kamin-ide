//! Fixed-потомки копий: кто устанавливает для них содержащий блок, и снятие привязанных к вьюпорту.

use crate::dom::Node;

/// css-position-3 §abspos-breaking: «User
/// agents must not paginate the content of
/// fixed-positioned boxes». Копия фрагмента —
/// ПОЛНЫЙ клон поддерева, и `position: fixed`
/// внутри неё уезжает в слой ICB из КАЖДОЙ
/// копии (`render.rs:1790` -> `:1823` ->
/// `icb_push`). Слой лежит вне коробки
/// многоколоночника, маска колонки
/// (`flow.rs:998`) его не режет — на экране
/// вышло бы столько зелёных коробок, сколько
/// колонок. Оставляем фиксированного потомка
/// только в ПЕРВОЙ копии: там же, где стоит
/// его щуп статической позиции.
/// Blink делает это тем же разделением —
/// `out_of_flow_layout_part.cc:1607`: «This
/// does not include repeated fixed-positioned
/// elements».
/// Устанавливает ли коробка содержащий блок для
/// `position: fixed` (css-position-3 §fixed-cb:
/// «the nearest ancestor box that establishes a
/// fixed positioning containing block»;
/// css-transforms-1 §3: трансформ даёт
/// «containing block for all descendants … and
/// fixed-position descendants»). Список ДОСЛОВНО
/// тот же, что в `inline::inherit`
/// (`transform_ancestor`): `transform`,
/// `contain: layout`, `contain: paint`. Шире
/// брать нельзя — `blocks` решает по `under_tf`
/// из `inherit`, и расхождение дало бы коробку и
/// в копии, и в слое ICB.
pub(super) fn fixed_cb_box(c: &crate::style::computed::Computed) -> bool {
    c.transform.is_some()
        || c.contain_layout == Some(true)
        || c.contain_paint == Some(true)
        || c.will_change & crate::style::computed::wc::CB_FIXED != 0
}

/// css-position-3 §abspos-breaking: «User
/// agents must not paginate the content of
/// fixed-positioned boxes». Копия фрагмента —
/// ПОЛНЫЙ клон поддерева, и `position: fixed`
/// внутри неё уезжает в слой ICB из КАЖДОЙ
/// копии (`render.rs:1790` -> `:1823` ->
/// `icb_push`). Слой лежит вне коробки
/// многоколоночника, маска колонки
/// (`flow.rs:998`) его не режет — на экране
/// вышло бы столько зелёных коробок, сколько
/// колонок. Оставляем фиксированного потомка
/// только в ПЕРВОЙ копии: там же, где стоит
/// его щуп статической позиции.
/// Blink делает это тем же разделением —
/// `out_of_flow_layout_part.cc:1607`: «This
/// does not include repeated fixed-positioned
/// elements».
///
/// Запрет этот — про коробки, чей содержащий
/// блок ОКНО. Если содержащий блок `fixed`
/// лежит ВНУТРИ контекста фрагментации
/// (трансформированный или обособленный предок
/// внутри копии, либо сама коробка
/// многоколоночника), коробка — обычный абсолют
/// того предка, и §abspos-breaking выше требует
/// обратного: «positioned relative to its
/// containing block ignoring any fragmentation
/// breaks … may subsequently be broken over
/// several fragmentation containers». В слой ICB
/// такая коробка у нас и не уходит: `blocks`
/// (`render.rs:3966`) считает её `abs_like` при
/// `under_tf` и оставляет НА МЕСТЕ, значит копия
/// ≥ 1 без неё теряет единственную отрисовку.
/// Blink делит так же:
/// `fixedpos_containing_block`
/// (`out_of_flow_layout_part.cc:1369, :2978`) —
/// обычный фрагментаинерный потомок, и только
/// оконные попадают в
/// `repeated_fixedpos_descendants` (:1515).
///
/// `fixed_cb` — встретился ли по пути ВНИЗ от
/// коробки многоколоночника предок, который
/// устанавливает содержащий блок для `fixed`.
/// Предки ВЫШЕ многоколоночника сюда не входят:
/// их содержащий блок вне контекста, коробка по
/// спеке одна, и место ей — копия 0.
pub(super) fn drop_viewport_fixed(n: &Node, fixed_cb: bool) -> Option<Node> {
    match n {
        Node::Element(k)
            if !fixed_cb && k.style.position == Some(crate::style::computed::Position::Fixed) =>
        {
            None
        }
        Node::Element(k) => {
            let mut c = k.clone();
            let deeper = fixed_cb || fixed_cb_box(&k.style);
            c.children = k
                .children
                .iter()
                .filter_map(|kid| drop_viewport_fixed(kid, deeper))
                .collect();
            Some(Node::Element(c))
        }
        other => Some(other.clone()),
    }
}
