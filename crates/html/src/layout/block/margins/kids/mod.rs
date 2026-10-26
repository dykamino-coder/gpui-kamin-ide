//! Поля детей блока и распорки слитых полей (collapse_margins).

use crate::dom::Node;
use crate::layout::block::containing::with_inner_cb;
use crate::layout::block::inline_splits::split_block_in_inline;
use crate::layout::block::struts::{
    ZeroPaths, first_in_flow, leading_chain, margin_px, pin_inherited_margins, trailing_chain,
    zero_paths,
};
use crate::layout::block::{margin_edges, margin_height};
use crate::layout::box_kind::{inline_level_box, own_context};
use crate::layout::table::anon::wrap_anon_tables;
use crate::style::computed::Display;
use crate::style::values::value::Len;
mod struts;
pub(super) use struts::emit_margin_struts;

pub(super) fn collapse_kid_margins(
    out: &mut [Node],
    cb: Option<f32>,
    vcx: crate::document::state::ValueCx<'_>,
) {
    for node in out.iter_mut() {
        let Node::Element(e) = node else { continue };
        if inline_level_box(e) {
            continue;
        }
        // Отступ не протекает наружу и через край блока с собственным
        // контекстом: прокрутка, обрезка, гибкая раскладка, сетка,
        // позиционирование. Раньше учитывались только рамка и внутренний
        // отступ, и содержимое прокручиваемой панели вставало на 6-8 точек
        // выше браузерного.
        let own_context = own_context(e);
        // Отсечка по ЗНАЧЕНИЮ, а не по «свойство написано»: `padding: 0` и
        // `border: 0` схлопыванию не мешают (CSS 2.1 §8.3.1).
        let zero = |l: Option<Len>| matches!(l, None | Some(Len::Px(0.0)) | Some(Len::Pct(0.0)));
        // `margin-trim` (css-box-4 §margin-trim-block): поле первого/последнего
        // ребёнка у ВНУТРЕННЕГО края контейнера обнуляется вместе со всем,
        // что с ним схлопнулось. Идёт ДО гейта схлопывания: обрезка работает
        // и когда край закрыт рамкой или внутренним отступом — там поле
        // наружу не уходит, но обрезать его всё равно надо.
        // Собственное поле контейнера не трогается («but not its own»).
        // Блок внутри строчного (`<span><div>…</div></span>`) — тоже первый/
        // последний потоковый ребёнок контейнера: строчный рвётся на
        // анонимные коробки, а пустые куски коробок не дают (CSS 2.1
        // §9.2.1.1). Разрыв делает `blocks()` уже ПОСЛЕ этого шага, и цепочки
        // обрезки упирались в `<span>` как в строчную коробку
        // (`block-container-block-in-inline-001…007`). Контейнеру с обрезкой
        // рвём заранее тем же путём, что и `blocks()` (`wrap_anon_tables` →
        // `split_block_in_inline`); повторный разрыв там ничего не меняет.
        // У гибкого контейнера и сетки разрыва нет (`ordered_context`).
        if e.style.used().margin_trim & 3 != 0
            && !matches!(
                e.style.used().display,
                Some(Display::Flex)
                    | Some(Display::InlineFlex)
                    | Some(Display::Grid)
                    | Some(Display::InlineGrid)
                    | Some(Display::GridLanes)
            )
        {
            e.children = split_block_in_inline(&wrap_anon_tables(&e.children)).into();
        }
        if e.style.used().margin_trim & 1 != 0 {
            let mut eat = ZeroPaths::default();
            if leading_chain(&e.children, &mut eat, cb, vcx).is_some() {
                zero_paths(&mut e.children, &eat, true, vcx);
            }
        }
        if e.style.used().margin_trim & 2 != 0 {
            let mut eat = ZeroPaths::default();
            if trailing_chain(&e.children, &mut eat, vcx).is_some() {
                zero_paths(&mut e.children, &eat, false, vcx);
            }
        }
        // Root margins do not collapse with their children (CSS 2.1 §8.3.1).
        if e.tag == "html" {
            continue;
        }
        margin_edges::collapse_top(e, cb, vcx);
        // То же СНИЗУ: отступ последнего ребёнка протекает наружу, если
        // родителя от него не отделяют ни рамка, ни внутренний отступ, ни
        // заданная высота. Иначе следующий за родителем блок отодвигался на
        // сумму двух отступов вместо большего из них
        // (`text-align-end-015`: вторая коробка стояла на 20 точек ниже).
        // Не считать снизу ВСЮ хвостовую цепочку зеркально верхней
        // (`trailing_chain` + `zero_at(.., false, ..)`): наверху цепочку
        // ограничивает первый ребёнок с содержимым, а внизу ограничитель —
        // ВЫСОТА родителя, которой на этом шаге ещё нет; подъём уходит вглубь
        // и снимает поля там, где родитель уже кончился. Вернуть можно только
        // с настоящей проверкой итоговой высоты (тот же блокер, что у
        // `min-height` ниже; выигрыш — `margin-collapse-101/105`).
        //
        // Не снимать отсюда `min-height` целиком, хотя по спеке подъём
        // закрывает не написанное свойство, а РАСХОЖДЕНИЕ итоговой высоты с
        // высотой по содержимому (CSS 2.1 §8.3.1, так же считает Blink):
        // «дотянулась ли высота» на этом шаге ещё не известно, и снятие
        // условия открывает подъём там, где высота на деле выросла.
        // `min-height` закрывает подъём, только если он ДЕЙСТВИТЕЛЬНО тянет
        // коробку выше её содержимого. Нижняя оценка содержимого — сумма
        // разрешимых в точки высот блочных детей в потоке; неизвестная высота
        // хотя бы у одного оставляет прежний запрет
        // (`css-flexbox-height-animation-stretch`).
        let raises = margin_height::raises(e, vcx);
        // Край закрыт СВОИМИ свойствами: поле ребёнка остаётся внутри и
        // трогать его нечем.
        // Высота, которая «behaves as auto» (css-sizing-3: прозу CSS2
        // «computes to auto» читать так), подъёму не мешает: ключевые слова
        // содержимого по блочной оси блок-контейнера и доля при
        // НЕОПРЕДЕЛЁННОЙ высоте блока (§10.5), включая `stretch` — у нас это
        // та же доля. Прежде поле ребёнка оставалось внутри, и красная
        // подложка росла на 100 (`margin-collapse-with-indefinite-block-
        // size-001…005`). Blink — `BlockLengthUnresolvable`.
        let behaves_auto = match e.style.used().height {
            None | Some(Len::Auto) => true,
            Some(Len::MinContent) | Some(Len::MaxContent) | Some(Len::FitContent) => true,
            Some(Len::Pct(_)) => !vcx.collapse.height_def,
            _ => false,
        };
        if !zero(e.style.used().padding.bottom)
            || !zero(e.style.used().borders().bottom)
            || !behaves_auto
            || margin_height::separate(e, vcx)
            || own_context
        {
            continue;
        }
        // Снизу плавающий ИЛИ АБСОЛЮТНЫЙ ребёнок ЗАКРЫВАЕТ подъём: float
        // заякорен в потоке после последнего блока (box-shadow-overlapping-002),
        // а статическая позиция абсолютного считается от места в потоке —
        // утёкший отступ поднимал их обоих (z-index-015: квадрат вставал
        // на отступ параграфа выше эталона).
        let child_bottom =
            first_in_flow(e.children.iter().enumerate().rev().take_while(|(_, c)| {
                !matches!(c, Node::Element(ch)
                if ch.style.used().float.is_some()
                    || matches!(
                        ch.style.used().position,
                        Some(crate::style::computed::Position::Absolute)
                            | Some(crate::style::computed::Position::Fixed)
                    ))
            }))
            .and_then(|(i, ch)| {
                with_inner_cb(&e.style.used(), vcx, |vcx| {
                    margin_px(ch.style.used().margin.bottom, &ch.style.used(), vcx)
                })
                .map(|_| i)
            });
        // Проверено: запрет поглощения, когда в хвосте есть коробка с клиренсом
        // (CSS 2.1 §8.3.1, «does not collapse with a top margin that has
        // clearance»), ничего не меняет — `margin-collapse-clear-012/-013`
        // держит не это место: либо дело сюда не доходит (гейты выше), либо
        // `child_bottom` уже `None`; искать надо во втором проходе
        // (`cleared_run`).
        if let Some(i) = child_bottom {
            // Минимальная высота выше содержимого: поле последнего ребёнка
            // ПРИМЫКАЕТ к его нижнему краю (§8.3.1), но наружу не идёт и
            // родителя не растит — низ родителя решает `min-height` (§10.6.3).
            // Третьего исхода не было вовсе: поле оставалось внутри и место
            // занимало.
            if raises {
                if let Node::Element(ch) = &mut e.children[i] {
                    pin_inherited_margins(ch, false, true, vcx);
                    ch.style.used_mut().margin.bottom = Some(Len::Px(0.0));
                }
                continue;
            }
            margin_edges::collapse_bottom(e, vcx);
        }
    }
}
