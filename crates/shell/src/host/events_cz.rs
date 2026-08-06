//! Вариантты `ShellEvent` домена `events_cz`
//! (`plan/100-refactor-250.md`).

use super::events::*;
use serde_json::Value;

#[derive(Clone)]
pub enum CzEvent {
    /// Свернуть/развернуть узел contributed-страниц Customize.
    /// Свернуть/развернуть contributed-контейнер Customize ПО ЕГО id: у
    /// оригинала состояние живёт внутри узла (`useState` на контейнер), а один
    /// общий флаг сворачивал все контейнеры разом (ревью ц.13).
    ToggleCustomizeContribGroup(String),
    /// Customize-режим (gear): вкл/выкл + выбор подпанели.
    ToggleCustomize,
    SetCustomizePanel(&'static str),
    /// Открыть Customize НА разделе (без переключения туда-обратно):
    /// `SetCustomizePanel` сам режим не включает.
    OpenCustomizePanel(&'static str),
    /// Прилетели app-prefs хоста:
    /// (backgroundToasts, useConptyDll, skipDeleteConfirm).
    PrefsLoaded(bool, bool, bool),
    /// Список расширений хоста (kamin:extensions:list).
    ExtensionsLoaded(Vec<crate::ui::extensions_panel::ExtDesc>),
    /// Вкл/выкл расширение (RPC + перезагрузка списка).
    ToggleExtension(String, bool),
    /// Problems: тумблер severity-фильтра (0 err / 1 warn).
    ToggleProblemsFilter(u8),
    /// Problems: свернуть/развернуть файл-группу.
    ToggleProblemsFile(String),
    /// Problems: показать ещё файлов (+step 200).
    ProblemsShowMore,
    /// Удалить sideloaded-расширение (kamin:extensions:uninstall).
    UninstallExtension(String),
    /// kamin:output:event: (extensionId, channel, op, text).
    OutputEvent(String, String, String, Option<String>),
    /// Очистить буфер output-канала (локально).
    ClearOutputChannel(String),
    /// Выбрать активный output-канал.
    SelectOutputChannel(String),
    /// Запись system-лога: (level, source, message).
    SystemLog(&'static str, String, String),
    /// Очистить system-лог.
    ClearSystemLog,
    /// Клик по бренду в статус-баре — проверить обновления.
    CheckForUpdates,
    /// Человекочитаемый статус загрузки списка расширений.
    ExtensionsStatus(String),
    /// Фильтр уровня System-лога: all|error|warning|info.
    SetSysLogLevel(&'static str),
    /// Contributed statusbar: снапшот (массив) / upsert одного / удаление.
    StatusBarSnapshot(Value),
    StatusBarUpsert(Value),
    StatusBarRemove(String),
    /// Problems: полный снапшот [{owner,uri,diagnostics}] / дельта одного uri.
    DiagSnapshot(Value),
    DiagSet(Value),
    /// Contributed explorer/context пункты (из registry:snapshot).
    ExplorerMenuItems(Vec<crate::ui::file_menu::ContribMenuItem>),
    /// Contributed темы из registry: (id, label, path, dark_ui).
    ThemesList(Vec<(String, String, String, bool)>),
    /// Применить contributed-тему: (id, путь к JSON, dark_ui).
    SetContributedTheme(String, String, bool),
    /// Contributed keybindings: (key, command, when) из registry.
    KeybindingsList(Vec<(String, String, String)>),
    /// Contributed iconThemes из registry: (id, label, path).
    IconThemesList(Vec<(String, String, String)>),
    /// Выбор icon-темы: None = builtin Catppuccin.
    SetIconTheme(Option<(String, String)>),
    /// Тема загружена и распарсена (id) — активировать.
    IconThemeLoaded(String, Box<crate::icon_theme::IconTheme>),
    /// Апдейтер: найдено обновление (version, url инсталлера).
    UpdateAvailable(String, String),
    /// Extensions-панель: открыть пикер VSIX для установки.
    InstallVsixPrompt,
    /// kamin:index:status — хост (пере)индексирует воркспейс.
    IndexStatus(bool),
    /// Contributed Customize-страницы из registry: (view_id, name, icon).
    CustomizePages(Vec<CzContainer>),
    /// Клик по contributed-странице Customize: открыть её вебвью.
    SetCustomizeContribPage(String),
    /// Тумблер настройки: ключ app-prefs хоста и новое значение
    /// ("backgroundToasts"|"useConptyDll"|"skipDeleteConfirm").
    SetPref(&'static str, bool),
    /// Счётчики статус-бара (расширения/команды).
    StatusCounts(crate::ui::status_bar::StatusCounts),
    StartUpdateInstall,
    /// Прогресс скачивания апдейта: (скачано, всего). `total = None` —
    /// сервер не отдал `Content-Length` (у оригинала это «Updating N.N MB»).
    UpdateProgress(u64, Option<u64>),
    /// Скачивание/запуск апдейта не удались — пилюля возвращается в обычный
    /// вид, пользователю уходит sticky-тост.
    UpdateInstallFailed(String),
    /// Инсталлер запущен — выйти ШТАТНО (закрыть окно → web::shutdown()).
    /// std::process::exit(0) пропускал CEF-шатдаун: грязный кэш ронял
    /// libcef CHECK'ом (0x80000003) на ПЕРВОМ старте после каждого апдейта.
    GracefulQuit,
    /// Клик в интерактивном семпле Design-панели (дропдаун/чекбоксы).
    DesignSample(crate::ui::design_samples::DesignAction),
    /// Иконка расширения: (id, data-URL или None).
    ExtensionIcon(String, Option<String>),
    /// Записать строку в системный лог. Нужно парити-гейту: перенос длинного
    /// токена внутри слова иначе нечем воспроизвести.
    PushSysLog(String),
}
