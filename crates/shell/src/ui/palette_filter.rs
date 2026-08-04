//! Фильтр палитры: гейт when и отбор по запросу.
//!
//! Перенесено без изменения поведения (`plan/100-refactor-250.md`).

pub(crate) const PALETTE_W: f32 = 640.0;
pub(crate) const PALETTE_TOP: f32 = 84.0;
pub(crate) const MAX_ROWS: usize = 50;
/// Команда реестра (id/title/category) для палитры.
#[derive(Clone)]
pub struct CommandItem {
    pub id: String,
    pub title: String,
    pub category: Option<String>,
}
/// Гейт видимости из `contributes.menus.commandPalette` (`state.ts:68-76`):
/// команда С записями в палитре показывается, только если у ОДНОЙ из них
/// `when` истинен (пустой `when` = всегда); команды без записи видимы по
/// умолчанию.
pub fn palette_gate(
    entries: &[(String, String)],
    ctx: &crate::when::ContextValues,
) -> std::collections::HashMap<String, bool> {
    let mut gate: std::collections::HashMap<String, bool> = std::collections::HashMap::new();
    for (command, when) in entries {
        if command.is_empty() {
            continue;
        }
        let ok = crate::when::evaluate_when(when, ctx);
        let prev = gate.get(command).copied().unwrap_or(false);
        gate.insert(command.clone(), prev || ok);
    }
    gate
}
/// Отфильтровать по подстроке (title/id/category), выкинуть внутренние (`_`)
/// и скрытые гейтом палитры.
pub fn filter_gated(
    commands: &[CommandItem],
    query: &str,
    gate: &std::collections::HashMap<String, bool>,
) -> Vec<CommandItem> {
    filter(commands, query)
        .into_iter()
        .filter(|c| gate.get(&c.id).copied().unwrap_or(true))
        .collect()
}
/// Отфильтровать по подстроке (title/id/category), выкинуть внутренние (`_`).
pub fn filter(commands: &[CommandItem], query: &str) -> Vec<CommandItem> {
    let q = query.trim().to_lowercase();
    commands
        .iter()
        // `isInternalCommand` (`state.ts:52-56`): множество `INTERNAL_COMMANDS`
        // (сейчас в нём `setContext`) плюс соглашение о префиксе `_`.
        // Своего фильтра по пустому title у оригинала НЕТ — команда без
        // заголовка рисуется пустым ряом с одним id (ревью ц.26)
        .filter(|c| !c.id.starts_with('_') && c.id != "setContext")
        .filter(|c| {
            q.is_empty()
                || c.title.to_lowercase().contains(&q)
                || c.id.to_lowercase().contains(&q)
                || c.category
                    .as_deref()
                    .unwrap_or("")
                    .to_lowercase()
                    .contains(&q)
        })
        .cloned()
        .collect()
}
