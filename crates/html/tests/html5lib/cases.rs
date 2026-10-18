//! Parse html5lib .dat sections into inputs, contexts and expected trees.

/// Один тест набора.
#[derive(Default, Debug)]
pub(super) struct Case {
    pub(super) data: String,
    pub(super) document: String,
    /// Контекстный элемент разбора куска: `#document-fragment`.
    pub(super) fragment: Option<String>,
    /// Ограничение по режиму сценариев, если оно задано.
    pub(super) script: Option<bool>,
    /// Номер строки, с которой начался блок, — для сообщения о падении.
    pub(super) line: usize,
}

/// Разобрать `.dat`-файл на тесты.
///
/// Формат построчный, поэтому и разбор построчный: секция начинается со
/// строки `#имя`, тело идёт до следующей секции. Пустая строка внутри
/// `#data` и `#document` значима, поэтому блоки режутся по `#data`, а не по
/// пустым строкам.
pub(super) fn parse_dat(text: &str) -> Vec<Case> {
    let mut cases = vec![];
    let mut current: Option<Case> = None;
    let mut section = String::new();
    for (ix, raw) in text.lines().enumerate() {
        if raw == "#data" {
            if let Some(case) = current.take() {
                cases.push(case);
            }
            current = Some(Case {
                line: ix + 1,
                ..Default::default()
            });
            section = "data".into();
            continue;
        }
        let Some(case) = current.as_mut() else {
            continue;
        };
        if let Some(name) = raw.strip_prefix('#') {
            section = name.to_string();
            match name {
                "script-off" => case.script = Some(false),
                "script-on" => case.script = Some(true),
                "document-fragment" => case.fragment = Some(String::new()),
                _ => {}
            }
            continue;
        }
        match section.as_str() {
            "data" => {
                if !case.data.is_empty() {
                    case.data.push('\n');
                }
                case.data.push_str(raw);
            }
            "document" => {
                case.document.push_str(raw);
                case.document.push('\n');
            }
            "document-fragment" => {
                case.fragment = Some(raw.trim().to_string());
            }
            _ => {}
        }
    }
    if let Some(case) = current.take() {
        cases.push(case);
    }
    cases
}
