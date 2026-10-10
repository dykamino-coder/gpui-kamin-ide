//! Painting for vertical_text; split out to keep the owning module within 250 lines.

use super::VerticalText;
use gpui::{App, Bounds, GlobalElementId, InspectorElementId, Pixels, Window};

impl VerticalText {
    pub(crate) fn paint_impl(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _state: &mut (),
        _prepaint: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        // ЗАМЕРЕНО И ОТКАЧЕНО (01.09): статическая позиция абсолюта внутри
        // ВЕРТИКАЛЬНОЙ строки. Корень виден числом: щуп пишет дырку в
        // ДО-поворотных координатах (матрица ниже применяется только на
        // отрисовке), а стиль строки собран горизонтальным
        // (`render.rs`: `horizontal.vertical = None`), поэтому вынесенный
        // абсолют считается горизонтальным — печать дала `vert=false`,
        // коробку 80×16 вместо 16×80 и сдвиг по поперечной оси.
        // Пробовал вдвоём: (1) отображать дырку через тот же поворот,
        // (2) возвращать вынесенному абсолюту письмо. Срез из 24 пар
        // `static-position/v{lr,rl}-*` — 0/24 и до, и после, а две пары
        // (`vrl-rtl-*-in-multicol`) ушли 4.32 → «красное видно».
        // Осталось незакрытым: коробка растягивается на всю строку (16×400
        // вместо 16×80), и `display: inline` под абсолютом доходит сюда
        // блочным (`line=Some(16)` во всех шести местах документа), поэтому
        // берётся блочный рукав и смещение вдоль строки теряется.
        let scale_factor = window.scale_factor();
        let dev = |v: Pixels| v.scale(scale_factor);
        // Поворот на четверть по часовой стрелке вокруг левого верхнего угла
        // уводит содержимое влево от коробки; сдвиг на её ширину возвращает
        // его на место.
        //
        // `sideways-lr` (css-writing-modes-4, Abstract-Physical Mapping):
        // строчная ось идёт СНИЗУ ВВЕРХ, ascender смотрит ВЛЕВО — значит
        // поворот ПРОТИВ часовой. Он уводит содержимое ВВЕРХ от коробки,
        // поэтому возвращает его сдвиг на ВЫСОТУ, а не на ширину.
        // После такого поворота первая горизонтальная строка сама оказывается
        // ЛЕВОЙ колонкой, а её начало — у нижнего края: подача строк снизу
        // вверх (`lines_reversed`) больше не нужна, см. `render.rs`.
        let (shift, angle) = if self.ccw {
            (
                gpui::point(
                    dev(bounds.origin.x),
                    dev(bounds.origin.y + bounds.size.height),
                ),
                -std::f32::consts::FRAC_PI_2,
            )
        } else {
            (
                gpui::point(
                    dev(bounds.origin.x + bounds.size.width),
                    dev(bounds.origin.y),
                ),
                std::f32::consts::FRAC_PI_2,
            )
        };
        let matrix = gpui::TransformationMatrix::unit()
            .translate(shift)
            .rotate(gpui::Radians(angle))
            .translate(gpui::point(dev(-bounds.origin.x), dev(-bounds.origin.y)));
        let child = self.child.as_mut().unwrap();
        // Свой слой с ПОСЛЕ-поворотными границами: порядок отрисовки сцена
        // считает по границам примитивов, а у повёрнутого текста они
        // ДО-трансформные — лежат вне своей коробки, перекрытие с фоном не
        // видно, и градиент соседа красился ПОВЕРХ глифа
        // (table-cell-align-005: с третьей ячейки текст пропадал под фоном).
        window.paint_layer(bounds, |window| {
            window.with_transformation(matrix, |window| child.paint(window, cx));
        });
    }
}
