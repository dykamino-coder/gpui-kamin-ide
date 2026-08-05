//! Живая проверка: окно, в котором документ нарисован элементами GPUI.
//!
//! Пример заодно показывает ПРАВИЛЬНОЕ использование, а не только результат:
//!
//! * документ разбирается ОДИН раз и живёт между кадрами (`Document`);
//! * блоки рисуются через список GPUI — раскладывается только видимое;
//! * базовый стиль текста берётся уже с нужным цветом, иначе прогоны красятся
//!   цветом по умолчанию (на это легко наступить: цвет корня применяется
//!   позже, чем снимается стиль).
//!
//!     cargo run -p kamin-html --example demo

use gpui::{
    AppContext as _, Application, Bounds, Context, Entity, InteractiveElement, IntoElement,
    ParentElement, Render, StatefulInteractiveElement, Styled, Window, WindowBounds, WindowOptions,
    div, px, rgb, size,
};
use kamin_html::{Document, RenderOpts, render_block};
use std::rc::Rc;

const INK: u32 = 0xe6e6ee;

const DOC: &str = r##"
<style>
  .card    { background: #2a2b36; border: 1px solid #3d3f51; border-radius: 8px;
             padding: 14px 16px; margin: 0 0 12px }
  .title   { font-size: 15px; font-weight: 700; color: #ffffff; margin: 0 0 8px }
  .row     { display: flex; gap: 10px; align-items: center }
  .chip    { background: #3b5bdb; color: #ffffff; border-radius: 12px;
             padding: 3px 10px; font-size: 12px }
  .chip:hover { background: #5c7cfa }
  .muted   { color: #9aa0b4; font-size: 12px }
  .grad    { background: linear-gradient(90deg, #3b5bdb, #2b8a3e);
             border-radius: 6px; padding: 10px; color: #ffffff }
</style>

<div class="card">
  <p class="title">Бокс, флекс и текст</p>
  <div class="row">
    <span class="chip">наведи на меня</span>
    <span class="muted">рядом — приглушённый текст</span>
  </div>
</div>

<div class="card">
  <p class="title">Инлайн-поток</p>
  <p>Обычный текст, <b>жирный</b>, <i>курсив</i>, <a href="#">ссылка</a> и
  <code>инлайн-код</code> — всё в одном абзаце, с переносом по словам.</p>
</div>

<div class="card">
  <p class="title">Список и таблица</p>
  <ul>
    <li>Первый пункт</li>
    <li>Второй пункт, который заметно длиннее первого и должен перенестись</li>
  </ul>
  <table>
    <tr><th>Дата</th><th>Статус</th><th>Описание</th></tr>
    <tr><td>05.08</td><td>готово</td><td>Колонки шириной по содержимому, последняя тянется</td></tr>
    <tr><td>06.08</td><td>в работе</td><td>Короткая строка</td></tr>
  </table>
</div>

<div class="card">
  <p class="title">Векторная графика</p>
  <svg viewBox="0 0 220 60" xmlns="http://www.w3.org/2000/svg">
    <rect x="0"   y="20" width="30" height="40" fill="#3b5bdb"/>
    <rect x="40"  y="8"  width="30" height="52" fill="#2b8a3e"/>
    <rect x="80"  y="30" width="30" height="30" fill="#b8860b"/>
    <rect x="120" y="2"  width="30" height="58" fill="#c92a2a"/>
    <circle cx="185" cy="30" r="26" fill="none" stroke="#8ab4f8" stroke-width="6"/>
  </svg>
</div>

<div class="card">
  <p class="title">Градиент, тень, скругление</p>
  <div class="grad" style="box-shadow: 0 4px 14px rgba(0,0,0,.5)">
    Заливка переходом цветов с тенью под блоком
  </div>
</div>

<div class="card">
  <p class="title">Блок кода</p>
  <pre>fn main() {
    let doc = Document::new(html, theme);
    render_block(doc.nodes(), i, &amp;opts);
}</pre>
</div>

<div class="card">
  <p class="title">Прокрутка</p>
  <p class="muted">Ниже — намеренно длинный хвост, чтобы проверить и полосу
  прокрутки, и то, что невидимые блоки не стоят ничего.</p>
</div>
"##;

struct Demo {
    /// Разобран ОДИН раз: на кадре только отрисовка.
    doc: Rc<Document>,
}

impl Demo {
    fn new(_cx: &mut Context<Self>) -> Self {
        // Хвост из повторов — чтобы документ заведомо не помещался в окно.
        let mut html = String::from(DOC);
        for i in 1..=40 {
            html.push_str(&format!(
                r#"<div class="card"><p class="title">Блок {i}</p>
                   <p class="muted">Строка внутри блока номер {i}.</p></div>"#
            ));
        }
        Demo {
            doc: Rc::new(Document::new(&html, "")),
        }
    }
}

impl Render for Demo {
    fn render(&mut self, window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        // Стиль текста снимается с цветом корня — иначе прогоны красятся
        // цветом по умолчанию (тёмным на тёмном фоне).
        let mut text = window.text_style();
        text.color = rgb(INK).into();
        let opts = RenderOpts {
            text: text.clone(),
            table_min_col: 80.,
        };
        div()
            .size_full()
            .bg(rgb(0x1e1f29))
            .text_color(rgb(INK))
            .text_size(px(13.))
            .p(px(18.))
            .flex()
            .flex_col()
            .child(
                div()
                    .id("doc")
                    .flex_1()
                    .min_h(px(0.))
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .children((0..self.doc.top_level_blocks()).filter_map(|ix| {
                        // Блоки собираются по одному — той же функцией, которой
                        // их берёт виртуализованный список в реальном приложении.
                        render_block(self.doc.nodes(), ix, &opts)
                    })),
            )
            .child(
                div()
                    .pt(px(8.))
                    .text_size(px(11.))
                    .text_color(rgb(0x9aa0b4))
                    .child(format!(
                        "блоков верхнего уровня: {} · узлов в документе: {}",
                        self.doc.top_level_blocks(),
                        self.doc.node_count()
                    )),
            )
    }
}

fn main() {
    Application::new().run(|cx| {
        let bounds = Bounds::centered(None, size(px(760.), px(900.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |_, cx| -> Entity<Demo> { cx.new(Demo::new) },
        )
        .unwrap();
        cx.activate(true);
    });
}
