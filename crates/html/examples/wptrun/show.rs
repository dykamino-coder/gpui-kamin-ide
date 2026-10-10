//! Show one document in the runner window and poll until its own frame has settled.

use super::capture::capture;
use super::capture_dump::Shot;
use super::frame_metrics::flat;
use super::page::Page;
use super::reftest_meta::reftest_pages;
use gpui::{AnyWindowHandle, AppContext as _, AsyncApp};
use kamin_html::{BROWSER_CSS, Document};
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

/// Окно стенда и след опроса кадров.
pub(super) struct Stage {
    pub(super) window: AnyWindowHandle,
    /// Указатель окна нужен снимку: рисует его система, а не мы.
    pub(super) hwnd: isize,
    /// Чем кончился опрос кадра у каждого показа пары: `s` — кадр
    /// сменился и устоялся, `b` — страница осталась равной разделителю,
    /// `o` — запас опроса вышел, кадр есть, `n` — окно за весь запас не
    /// отдало НИ ОДНОГО кадра. Без этого «пара идёт 27 секунд» и «пара
    /// пустая» — два разных наблюдения без связи между ними.
    pub(super) trace: Rc<RefCell<String>>,
}

impl Stage {
    /// Показать страницу и дождаться ЕЁ кадра.
    ///
    /// Снимок делает система, а рисует окно по своему расписанию:
    /// сна на глазок не хватает, и тяжёлая страница отдаёт кадр
    /// ПРЕДЫДУЩЕЙ. По числам это выглядит как совпадение (ноль
    /// расхождения) или как случайные скачки между прогонами.
    /// Поэтому кадр опрашивается, пока не сменится и не устоится.
    pub(super) async fn show(
        &self,
        cx: &mut AsyncApp,
        html: String,
        prev: Option<Vec<u8>>,
        want_flat: bool,
    ) -> Option<Shot> {
        let (window, hwnd, note) = (self.window, self.hwnd, &self.trace);
        let _ = cx.update_window(window, |view, window, cx| {
            if let Ok(page) = view.downcast::<Page>() {
                page.update(cx, |page, cx| {
                    page.doc = Rc::new(Document::new(&html, BROWSER_CSS));
                    page.select = reftest_pages(&html);
                    cx.notify();
                });
            }
            window.refresh();
        });
        let lower = html.to_ascii_lowercase();
        let has_image = lower.contains("<img");
        // Страница с ДВИЖЕНИЕМ устаивается ложно: медленная анимация
        // даёт два одинаковых кадра подряд просто потому, что за 16 мс
        // картинка не изменилась на целую точку. Такой странице нужен
        // куда более длинный признак покоя — иначе одна и та же пара
        // скачет между прогонами (`css-flexbox-height-animation-stretch`:
        // то 1.13 %, то 0.03 %).
        let has_anim = lower.contains("animation") || lower.contains("transition");
        let mut last: Option<(u32, u32, Vec<u8>)> = None;
        let mut same = 0u32;
        let mut steady = 0u32;
        // Запас опроса: тяжёлой странице нужно время устояться, а лишнего
        // ожидания на лёгких не будет — цикл выходит по первому же
        // устоявшемуся кадру. Прежние 120 шагов (около двух секунд)
        // такие страницы обрезали, и одна и та же пара скакала между
        // прогонами на 15% расхождения.
        for step in 0..400u32 {
            cx.background_executor()
                .timer(Duration::from_millis(16))
                .await;
            // Окно ИНОГДА не перерисовывается после подмены документа:
            // экран продолжает показывать разделитель, и страница
            // числится пустой (`column-auto-repeat-auto-001`: эталон
            // одиночно рисуется, в паре — нет; след `sssb`).
            // Напоминание раз в полсекунды выводит его из этого
            // состояния; на здоровых показах до него не доходит —
            // кадр устаивается раньше.
            if step % 30 == 29 {
                let _ = cx.update_window(window, |view, window, cx| {
                    if let Ok(page) = view.downcast::<Page>() {
                        page.update(cx, |_, cx| cx.notify());
                    }
                    window.refresh();
                });
            }
            let shot = capture(hwnd);
            let Some(now) = shot else { continue };
            let changed = prev.as_ref().is_none_or(|before| *before != now.2);
            let settled = last.as_ref().is_some_and(|before| before.2 == now.2);
            // ПРОБОВАЛИ И ОТКАТИЛИ: требовать ТРИ одинаковых кадра
            // подряд вместо двух — ради устойчивости чисел. Вышло
            // наоборот: тяжёлая страница не успевает набрать три за
            // отведённый опрос, снимок берётся последним и уходит
            // недорисованным (css-grid 386 → 363, и разброс между
            // двумя одинаковыми прогонами вырос до 18 пар). Лечится
            // не строгостью признака, а ЗАПАСОМ ВРЕМЕНИ.
            // Картинка приходит ПОЗЖЕ первого устоявшегося кадра:
            // файл читается и раскодируется вне кадра, и страница
            // успевает «устояться» без неё. На странице с `<img>`
            // ждём подряд ЧЕТЫРЕ одинаковых кадра вместо двух —
            // иначе одна и та же пара скачет между прогонами
            // (`wm-propagation-body-032`: то 1.01 %, то 2.90 %, и
            // картинки то есть, то нет).
            let need = if has_anim {
                30
            } else if has_image {
                16
            } else {
                4
            };
            steady = if settled { steady + 1 } else { 0 };
            // Разделителю мало «устоялся»: он обязан быть РОВНЫМ.
            // Иначе на экране остаётся кадр прошлой страницы, он и
            // уходит в `blank`, а дальше от него считаются и «сменился»,
            // и чернила — то есть следующая страница выглядит пустой
            // (`css-position/multicol/static-position/*`).
            if changed && steady + 1 >= need && (!want_flat || flat(&now.2)) {
                note.borrow_mut().push('s');
                return Some(now);
            }
            // Страница, которая НЕ нарисовалась, равна разделителю, и
            // «сменился» на ней не наступает никогда — стенд вырабатывал
            // весь запас опроса. На экране это долгая белизна, в
            // числах — по 12 секунд на пару. Устоявшийся неизменный
            // кадр принимается как ответ: он и есть итог, просто пустой.
            // Ранний выход — ТОЛЬКО для кадра, равного разделителю:
            // это и есть «страница не нарисовалась», ждать больше
            // нечего. Принимать по устойчивости любой кадр нельзя —
            // тяжёлая страница успевает застыть на полпути, и в
            // сравнение уходит недорисованное (замерено: css-position
            // просел 49 → 22 на одних таблицах).
            let blank_now = prev.as_ref().is_some_and(|before| *before == now.2);
            same = if settled && blank_now { same + 1 } else { 0 };
            // Порог намеренно высокий (около двух секунд): тяжёлая
            // страница даёт первый кадр далеко не сразу, и прежние
            // восемь шагов (130 мс) объявляли пустой любую такую
            // страницу. Ценой полутора секунд на действительно пустой
            // странице покупается верный вердикт на тяжёлой
            // (`css-position/multicol/static-position/*`: все 16 пар
            // выходили пустыми, а по одной рисуются).
            if same >= 120 {
                note.borrow_mut().push('b');
                return Some(now);
            }
            last = Some(now);
        }
        note.borrow_mut()
            .push(if last.is_some() { 'o' } else { 'n' });
        last
    }
}
