//! Сдвиг заместителя к его месту: дыра в строке, следующая строка, оси и направление.

use super::super::Spot;
use gpui::{Bounds, Pixels, px};

pub(crate) fn late_shift(bounds: Bounds<Pixels>, now: Spot) -> gpui::Point<Pixels> {
    match (now.hole, now.next_line) {
        // Блочный: слева — край содержимого родителя (там же, где стоит сам
        // заместитель), сверху — низ строки, в которой он оказался.
        // Поперёк потока строк заместитель уже стоит у нужного края (его
        // ставит распорка ряда), поэтому правится только та ось, вдоль
        // которой идут строки.
        (Some(hole), Some(line)) if now.vertical && now.vertical_rl => {
            gpui::point(hole.origin.x - px(line) - bounds.origin.x, px(0.0))
        }
        (Some(hole), Some(line)) if now.vertical => {
            gpui::point(hole.origin.x + px(line) - bounds.origin.x, px(0.0))
        }
        // Horizontal block-level box in a line (Blink `InlineLayoutAlgorithm::
        // PlaceOutOfFlowObjects`): its static position is the line's start and
        // the line's TOP when no in-flow content precedes it on the line, the
        // line's bottom otherwise (`static-inside-inline-001..003`). A probe
        // in a row of words spans its line (`spot_probe`); a text paragraph
        // hands its overlay probes the line box (`with_line_box`).
        // Self-alignment uses the inline parent's static-position
        // rectangle: zero-thick along the line at the static point, the line
        // box across it (Blink `AddOutOfFlowInlineChildCandidate`;
        // `align-self-static-position-003/007`).
        (Some(hole), Some(line)) => {
            let (line_top, lh, line_start) = now.line_box.unwrap_or((
                hole.origin.y,
                if hole.size.height > px(0.0) {
                    hole.size.height
                } else {
                    px(line)
                },
                bounds.origin.x,
            ));
            let preceded = now.rtl || hole.origin.x - line_start > px(0.5);
            let top = line_top + if preceded { lh } else { px(0.0) };
            match now.self_align.filter(|_| now.line_rect) {
                Some((kx, ky)) => gpui::point(
                    -bounds.size.width * kx,
                    top + (lh - bounds.size.height) * ky - bounds.origin.y,
                ),
                None => gpui::point(px(0.0), top - bounds.origin.y),
            }
        }
        // Static-position rectangle with `justify-self`/`align-self`
        // (`Spot::self_align`, physical fractions): the block strut is the
        // parent's content box along the inline axis and zero-thick across
        // it, so one formula serves both writing modes (horizontal: the hole
        // has no height; vertical: no width). Ahead of the rtl/vertical arms
        // — `render::static_self_align` already chose the side
        // (`align-self-static-position-001/002/006/008`,
        // `justify-self-static-position-001`).
        (Some(hole), None) if now.block_strut && now.self_align.is_some() => {
            let (kx, ky) = now.self_align.unwrap_or_default();
            gpui::point(
                hole.origin.x + (hole.size.width - bounds.size.width) * kx - bounds.origin.x,
                hole.origin.y + (hole.size.height - bounds.size.height) * ky - bounds.origin.y,
            )
        }
        // Начало строчной оси при письме справа налево — правый край:
        // поперёк потока заместителя уже выровняла обёртка (justify_end /
        // items_end), правится только ось потока строк.
        // Блочный заместитель в вертикальном письме: начало блочного
        // потока при vertical-rl — ПРАВЫЙ край, коробка уходит от него
        // влево (css-writing-modes §block-flow).
        (Some(hole), None) if now.vertical => {
            let x = if now.vertical_rl {
                hole.origin.x - bounds.size.width
            } else {
                hole.origin.x
            };
            // `direction: rtl` turns the vertical inline axis bottom-to-top
            // (css-writing-modes-4 §6.4): the block strut spans the parent's
            // content box along it, and the box hangs from its BOTTOM end by
            // its inline-start (bottom) edge (Blink `absolute_utils.cc`
            // `GetStaticPositionInsetBias` → `InsetBias::kEnd`;
            // `abs-pos-border-offset-001`).
            let y = if now.rtl && now.block_strut {
                hole.origin.y + hole.size.height - bounds.size.height
            } else {
                hole.origin.y
            };
            gpui::point(x - bounds.origin.x, y - bounds.origin.y)
        }
        // Письмо справа налево: заместитель вешается ПРАВЫМ краем на
        // начало строчной оси (CSS 2.1 §10.3.7: статическая позиция в rtl
        // отсчитывается от правого края). У блочной распорки во всю
        // ширину правый край — правый край содержимого, у точечной в
        // строке — сама точка.
        // Правокрайняя формула верна для БЛОЧНОЙ распорки во всю
        // ширину; у ТОЧЕЧНОГО щупа в строке ширина нулевая, и вычитание
        // своей ширины уводило коробку влево на неё целиком
        // (htb-rtl-*: регресс 08-19).
        // ЗАМЕРЕНО, ЭФФЕКТА НЕТ (03.09): в вертикальном письме отсчитывать
        // `rtl` от НИЖНЕГО края дырки, а не от правого (css-writing-modes-3
        // §7.1: `direction` переворачивает строчную ось, а она вертикальна).
        // Правка рассуждением верна, но на двенадцати пробах `apw-*` не
        // сдвинула НИ ОДНОЙ сотой, и сборка HEAD без неё даёт те же числа.
        // Прежняя запись «apw-l 2.67 -> 0.00» была ЛОЖНОЙ: список пар
        // собирался конкатенацией, `"\a"` давал байт BEL, обе стороны не
        // грузились, и пустая страница сходилась с пустой. Списки строить
        // только через `Path`, проверять `od -c`.
        // Возвращать вместе с независимым решением осей (корень B): до
        // боевых пар этот рукав просто не доезжает.
        // Повёрнутый абзац: дырка уже экранная (`vt_map`). Строчная ось
        // здесь вертикальна, и при `direction: rtl` она идёт снизу вверх
        // — щуп отмечает НИЖНИЙ край коробки, а не верхний
        // (`abs-pos-non-replaced-vrl-126`: коробка стояла ровно на свою
        // высоту ниже нужного). Горизонтальные rtl-рукава ниже к такой
        // дырке не относятся — они сдвигают по x.
        (Some(hole), None) if now.rotated && now.rtl => gpui::point(
            hole.origin.x - bounds.origin.x,
            hole.origin.y - bounds.size.height - bounds.origin.y,
        ),
        (Some(hole), None) if now.rotated => hole.origin - bounds.origin,
        // Гейт — ПОРОДА щупа, а не размер дырки. Полоса выравнивания
        // может быть нулевой (содержащий блок нулевой ширины), но сторона
        // отсчёта от этого не меняется: §10.3.7 при rtl вешает на
        // статическую точку `right`, а не `left`. Blink разводит эти два
        // шага явно — прибавка `available_inline_size` (может быть нулём,
        // `block_layout_algorithm.cc:1740`) и `InsetBias::kEnd`
        // (`absolute_utils.cc:34`).
        // Прямоугольник статической позиции с `justify-self`/`align-self`
        // (`Spot::self_align`): правило выше строк rtl — сторону уже
        // посчитал `render::static_self_align` (при rtl `start` — доля 1,
        // та же формула, что у рукава ниже) (`align-self-static-position-
        // 001/006/008`, `justify-self-static-position-001`).
        (Some(hole), None) if now.rtl && now.block_strut => gpui::point(
            hole.origin.x + hole.size.width * now.line_align.unwrap_or(1.0)
                - bounds.size.width
                - bounds.origin.x,
            hole.origin.y - bounds.origin.y,
        ),
        // Статическая позиция по СВОБОДНОЙ строчной оси при `direction:
        // rtl` содержащего блока: §10.3.7 вешает на точку не `left`, а
        // `right`, то есть коробка стоит на ней ПРАВЫМ краем и растёт
        // назад (Blink: `static_position.h: ConvertToLogical` даёт
        // `kInlineEnd`, `absolute_utils.cc: GetStaticPositionInsetBias`
        // переводит его в `InsetBias::kEnd`).
        //
        // Гейт — набор осей, а не одно `now.rtl`. `(false, true)` ставят
        // РОВНО две ветки `atom_element` с `x_set != y_set`
        // (замещаемая и незамещаемая коробка с заданным только `top`
        // и/или `bottom`), и обе уходят `Piece::Overlay`: абзац остаётся
        // ТЕКСТОВЫМ, а дырку щупу даёт `lines.rs: point_of`. Замер по
        // снимкам всех двенадцати боевых пар
        // `abs-pos-non-replaced-v{lr,rl}-{128,129,160,161,176,177,192,
        // 193,208,209,224,225}`: зелёный квадрат стоит на x 168.0..248.0
        // при эталонных 88.0..168.0 — расхождение РОВНО в свою ширину 80
        // и ни в чём больше (вне двух квадратов разошедшихся точек 0).
        //
        // Чисто статическая коробка `(false, false)` сюда НЕ пускается
        // намеренно: её абзац уходит `Piece::Atom` в гибкий ряд
        // (`inline.rs: as_wrapped_row`), ряд при `rtl` не разворачивается,
        // и щуп садится в его конец — замер тех же снимков даёт x = 328.0
        // (правый край содержащего блока) при верных 168.0. Вычитание
        // своей ширины там сложило бы вторую ошибку с первой — это и есть
        // откат 08-19 из комментария выше. Блочный щуп во всю ширину
        // забирает рукав ВЫШЕ (`hole.size.width > 0`), поэтому семья
        // `css-position/static-position/htb-*` не задета.
        (Some(hole), None) if now.rtl && now.fixed_axes == (false, true) => gpui::point(
            hole.origin.x - bounds.size.width - bounds.origin.x,
            hole.origin.y - bounds.origin.y,
        ),
        (Some(hole), None) if now.rtl => gpui::point(
            hole.origin.x - bounds.origin.x,
            hole.origin.y - bounds.origin.y,
        ),
        // Ширину коробки прибавляет только БЛОЧНЫЙ щуп
        // (`abs-pos-border-offset-003`: ортогональный `.parent` прижат к
        // правому краю vrl-контейнера); строчный щуп 0×0 сдвига не
        // получает.
        // Прижим к ПРАВОМУ краю дырки верен только когда сторону задаёт
        // `direction: rtl` САМОГО КОНТЕЙНЕРА (`now.rtl` — направление
        // потока, а не собственное письмо коробки: css-writing-modes-4
        // §7.1, Blink `absolute_utils.cc` берёт сторону у
        // `container_writing_direction`). При `direction: ltr` статическая
        // позиция — левый край дырки без добавки (рукав ниже).
        (Some(hole), None)
            if now.own_vertical && !now.vertical && now.rtl && hole.size.width > px(0.0) =>
        {
            gpui::point(
                hole.origin.x + bounds.size.width - bounds.origin.x,
                hole.origin.y - bounds.origin.y,
            )
        }
        // Строка выровнена не по началу: статическая точка едет вдоль неё
        // на долю `line_align` (ltr вешает на неё ЛЕВЫЙ край коробки).
        (Some(hole), None) => gpui::point(
            hole.origin.x + hole.size.width * now.line_align.unwrap_or(0.0) - bounds.origin.x,
            hole.origin.y - bounds.origin.y,
        ),
        (None, _) => gpui::point(px(0.0), px(0.0)),
    }
}
