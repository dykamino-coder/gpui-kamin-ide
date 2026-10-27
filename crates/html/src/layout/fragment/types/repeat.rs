//! Повторяемые шапка и подвал таблицы во фрагментах (css-tables-3 §repeated-headers): полосы
//! копий (`Repeat`), их геометрия для укладки (`RepeatGeom`), допуск полос в конкретном
//! фрагментаинере и место подвала у фрагмента.

use super::Frag;
use crate::tolerance::COORD_EPS;
use gpui::AnyElement;

/// Повтор секций таблицы во фрагментах. Фрагмент в нашей модели — СРЕЗ одной
/// нарисованной копии `[from, from + h)`, а повторённой шапки в срезе нет:
/// она рисуется ОТДЕЛЬНОЙ копией той же таблицы, поднятой так, что полоса
/// шапки встаёт на верх фрагмента, и режется маской по этой полосе. Тело
/// фрагмента-продолжения уезжает вниз на высоту полосы (`Frag::head`), у
/// непоследнего фрагмента снизу оставлено место под подвал (`Frag::foot`) —
/// это Blink `reserved_space = repeated_header_block_size +
/// repeated_footer_block_size` (`:1323-1327`).
pub struct Repeat {
    /// Полоса шапки в координатах коробки: `(верх, высота)`; высота — шапка
    /// плюс `border-spacing` под ней (Blink `repeated_header_block_size`,
    /// `:1393-1395`: продолжение шапку кладёт без зазора над ней, `:942`).
    pub head: Option<(f32, f32)>,
    /// Полоса подвала: `(верх, высота)`; высота — подвал плюс зазор над ним
    /// (Blink `repeated_footer_block_size`, `:1147-1149`).
    pub foot: Option<(f32, f32)>,
    /// Где полосам место (`RepeatGeom`).
    pub geom: RepeatGeom,
    /// Концы рядов разложенной сетки в координатах коробки: фрагмент,
    /// кончившийся на одном из них, разорван между рядами (`foot_top`).
    pub(crate) row_ends: Vec<f32>,
    /// Копии для полос: шапка — фрагменту `c ≥ 1` (`head_els[c - 1]`),
    /// подвал — НЕпоследнему фрагменту `c` (`foot_els[c]`).
    pub head_els: Vec<AnyElement>,
    pub foot_els: Vec<AnyElement>,
}

impl Repeat {
    /// Верх полосы подвала у фрагмента `f` от верха его содержимого. Фрагмент,
    /// кончившийся на границе рядов, ставит подвал сразу под последним рядом,
    /// а зазор — после подвала (css-tables-3 §breaking-rules: «the footer must
    /// come directly after the last row in the fragmentainer and the vertical
    /// gap must be inserted after the repeated footer»). Разорванный ряд
    /// тянется до края фрагментаинера (css-break-3 §box-splitting), и подвал
    /// стоит у низа. `None` — у фрагмента полосы подвала нет.
    pub(crate) fn foot_top(&self, f: &Frag) -> Option<f32> {
        let (_, band) = self.foot.filter(|_| f.foot > COORD_EPS)?;
        let end = f.from + f.h;
        Some(
            if self.row_ends.iter().any(|&e| (e - end).abs() <= COORD_EPS) {
                f.h
            } else {
                f.h + f.foot - band
            },
        )
    }
}

/// Геометрия повтора для укладки (`fill_at`), в координатах коробки. Нули —
/// повтора нет, укладка тождественна прежней.
#[derive(Clone, Copy, Debug, Default)]
pub struct RepeatGeom {
    /// Место полосы шапки сверху у продолжения и полосы подвала снизу.
    pub head: f32,
    pub foot: f32,
    /// Конец полосы шапки на её родном месте: продолжение, начатое раньше,
    /// шапку ещё не прошло.
    pub head_end: f32,
    /// Точка разрыва перед подвалом на родном месте и конец подвала: фрагмент,
    /// куда родной подвал влезает целиком, полосы не берёт — подвал в нём
    /// последний (Blink `has_pending_repeated_footer = false`, `:1315`).
    pub foot_at: f32,
    pub foot_end: f32,
    /// Коробка рядов (без подписей): повтор — только во фрагментах, где она
    /// есть (Blink: «If this isn't the first fragment for the table box …»,
    /// `:1108-1114`; подписи о повторе не знают, `:1248-1249`).
    pub box_top: f32,
    pub box_end: f32,
    /// Наименьший блочный размер страницы, на которой секция шапки и подвала
    /// не больше её четверти (css-tables-3 §repeated-headers: «up to one
    /// quarter for header rows, and up to one quarter for footer rows»).
    pub head_need: f32,
    pub foot_need: f32,
}

impl RepeatGeom {
    /// Полосы, допустимые во фрагментаинере блочного размера `target`. На
    /// странице (`paged`) правило четверти действует для каждой страницы,
    /// которую занимает таблица (css-tables-3 §repeated-headers: «on each page
    /// spanned by a table … if the height required to do so is inferior to two
    /// quarters of the page height»). В любом фрагментаинере полосы вместе
    /// оставляют телу таблицы место: фрагмент из одних повторов не продвигал
    /// бы укладку (css-break-4 §unforced-breaks: хоть что-то из содержимого в
    /// каждом фрагментаинере), и последовательность фрагментов не кончалась бы.
    pub(crate) fn admit(self, target: f32, paged: bool) -> Self {
        let mut g = self;
        if paged && g.head_need > target + COORD_EPS {
            g.head = 0.0;
        }
        if paged && g.foot_need > target + COORD_EPS {
            g.foot = 0.0;
        }
        if g.head + g.foot >= target - COORD_EPS {
            g.head = 0.0;
            g.foot = 0.0;
        }
        g
    }

    /// Полоса шапки у фрагмента-продолжения, начатого с `from`.
    pub(crate) fn head_at(&self, from: f32) -> f32 {
        if self.head > 0.0 && from >= self.head_end - COORD_EPS && from < self.box_end - COORD_EPS {
            self.head
        } else {
            0.0
        }
    }

    /// Место под подвал у фрагмента с `from`, которому доступно `room`.
    pub(crate) fn foot_for(&self, from: f32, room: f32) -> f32 {
        if self.foot > 0.0
            && from < self.foot_at - COORD_EPS
            && from + room > self.box_top + COORD_EPS
            && from + room < self.foot_end - COORD_EPS
        {
            self.foot
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod tests;
