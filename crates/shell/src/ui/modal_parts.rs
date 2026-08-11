//! Части модалки: текст тела и кнопки диалога.
//!
//! Перенесено без изменения поведения (`plan/100-refactor-250.md`).

use crate::colors::rgba;
use gpui::prelude::*;
use gpui::{AnyElement, SharedString, div, px};
use kamin_metrics as m;
use kamin_theme::Palette;

/// Инлайновая разметка тела модалки: `<b>`/`<strong>`, `<code>`, `<br>` —
/// ровно то, чем пользуется оригинал (`bodyHtml` + санитайз).
///
/// Абзац — ОДИН `StyledText` с пер-ранными стилями, а не набор div-слов во
/// `flex-wrap`: слова-дети рвали шейпинг, и пробелы между кусками разной
/// разметки гуляли на 2-3 px (ревью ц.24).
pub(crate) fn body_text(body: &str, p: &Palette, base: gpui::Font) -> AnyElement {
    #[derive(Clone, Copy, PartialEq)]
    enum Kind {
        Plain,
        Bold,
        Code,
    }
    let mut text = String::with_capacity(body.len());
    let mut spans: Vec<(usize, Kind)> = Vec::new();
    let push = |text: &mut String, spans: &mut Vec<(usize, Kind)>, s: &str, k: Kind| {
        if s.is_empty() {
            return;
        }
        text.push_str(s);
        match spans.last_mut() {
            Some((len, last)) if *last == k => *len += s.len(),
            _ => spans.push((s.len(), k)),
        }
    };
    let mut rest = body;
    let mut kind = Kind::Plain;
    while !rest.is_empty() {
        let Some(lt) = rest.find('<') else {
            push(&mut text, &mut spans, rest, kind);
            break;
        };
        push(&mut text, &mut spans, &rest[..lt], kind);
        let Some(gt) = rest[lt..].find('>') else {
            push(&mut text, &mut spans, &rest[lt..], kind);
            break;
        };
        let tag = rest[lt + 1..lt + gt].trim().to_ascii_lowercase();
        match tag.as_str() {
            "b" | "strong" => kind = Kind::Bold,
            "/b" | "/strong" => kind = Kind::Plain,
            "code" => kind = Kind::Code,
            "/code" => kind = Kind::Plain,
            // `<br>` — жёсткий перенос внутри того же абзаца
            "br" | "br/" | "br /" => push(&mut text, &mut spans, "\n", kind),
            // Неизвестный тег отдаём как текст: санитайз оригинала их не
            // выбрасывает, а показывает
            other => {
                let raw = format!("<{other}>");
                push(&mut text, &mut spans, &raw, kind);
            }
        }
        rest = &rest[lt + gt + 1..];
    }

    let mut code_font = base.clone();
    // У `<code>` нет CSS-правила ни в модалке, ни в глобальных, значит
    // рисуется ШРИФТОМ ПО УМОЛЧАНИЮ движка для monospace — на Windows это
    // Consolas, а не JetBrains Mono нашей темы (ревью ц.35)
    code_font.family = "Consolas".into();
    let mut bold_font = base.clone();
    bold_font.weight = gpui::FontWeight::SEMIBOLD;
    let runs: Vec<gpui::TextRun> = spans
        .into_iter()
        .map(|(len, k)| gpui::TextRun {
            len,
            font: match k {
                Kind::Bold => bold_font.clone(),
                Kind::Code => code_font.clone(),
                Kind::Plain => base.clone(),
            },
            color: match k {
                // `<code>` своего цвета в оригинале НЕ получает: правила
                // `code` нет ни в `ConfirmModal.module.css`, ни в глобальных —
                // работает только UA-дефолт (моноширинное семейство)
                Kind::Bold => rgba(p.text_primary).into(),
                _ => rgba(p.text_secondary).into(),
            },
            background_color: None,
            // Поля строчного бокса вокруг фона прогона: здесь их нет.
            font_size: None,
            background_pad: Default::default(),
            background_radius: Default::default(),
            background_border: None,
            underline: None,
            strikethrough: None,
        })
        .collect();
    gpui::StyledText::new(SharedString::from(text))
        .with_runs(runs)
        .into_any_element()
}
pub(crate) fn dialog_button(
    id: &'static str,
    label: impl Into<SharedString>,
    _filled: bool,
    p: &Palette,
    on_click: impl Fn(&mut gpui::Window, &mut gpui::App) + 'static,
) -> AnyElement {
    let hover_bg = rgba(p.bg_surface);
    let b = div()
        .id(id)
        .px(px(m::SPACE_4))
        .py(px(m::SPACE_1))
        .rounded(px(m::RADIUS_SM))
        .border_1()
        .border_color(rgba(p.bg_overlay))
        .text_size(px(m::FS_SM))
        .text_color(rgba(p.text_primary))
        .cursor_pointer()
        .hover(move |s| s.bg(hover_bg))
        .on_mouse_down(gpui::MouseButton::Left, move |_, w, cx| on_click(w, cx))
        .child(label.into());
    // Кнопки диалога — `<button>` оригинала: таб-стопы + `:focus-visible`
    crate::ui::focus_ring::focusable(b, id, m::RADIUS_SM, rgba(p.accent_primary)).into_any_element()
}
#[allow(clippy::too_many_arguments)]
pub(crate) fn dialog_button_bg(
    id: &'static str,
    label: impl Into<SharedString>,
    bg: gpui::Rgba,
    hover_bg: gpui::Rgba,
    fg: gpui::Rgba,
    // `disabled`: opacity .5, клик не проходит (`PromptModal.module.css:89-92`)
    disabled: bool,
    // Цвет кольца `:focus-visible` — accent-primary темы
    accent: gpui::Rgba,
    on_click: impl Fn(&mut gpui::Window, &mut gpui::App) + 'static,
) -> AnyElement {
    let b = div()
        .id(id)
        .px(px(m::SPACE_4))
        .py(px(m::SPACE_1))
        .rounded(px(m::RADIUS_SM))
        .bg(bg)
        .text_size(px(m::FS_SM))
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .text_color(fg)
        .when(!disabled, |b| {
            b.cursor_pointer()
                .hover(move |s| s.bg(hover_bg))
                .on_mouse_down(gpui::MouseButton::Left, move |_, w, cx| on_click(w, cx))
        })
        // `:disabled { opacity: .5; cursor: not-allowed }` — курсора
        // «not-allowed» в gpui нет, остаётся гашение и отсутствие реакции
        .when(disabled, |b| b.opacity(0.5))
        .child(label.into());
    crate::ui::focus_ring::focusable(b, id, m::RADIUS_SM, accent).into_any_element()
}
