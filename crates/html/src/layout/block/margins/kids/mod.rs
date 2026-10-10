//! Поля детей блока и распорки слитых полей (collapse_margins).

use crate::dom::Node;
use crate::layout::block::containing::with_inner_cb;
use crate::layout::block::margins::COLLAPSE_CB_HEIGHT_DEF;
use crate::layout::block::struts::{
    first_in_flow, leading_chain, margin_px, pin_inherited_margins, trailing_chain, zero_at,
};
use crate::layout::block::{margin_edges, margin_height};
use crate::layout::table::anon::wrap_anon_tables;
use crate::render::{inline_level_box, own_context, split_block_in_inline};
use crate::style::computed::Display;
use crate::style::values::value::Len;
mod struts;
pub(super) use struts::emit_margin_struts;

pub(super) fn collapse_kid_margins(out: &mut [Node]) {
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
        if e.style.margin_trim & 3 != 0
            && !matches!(
                e.style.display,
                Some(Display::Flex)
                    | Some(Display::InlineFlex)
                    | Some(Display::Grid)
                    | Some(Display::InlineGrid)
                    | Some(Display::GridLanes)
            )
        {
            e.children = split_block_in_inline(&wrap_anon_tables(&e.children));
        }
        if e.style.margin_trim & 1 != 0 {
            let mut path: Vec<usize> = vec![];
            let mut eat: Vec<(Vec<usize>, bool)> = vec![];
            if leading_chain(&e.children, &mut path, &mut eat).is_some() {
                for (p, deep) in &eat {
                    zero_at(&mut e.children, p, true, *deep);
                }
            }
        }
        if e.style.margin_trim & 2 != 0 {
            let mut path: Vec<usize> = vec![];
            let mut eat: Vec<(Vec<usize>, bool)> = vec![];
            if trailing_chain(&e.children, &mut path, &mut eat).is_some() {
                for (p, deep) in &eat {
                    zero_at(&mut e.children, p, false, *deep);
                }
            }
        }
        // Root margins do not collapse with their children (CSS 2.1 §8.3.1).
        if e.tag == "html" {
            continue;
        }
        margin_edges::collapse_top(e);
        // То же СНИЗУ: отступ последнего ребёнка протекает наружу, если
        // родителя от него не отделяют ни рамка, ни внутренний отступ, ни
        // заданная высота. Иначе следующий за родителем блок отодвигался на
        // сумму двух отступов вместо большего из них
        // (`text-align-end-015`: вторая коробка стояла на 20 точек ниже).
        // ЗАМЕРЕНО И ОТКАЧЕНО: считать снизу ВСЮ хвостовую цепочку, зеркально
        // верхней (`trailing_chain` + `zero_at(.., false, ..)`). CSS2 4684 ->
        // 4594: прибавка 4 (`margin-collapse-101/105`, два
        // `inline-formatting-context`) против 94 потерь. Наверху цепочку
        // ограничивает первый ребёнок с содержимым, а внизу ограничитель —
        // ВЫСОТА родителя, которой на этом шаге ещё нет: подъём уходит вглубь
        // и снимает поля там, где родитель на деле уже кончился. Возвращать
        // вместе с настоящей проверкой итоговой высоты (тот же блокер, что у
        // `min-height` ниже).
        //
        // ПРОБОВАЛИ И ОТКАТИЛИ: снять отсюда `min-height`, потому что по
        // спеке подъём закрывает не написанное свойство, а РАСХОЖДЕНИЕ
        // итоговой высоты с высотой по содержимому (CSS 2.1 §8.3.1, так же
        // считает Blink). Замерено: oldfront 2332 -> 2328. Нашей раскладке
        // «дотянулась ли высота» на этом шаге ещё не известно, и снятие
        // условия открывало подъём там, где высота на деле выросла.
        // Возвращать вместе с настоящей проверкой итоговой высоты.
        // `min-height` закрывает подъём, только если он ДЕЙСТВИТЕЛЬНО тянет
        // коробку выше её содержимого (§8.3.1 говорит о РАСХОЖДЕНИИ итоговой
        // высоты с высотой по содержимому, а не о написанном свойстве).
        // Нижняя оценка содержимого — сумма разрешимых в точки высот блочных
        // детей в потоке; неизвестная высота хотя бы у одного оставляет
        // прежний запрет. Замерено: CSS2 5074 -> 5077, oldfront 2353 -> 2352
        // (`css-flexbox-height-animation-stretch` 0.47 -> 1.00).
        let raises = margin_height::raises(e);
        // Край закрыт СВОИМИ свойствами: поле ребёнка остаётся внутри и
        // трогать его нечем.
        // Высота, которая «behaves as auto» (css-sizing-3: прозу CSS2
        // «computes to auto» читать так), подъёму не мешает: ключевые слова
        // содержимого по блочной оси блок-контейнера и доля при
        // НЕОПРЕДЕЛЁННОЙ высоте блока (§10.5), включая `stretch` — у нас это
        // та же доля. Прежде поле ребёнка оставалось внутри, и красная
        // подложка росла на 100 (`margin-collapse-with-indefinite-block-
        // size-001…005`). Blink — `BlockLengthUnresolvable`.
        let behaves_auto = match e.style.height {
            None | Some(Len::Auto) => true,
            Some(Len::MinContent) | Some(Len::MaxContent) | Some(Len::FitContent) => true,
            Some(Len::Pct(_)) => !COLLAPSE_CB_HEIGHT_DEF.with(std::cell::Cell::get),
            _ => false,
        };
        if !zero(e.style.padding.bottom)
            || !zero(e.style.borders().bottom)
            || !behaves_auto
            || margin_height::separate(e)
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
                if ch.style.float.is_some()
                    || matches!(
                        ch.style.position,
                        Some(crate::style::computed::Position::Absolute)
                            | Some(crate::style::computed::Position::Fixed)
                    ))
            }))
            .and_then(|(i, ch)| {
                with_inner_cb(&e.style, || margin_px(ch.style.margin.bottom, &ch.style)).map(|_| i)
            });
        // ★ ЗАМЕРЕНО И ОТКАЧЕНО (05.09): запрет поглощения, когда в хвосте
        // есть коробка с клиренсом (CSS 2.1 §8.3.1, «does not collapse with a top
        // margin that has clearance»): срез CSS2 6204 пары, 5691 -> 5691 (+0/-0),
        // целевые `margin-collapse-clear-012/-013` как были «красное видно»,
        // так и остались. Значит потеря не здесь: до этого места дело либо не
        // доходит (гейты выше), либо `child_bottom` уже `None` — искать
        // надо во втором проходе (`cleared_run`).
        if let Some(i) = child_bottom {
            // Минимальная высота выше содержимого: поле последнего ребёнка
            // ПРИМЫКАЕТ к его нижнему краю (§8.3.1), но наружу не идёт и
            // родителя не растит — низ родителя решает `min-height` (§10.6.3).
            // Третьего исхода не было вовсе: поле оставалось внутри и место
            // занимало.
            if raises {
                if let Node::Element(ch) = &mut e.children[i] {
                    pin_inherited_margins(ch, false, true);
                    ch.style.margin.bottom = Some(Len::Px(0.0));
                }
                continue;
            }
            margin_edges::collapse_bottom(e);
        }
    }
}
