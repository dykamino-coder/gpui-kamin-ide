//! Адресация `text-box-trim` к абзацу, который держит первую/последнюю
//! отформатированную строку срезающего контейнера.
//!
//! Срез строки делится на две части (css-inline-3 §text-box-trim; Blink
//! `InlineLayoutAlgorithm::ApplyTextBoxTrim`): от верха строчной коробки до
//! струта — её знает только замер абзаца (высокий кусок, атом, руби), — и от
//! струта до метрики края корневой строчной коробки — её считает по стилю
//! хвост `blocks_flow` и кладёт отрицательным полем. Первую часть абзац
//! снимает сам (`Paragraph::line_padding`), но ему надо знать, что именно
//! он держит срезаемую строку: строка может лежать в потомке контейнера
//! (css-pseudo-4 «first formatted line»). Контейнер адресует запрос
//! узлу-владельцу строки (`TrimRequest`) и передаёт его своим детям
//! (`InlineContext::trim_requests`), `blocks_flow` владельца забирает свои
//! запросы (`take`) и собирает нужный абзац с флагами
//! (`InlineContext::trim_para`), а сборщик абзаца читает их по своему узлу
//! (`para`).

/// Флаги среза абзаца: начало, конец и предел надбавки первой строки над
/// струтом блока (`over`): корневая строчная коробка первой строки берёт
/// стиль `::first-line`, и её край стоит на `over` выше струта блока.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Trim {
    pub start: bool,
    pub end: bool,
    pub over: f32,
}

/// A trimming container's request to the block `owner` that holds its first
/// (`start`) or last formatted line; `over` — see `Trim::over`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TrimRequest {
    pub owner: u64,
    pub start: bool,
    pub over: f32,
}

/// The requests addressed to block `owner`.
pub(crate) fn take(requests: &[TrimRequest], owner: u64) -> Trim {
    if owner == 0 {
        return Trim::default();
    }
    let start = requests.iter().find(|q| q.owner == owner && q.start);
    Trim {
        start: start.is_some(),
        end: requests.iter().any(|q| q.owner == owner && !q.start),
        over: start.map_or(0.0, |q| q.over),
    }
}

/// Trim flags for a paragraph of block `owner`, given the paragraph scope
/// `(block, flags)` it is built in (`InlineContext::trim_para`; the block is
/// checked: nested atoms build their own paragraphs inside the same call).
/// `None` — the paragraph is not built by its block's `blocks_flow` loop:
/// its own style decides.
pub(crate) fn para(scope: Option<(u64, Trim)>, owner: u64) -> Option<Trim> {
    scope
        .filter(|(o, _)| *o == owner && owner != 0)
        .map(|(_, t)| t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn take_and_para_match_the_owner() {
        let requests = [
            TrimRequest {
                owner: 7,
                start: true,
                over: 3.0,
            },
            TrimRequest {
                owner: 7,
                start: false,
                over: 0.0,
            },
            TrimRequest {
                owner: 9,
                start: true,
                over: 0.0,
            },
        ];
        let t = take(&requests, 7);
        assert!(t.start && t.end && t.over == 3.0);
        assert_eq!(take(&requests, 8), Trim::default());
        assert_eq!(take(&requests, 0), Trim::default());
        let on = Trim {
            start: true,
            end: false,
            over: 0.0,
        };
        assert_eq!(para(Some((7, on)), 7), Some(on));
        assert_eq!(para(Some((7, on)), 8), None);
        assert_eq!(para(None, 7), None);
    }
}
