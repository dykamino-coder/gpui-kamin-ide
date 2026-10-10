//! Предикат has_own_box: получает ли позиционированный элемент собственную коробку.

use crate::style::computed::{Computed, Display};
use crate::style::values::value::Len;

/// Есть ли у инлайнового куска собственная коробка.
///
/// Прогон текста не умеет рисовать вокруг себя ничего: ни рамку, ни тень, ни
/// отступ. Раньше проверялись только верх и лево, поэтому `padding-right`,
/// боковая рамка, тень, прозрачность и три угла из четырёх у `<span>` молча
/// пропадали. У настоящего `display: inline` размеры не проверяются: CSS их
/// такому элементу и не даёт.
pub(crate) fn has_own_box(c: &Computed, font_px: f32) -> bool {
    // Нулевая величина коробки не создаёт: `padding: 0` и `border: 0` пишут
    // в стиль ноль, и по одному лишь «задано» кусок вынимался из строки —
    // а вынутый кусок рвёт соединение букв и общий перенос по словам.
    let set = |l: &Option<Len>| !matches!(l, None | Some(Len::Px(0.0)) | Some(Len::Pct(0.0)));
    let any = |s: &crate::style::computed::Sides| {
        set(&s.top) || set(&s.right) || set(&s.bottom) || set(&s.left)
    };
    // Вертикальные поля признаком коробки НЕ служат: строку они не двигают
    // (замерено на `flexbox_inline`, где `margin-top: -20em` обязан пройти
    // впустую), и по ним коробка заводилась бы только затем, чтобы уехать за
    // экран.
    // Атомарная строчная коробка — коробка по определению: у неё свои ширина,
    // высота и вертикальные поля, а прогон текста не умеет ни одного из трёх.
    //
    // ПЕРЕМЕРЕНО 28.08 и ВКЛЮЧЕНО: раньше коробкой считался только атом С
    // ЗАДАННЫМ размером, а безразмерный оставался прогоном — под флагом
    // `ATOM_BOX`, потому что прошлый замер давал flexbox 363→359 (атом-коробка
    // не отдавала строке базовую линию содержимого). Сейчас не
    // воспроизводится: CSS2 5039 → 5043, oldfront 2345 без изменений. Внутри
    // такой коробки живут отступ первой строки, сжатие по содержимому и свой
    // перенос — прогон их не знает.
    // Настоящий `display: inline` сюда НЕ входит: разбор держит его как
    // `InlineBlock` с пометкой `inline_display`, и без этой отсечки каждый
    // `<span>` становился атомарной коробкой — вместе с ней уезжали
    // сохранённые пробелы и перенос (`white-space-pre-005`).
    let atomic = (matches!(
        c.display,
        Some(Display::InlineBlock)
            | Some(Display::InlineFlex)
            | Some(Display::InlineGrid)
            | Some(Display::InlineTable)
    ) && c.inline_display != Some(true))
        || (c.display == Some(Display::GridLanes) && c.lanes_inline);
    // ПЕРЕМЕРЕНО 28.08 и ВКЛЮЧЕНО: прежний замер (flexbox 363→359 из-за
    // непрокинутой базовой линии атома) больше не воспроизводится — CSS2
    // 5039 → 5043, oldfront 2345 без изменений. Флаг `ATOM_BOX`, под которым
    // проба жила, снят.
    // Позиционированный кусок — тем же порядком: его коробку двигают края, а
    // краёв у прогона нет.
    // ПРОБОВАЛИ И ОТКАТИЛИ: считать коробкой и `position: relative`, чтобы
    // относительный `<span>` служил содержащим блоком абсолютным потомкам
    // (по CSS это так). Выигрыш нулевой во всех разделах, css-grid 389 → 386.
    // Возвращать вместе с настоящей коробкой строчного фрагмента.
    let positioned = matches!(
        c.position,
        Some(crate::style::computed::Position::Absolute)
            | Some(crate::style::computed::Position::Fixed)
    );
    if atomic || positioned {
        return true;
    }
    // Сплошной фон коробки не требует: его несёт прогон текста, и тогда
    // подсветка переносится вместе со строкой. Раньше `<span>` с фоном
    // становился отдельным блоком, и его слова вставали столбиком.
    // ПРОЗРАЧНАЯ рамка не рисует ничего, поэтому и коробки не требует: место
    // под неё держит знак-распорка строки. Пока `border: solid transparent`
    // заводил коробку, `<span>` с такой рамкой рвал абзац на части
    // (`word-space-transform-010`, где рамка ровно для того и прозрачная,
    // чтобы проверить одни только отступы).
    // РОВНУЮ рамку рисует прогон текста по кускам строк, коробка ей не нужна
    // (`inline::uniform_border`): в коробке текст перестаёт переноситься
    // вместе с абзацем, и `<span>` с рамкой уезжал одной строкой за край
    // (`hanging-punctuation-inline-bound-001`).
    // …и только у СТРОЧНОГО уровня: у блока рамка принадлежит его коробке, и
    // без неё он теряет и фон, и поля (`flexbox_first-line`: `li` с рамкой в
    // 1px разъезжался на четверть страницы).
    // ЗАМЕРЕНО И ОТКАЧЕНО: отдавать прогону ТОЛЬКО ровную рамку, а рамку с
    // разными гранями уводить в коробку. По семьям `borders/*` и `css1/*brdr*`
    // это +19/−1, но по всему CSS2 — 4917 → 4906: семьи `bidi-*` теряют 33
    // пары, потому что кусок, разорванный переносом, коробке не даётся.
    // Порог «все ЗАДАННЫЕ грани одной толщины» тоже замерен: +13/−14
    // (`border-top-width-0NN` уходят в прогон и там ложатся мимо). Настоящая
    // развилка — геометрия полосы: 1.16 кегля вместо подъёма и спуска шрифта,
    // и рамка внутрь вместо наружу. Возвращаться вместе с ней.
    // Явный `display: inline` (`div { display: inline }`) — тот же строчный
    // уровень: разбор держит его как `InlineBlock` с `inline_display`, и
    // `div` с рамкой уходил в коробку — с вертикальными полями и рамкой
    // ВНУТРИ строки (`margin-top-applies-to-008`, §10.6.1: вертикальные
    // поля строчной коробки на строку не действуют).
    // Graphical effects of the inline box (css-masking-1 §1: `clip-path` and
    // `mask` apply to all elements; css-color-4 §opacity) act on everything
    // the box paints, its border and background included. A text run applies
    // none of them, so the box is needed even when the run could paint the
    // border itself (`clip-path-inline-006`: the red border stayed unclipped).
    if c.opacity.is_some() || c.mask_image.is_some() {
        return true;
    }
    let inline_level = c.display.is_none() || c.inline_display == Some(true);
    if inline_level
        && (crate::text::inline::uniform_border(c, font_px).is_some()
            || crate::text::inline::sided_border(c, font_px).is_some())
    {
        return false;
    }
    let visible = |col: &Option<crate::style::values::value::Color>| col.is_some_and(|x| x.a > 0.0);
    let colored = c.border_color.is_some() || c.border_colors.iter().any(Option::is_some);
    let border_paints = visible(&c.border_color)
        || c.border_colors.iter().any(visible)
        // Цвет не задан вовсе — рамка красится цветом текста, то есть видна.
        || (!colored && any(&c.borders()));
    c.gradient.is_some()
        || c.bg_image.is_some()
        || border_paints
        // Отступ и поле СТРОЧНОЙ коробки рисуют пустоту: место под них
        // держит знак-распорка внутри строки (`inline_sides`). Пока они
        // заводили коробку, строка рвалась по краю `<span>` — иероглифы
        // расходились по разным строкам, а коробки выходили разной ширины
        // (`word-space-transform-010`).
        // Скругление подсветки рисует прогон вместе с её фоном.
        || (c.background.is_none()
            && (set(&c.radius.tl)
                || set(&c.radius.tr)
                || set(&c.radius.br)
                || set(&c.radius.bl)))
        || !c.shadows.is_empty()
        || c.opacity.is_some()
        // Контур на раскладку не влияет ВООБЩЕ (css-ui §2): он рисуется за
        // краем коробки и места не занимает. Строчному куску коробку он
        // поэтому не заводит — иначе `<span>` с контуром переставал
        // переноситься вместе с абзацем и уезжал одной строкой за край
        // (`text-autospace-break-001`). Рисует его прогон строки, как и
        // ровную рамку (см. `inline::uniform_border`).
        || (!inline_level && c.outline.is_some())
}
