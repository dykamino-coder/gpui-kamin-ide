//! Пробег флоатов от узла i: хост, сбор, хвост, ряд обтекания (wrap_float_run).

use super::covered::covered_tail_box;
use super::lone::wrap_lone_float;
use super::prepare::{host_for_run, wrap_into_host};
use super::row::{push_float_row, wrap_beside_covered};
use super::run::{gather_floaters, run_min_width, split_rest, strut_for_next};
use super::shaped::{shaped_run_of, wrap_shaped_run};
use crate::dom::{Element, Node};
use crate::layout::float::clear::{bfc_no_fit, clears_side, leading_clear};
use crate::style::computed::Computed;
use std::ops::ControlFlow;

#[allow(clippy::too_many_arguments)]
pub(super) fn wrap_float_run(
    parent: &Computed,
    cb_top_open: bool,
    em: f32,
    measured_ok: bool,
    parent_bfc: bool,
    cell_bfc: bool,
    cb_width: Option<crate::style::values::value::Len>,
    nodes: &mut Vec<Node>,
    out: &mut Vec<Node>,
    i: &mut usize,
    side: i8,
) -> ControlFlow<()> {
    let hosted = host_for_run(em, measured_ok, cb_width, &*nodes, &*out, *i);
    if let ControlFlow::Break(_) = wrap_into_host(
        parent,
        cb_top_open,
        parent_bfc,
        cell_bfc,
        &*nodes,
        out,
        i,
        hosted,
    ) {
        return ControlFlow::Break(());
    }
    let mut floaters: Vec<Element> = vec![];
    let mut sides: Vec<i8> = vec![];
    let mut j = *i;
    gather_floaters(parent, &*nodes, *i, side, &mut floaters, &mut sides, &mut j);
    let run_min_w = run_min_width(&*nodes, *i, j);
    let separates = |next: &Element| {
        clears_side(next.style.clear, side)
            || clears_side(leading_clear(next), side)
            || bfc_no_fit(next, run_min_w)
    };
    let (mut rest, mut out_of_flow) = split_rest(&*nodes, &mut j, separates);
    let clearance_strut = strut_for_next(cb_top_open, nodes, &*out, j, separates, &mut rest);
    if let ControlFlow::Break(_) = wrap_lone_float(
        parent,
        out,
        i,
        side,
        &mut floaters,
        j,
        &mut rest,
        &mut out_of_flow,
    ) {
        return ControlFlow::Break(());
    }
    let shaped_run = shaped_run_of(&floaters);
    if shaped_run && sides.iter().any(|s| *s != side) {
        // A run of floats on BOTH sides: `shape_flow` reads each float's
        // side from the float itself (css-shapes-1 §1 — every float's
        // shape narrows its own side of the line boxes).
        for (f, s) in floaters.iter_mut().zip(sides.iter()) {
            f.style.float = Some(*s);
        }
    }
    if let ControlFlow::Break(_) = wrap_shaped_run(
        parent,
        cb_width,
        out,
        i,
        side,
        &mut floaters,
        j,
        &mut rest,
        &mut out_of_flow,
        shaped_run,
    ) {
        return ControlFlow::Break(());
    }
    let covered = covered_tail_box(em, &*out, &floaters, &rest);
    if let ControlFlow::Break(_) = wrap_beside_covered(
        &*nodes,
        out,
        i,
        side,
        &mut floaters,
        j,
        separates,
        &mut rest,
        &mut out_of_flow,
        clearance_strut,
        covered,
    ) {
        return ControlFlow::Break(());
    }
    push_float_row(out, floaters, sides, rest);
    out.extend(out_of_flow);
    *i = j;
    // Бандовый хост: пробег флоатов ОБЕИХ сторон, не обрывающийся на
    // `clear`, и хвост, раскладываемый по полосам занятости вместо
    // флекс-ряда. Гейт узкий (см. `band_host`); не сошёлся — идём
    // сегодняшней веткой ниже, ни строки в ней не меняя.
    // Правило 6 (§9.5.1): верх флоата — верх строки, в которой он
    // объявлен. Прогон АТОМОВ известного размера перед флоатом уходит в
    // хост вместе с ним, иначе флоат встаёт ПОД прогоном (`floats-001`).
    // Прогон ТЕКСТА полосам не отдаём: наборщик строк про них не знает.
    // Подряд идущие плавающие блоки стоят в ОДНОМ ряду, а не каждый в
    // своём: `float: left` у четырёх соседей выстраивает их бок о бок.
    // Прежде каждый начинал свой ряд, и они вставали столбиком.
    // Сторона КАЖДОГО собранного флоата: пробег берёт обе, и левые с
    // правыми стоят в одном ряду. Прежде пробег обрывался на смене
    // стороны, `rest` выходил пустым, и одинокий флоат становился обычным
    // блоком — он съедал строку потока, а всё за ним падало на его высоту
    // (`floats-wrap-top-below-bfc-*` и родня).
    // Соседи до ближайшего `clear` — они и обтекают. Внепоточный
    // (absolute/fixed) сосед НЕ обтекает: в колонке ряда он получил бы
    // её своим содержащим блоком, и `right: 96px` считался от узкой
    // колонки, а не от контейнера (эталоны css-shapes с рядом
    // absolute-коробок выходили пустыми).
    // Кто рвёт ряд: `clear` брата, `clear` в начале прозрачной обёртки
    // (`leading_clear`) и коробка своего контекста, которой рядом с
    // флоатами пробега нет места (`bfc_no_fit`). Все трое обязаны встать
    // ПОД флоатами, а не в колонку сбоку от них.
    // ПРОБОВАЛИ И ОТКАТИЛИ: забирать строчный прогон, стоящий ПЕРЕД
    // пробегом флоатов, обратно из `out` в хвост — §9.5.1 п.6 держит
    // верх флоата на верху текущей строчной коробки, а §9.5 сужает саму
    // эту строку. Гейт «только при иначе пустом хвосте». Проба по 319
    // парам семей `floats*`, `float-*`, `clear-float-*`: приобретено 1
    // (`floats-001` 3.84 -> 0.00), потеряно 4 — `float-nowrap-3` 0.14 ->
    // 0.50, `-7` 0.00 -> 0.40, `-9` 0.26 -> 0.67, `floats-114` 0.06 ->
    // 1.14, и `float-nowrap-hyphen-rewind-1` 0.42 -> 2.46. Прогон надо
    // не переносить целиком, а сужать по полосам — этого канала нет.
    //
    // §9.5.2: у очищающей коробки верхнее поле ЗАМЕНЯЕТСЯ зазором, а не
    // складывается с ним: её верх = max(своё место, низ флоатов). Ряд
    // обтекания сам даёт `max(флоаты, колонка)`, поэтому остаток поля
    // переносится распоркой в КОНЕЦ колонки, а у самой коробки гасится —
    // иначе она опускалась на своё поле ниже низа флоата.
    //
    // Оговорка: если брат ПЕРЕД флоатом схлопывается насквозь, его поле
    // ещё не выложено, и верх ряда у нас и так ниже настоящего — тогда
    // перенос только удваивает сдвиг (`clearance-006`).
    // Плавающий блок, рядом с которым НЕЧЕМУ обтекать, рядом не нуждается:
    // он остаётся обычным блоком потока. Ряд в этом случае только вредил —
    // ширину внутри него раскладка мерила по самому узкому слову.
    // ★ ЗАМЕРЕНО И ОТКАЧЕНО: распускать ряд, когда обтекать нечем, для
    // ЛЮБОГО числа плавающих (а не только одного) — css-text 1018 → 1019,
    // но flexbox 320 → **306**. Ряд соседних плавающих блоков нужен: без
    // него они встают столбиком.
    // ПРОБОВАЛИ И ОТКАТИЛИ: пробег РАВНОШИРОКИХ флоатов без заданной
    // высоты раскладывать колонкой флекс-рядов по `floor(cb / mw)` штук в
    // ряд (§9.5.1 п.3 и п.5) — высоты для этого знать не нужно. Замерено
    // по всему CSS2: 0 и 0. Пары `c414-flt-fit-002/003/004` держит не
    // раскладка рядов, а что-то ещё.
    //
    // Обтекание ФОРМОЙ (`shape-outside`): ряд-колонка его не выразит —
    // строки должны сужаться каждая по-своему. Плавающие блоки с
    // ИЗВЕСТНЫМИ размерами уходят синтетическим узлом shape-flow:
    // сборка положит их absolute и передаст вырезы абзацу.
    // ЗАМЕРЕНО И ОТКАЧЕНО: пускать сюда пробег из флоатов ОБЕИХ сторон
    // (снять этот конъюнкт и вернуть сторону детям перед пушем). Срез из
    // 259 пар семей *shape*: 0 и 0 — тройка `spec-examples/shape-outside-
    // 001…003` как была «красное видно», так и осталась, её держит не
    // односторонность пробега.
    // §9.5: обычный блок потока флоат ПЕРЕКРЫВАЕТ — обходят только его
    // строки, а Приложение E кладёт флоат (шаг 5) поверх фонов потока
    // (шаг 4). Флекс-ряд наложения не выражает вовсе и ставит блок СБОКУ.
    //
    // Берётся ровно тот случай, где итог считается арифметикой: ОДИН
    // ЛЕВЫЙ флоат с margin box в точках и хвост из ОДНОГО пустого блока
    // потока, border box которого целиком ложится внутрь этого margin
    // box (гейт — `covered_flow_tail`). Наложение выражается двумя
    // ОБЫЧНЫМИ блоками потока: сосед, а следом флоат с подъёмом на
    // высоту соседа. Порядок обязателен — флоат ВТОРЫМ, иначе он ляжет
    // ПОД соседа (проба §5.1: «красное видно»). Поле снизу возвращает
    // поток на НИЗ СОСЕДА: §10.6.3 — флоат высоты родителя не растит
    // (проба §5.2: 0.00 на случае, где флоат выше соседа).
    //
    // Прогон текста ПЕРЕД флоатом правку отменяет: там правило 6 §9.5.1
    // держит верх флоата на верху текущей строки, а этого канала здесь
    // нет.
    // ★ ЗАМЕРЕНО И ОТКАЧЕНО (01.09): §9.5 говорит, что коробка блочного
    // уровня флоат НЕ обходит — обходят только её строки. Пробовал
    // выразить это наложением: когда в хвосте нет ни одного текстового
    // узла, блоки идут своим чередом, а флоат кладётся поверх них
    // абсолютом в относительной обёртке. Срез из 299 пар флоатов:
    // 198 → 189 без гейта и 198 → 193 с гейтом «хвост не образует
    // своего контекста». Приобретение одно (`floats-rule3-outside-
    // right-001` 1.84 → 0.00), потери — `floats-rule7-outside-left-001`
    // 0.00 → 1.79, `floats-wrap-bfc-001/003-*-table` 0.00 → 2-7,
    // `floats-wrap-bfc-with-margin-008/009` 0.00 → 1.05. Наложение
    // рушит вертикальное место: у нас флоат в ряду задаёт высоту, а
    // абсолют её больше не держит.
    // Хвост со СВОИМИ боковыми полями остаётся на прежней основе: остаток
    // ряда достаётся ему без учёта этих полей, и коробка выходит у́же
    // нужного (`floats-wrap-bfc-with-margin-004/005/008/009`).
    ControlFlow::Continue(())
}
