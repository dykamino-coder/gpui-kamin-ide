//! Диспетчер событий: по варианту `ShellEvent` выбирает домен-обработчик.
//!
//! Раньше это был один `match` на 2133 строки внутри `root.rs`. Тела армов
//! разъехались по модулям рядом, а здесь остался только выбор адресата —
//! видно с одного экрана, кто за что отвечает.

use crate::host::events::CzEvent;
use crate::host::events::EdEvent;
use crate::host::events::TermEvent;
use crate::host::events::TreeEvent;
use gpui::Context;

use crate::host_link::ShellEvent;
use crate::root::RootView;

impl RootView {
    /// Разбор события состояния: находим домен и передаём ему событие.
    /// Применить событие. Возвращает `false`, если состояние ИНТЕРФЕЙСА оно
    /// не меняет и кадр заказывать незачем — таких событий большинство в
    /// горячих потоках (вывод терминала, доставка страницам, пустые маршруты).
    /// Ошибиться в сторону `true` безопасно (лишний кадр), в сторону `false`
    /// — нет (устаревшая картинка), поэтому список ЯВНЫЙ и короткий.
    pub fn apply(&mut self, event: ShellEvent, cx: &mut Context<Self>) -> bool {
        let needs_frame = !matches!(
            &event,
            // Насос терминала сам будит грид; событие состояния не меняет.
            ShellEvent::Term(TermEvent::TermWakeup)
                // JS в страницы: исполняется в CEF, поля RootView не трогает.
                | ShellEvent::Term(TermEvent::WvJs { .. })
                // Маршрут «в сторы следующих фаз» — пустой обработчик.
                | ShellEvent::HostEvent { .. }
                // Перечитка листингов: только исходящие запросы, мутации
                // придут отдельными DirListing.
                | ShellEvent::Tree(TreeEvent::RefreshTree)
        );
        match &event {
            ShellEvent::HostReady { .. }
            | ShellEvent::WsConnected
            | ShellEvent::WsDisconnected
            | ShellEvent::HostEvent { .. } => self.apply_connection(event, cx),
            ShellEvent::LocalSessionPinned { .. }
            | ShellEvent::LocalSessionClosed { .. }
            | ShellEvent::Sessions { .. }
            | ShellEvent::ToggleProjectCollapse { .. }
            | ShellEvent::ActivateSession { .. }
            | ShellEvent::OpenSessionMenu { .. }
            | ShellEvent::CloseSessionMenu
            | ShellEvent::OpenRenamePresetPrompt { .. }
            | ShellEvent::ToggleNewSessionMenu { .. }
            | ShellEvent::NewEmptySession
            | ShellEvent::NewSessionInFolderPrompt
            | ShellEvent::BeginRename { .. }
            | ShellEvent::CommitRename
            | ShellEvent::CancelRename
            | ShellEvent::ChipPress { .. }
            | ShellEvent::ChipDragOver { .. }
            | ShellEvent::ChipRelease
            | ShellEvent::CoverTick
            | ShellEvent::ChatBound => self.apply_sessions(event, cx),
            ShellEvent::Ed(EdEvent::LocateSelectedFile)
            | ShellEvent::Ed(EdEvent::FileMenuOpenIn { .. })
            | ShellEvent::Tree(TreeEvent::RefreshTreeHard)
            | ShellEvent::Tree(TreeEvent::RefreshTree)
            | ShellEvent::Tree(TreeEvent::CollapseTree)
            | ShellEvent::Tree(TreeEvent::DirListing { .. })
            | ShellEvent::Tree(TreeEvent::ToggleDir { .. })
            | ShellEvent::Ed(EdEvent::OpenFileMenu { .. })
            | ShellEvent::Ed(EdEvent::CloseFileMenu)
            | ShellEvent::Ed(EdEvent::FileOpenFailed { .. }) => self.apply_tree_listing(event, cx),
            ShellEvent::Tree(TreeEvent::ShowMoreDir { .. })
            | ShellEvent::Tree(TreeEvent::DecoSet { .. })
            | ShellEvent::Tree(TreeEvent::DecoInvalidate { .. })
            | ShellEvent::Cz(CzEvent::IndexStatus { .. })
            | ShellEvent::Tree(TreeEvent::SelectTreeNode { .. })
            | ShellEvent::Ed(EdEvent::FsCut { .. })
            | ShellEvent::Ed(EdEvent::FsCopy { .. })
            | ShellEvent::Ed(EdEvent::FsPaste { .. })
            | ShellEvent::Ed(EdEvent::FsDropMove { .. })
            | ShellEvent::Cz(CzEvent::ToggleProblemsFile { .. })
            | ShellEvent::Cz(CzEvent::ProblemsShowMore)
            | ShellEvent::Tree(TreeEvent::TreeChildren { .. })
            | ShellEvent::Tree(TreeEvent::TreeMetaSet { .. })
            | ShellEvent::Tree(TreeEvent::TreeChanged { .. }) => {
                self.apply_tree_decorations(event, cx)
            }
            ShellEvent::Tree(TreeEvent::TreeClick { .. })
            | ShellEvent::Tree(TreeEvent::TreeReveal { .. })
            | ShellEvent::Tree(TreeEvent::TreeDnd { .. })
            | ShellEvent::Tree(TreeEvent::TreeDragStart { .. })
            | ShellEvent::Tree(TreeEvent::TreeDrop { .. })
            | ShellEvent::Tree(TreeEvent::TreeCheckbox { .. })
            | ShellEvent::Ed(EdEvent::SetFileMode { .. })
            | ShellEvent::Ed(EdEvent::OpenFile { .. })
            | ShellEvent::Ed(EdEvent::OpenFileAt { .. })
            | ShellEvent::Ed(EdEvent::FileOpened { .. })
            | ShellEvent::Ed(EdEvent::ToggleFileTabsOverflow)
            | ShellEvent::Ed(EdEvent::PushFsUndo { .. })
            | ShellEvent::Ed(EdEvent::UndoFsOp)
            | ShellEvent::Ed(EdEvent::FsDeletePermanent { .. }) => {
                self.apply_tree_interaction(event, cx)
            }
            ShellEvent::Ed(EdEvent::FsDelete { .. })
            | ShellEvent::Ed(EdEvent::FilesChanged { .. }) => self.apply_editor(event, cx),
            ShellEvent::ToggleFindInFiles
            | ShellEvent::CloseFindInFiles
            | ShellEvent::FindInFilesResults { .. } => self.apply_find_in_files(event, cx),
            ShellEvent::OpenDevtools { .. }
            | ShellEvent::Cz(CzEvent::DiagSnapshot { .. })
            | ShellEvent::Cz(CzEvent::DiagSet { .. })
            | ShellEvent::OpenToolTabMenu { .. }
            | ShellEvent::Ed(EdEvent::OpenInTerminal { .. })
            | ShellEvent::OpenModal { .. }
            | ShellEvent::OpenToolPicker { .. }
            | ShellEvent::Cz(CzEvent::OpenCustomizePanel { .. })
            | ShellEvent::Ed(EdEvent::CloseEditorTab { .. })
            | ShellEvent::Ed(EdEvent::SelectEditorTab { .. })
            | ShellEvent::Ed(EdEvent::CloseOtherEditorTabs { .. })
            | ShellEvent::Ed(EdEvent::CloseEditorTabsRight { .. })
            | ShellEvent::Ed(EdEvent::CloseAllEditorTabs)
            | ShellEvent::Ed(EdEvent::OpenEditorTabMenu { .. })
            | ShellEvent::Ed(EdEvent::TogglePinEditorTab { .. })
            | ShellEvent::Ed(EdEvent::CloseEditorTabMenu)
            | ShellEvent::OpenSaveLayoutPrompt
            | ShellEvent::ToggleQuickOpen
            | ShellEvent::CloseQuickOpen
            | ShellEvent::QuickOpenResults { .. }
            | ShellEvent::Ed(EdEvent::EditorFind) => self.apply_editor(event, cx),
            ShellEvent::TooltipShow { .. }
            | ShellEvent::TooltipHide
            | ShellEvent::CloseInputOverlays
            | ShellEvent::Cz(CzEvent::ExplorerMenuItems { .. })
            | ShellEvent::QuickPickShow { .. }
            | ShellEvent::CloseToolTabMenu
            | ShellEvent::ToolMenuSub { .. }
            | ShellEvent::Cz(CzEvent::InstallVsixPrompt)
            | ShellEvent::QuickPickToggle { .. }
            | ShellEvent::QuickPickResolve { .. }
            | ShellEvent::ConfirmModal
            | ShellEvent::ConfirmModalInput { .. }
            | ShellEvent::CloseModal
            | ShellEvent::CloseToolPicker
            | ShellEvent::TogglePalette
            | ShellEvent::ClosePalette
            | ShellEvent::PaletteGate { .. }
            | ShellEvent::Toast { .. }
            | ShellEvent::DismissToast { .. }
            | ShellEvent::ToastGone { .. }
            | ShellEvent::WebMenu { .. }
            | ShellEvent::WebMenuCmd { .. }
            | ShellEvent::FocusStep { .. }
            | ShellEvent::OverlayMove { .. }
            | ShellEvent::ToggleLayoutPopover
            | ShellEvent::ToggleAppearancePopover
            | ShellEvent::OverlayRowHover { .. }
            | ShellEvent::SetOverlayQuery { .. }
            | ShellEvent::ExternalToast { .. }
            | ShellEvent::ToggleWorkspaceSymbols
            | ShellEvent::CloseWorkspaceSymbols
            | ShellEvent::WorkspaceSymbolsResults { .. } => self.apply_overlays(event, cx),
            ShellEvent::HoverPill { .. } | ShellEvent::DismissHoverPill => {
                self.apply_sessions(event, cx)
            }
            ShellEvent::Term(TermEvent::TermClose { .. })
            | ShellEvent::Term(TermEvent::ToggleTermMenu) => self.apply_terminal(event, cx),
            ShellEvent::Cz(CzEvent::ToggleCustomizeContribGroup { .. })
            | ShellEvent::ToggleTabsOverflow { .. }
            | ShellEvent::ActivityClicked { .. }
            | ShellEvent::ToggleSidebar
            | ShellEvent::ToggleInactive { .. }
            | ShellEvent::OverwriteLayoutPreset { .. }
            | ShellEvent::ExportPreset { .. }
            | ShellEvent::ExportPresets
            | ShellEvent::ImportPresets
            | ShellEvent::PinTool { .. } => self.apply_layout_panels(event, cx),
            ShellEvent::Cz(CzEvent::ToggleCustomize)
            | ShellEvent::Cz(CzEvent::SetCustomizePanel { .. })
            | ShellEvent::Cz(CzEvent::ToggleExtension { .. })
            | ShellEvent::Cz(CzEvent::ToggleProblemsFilter { .. })
            | ShellEvent::Cz(CzEvent::PrefsLoaded { .. })
            | ShellEvent::Cz(CzEvent::SetPref { .. })
            | ShellEvent::Ed(EdEvent::TabDragOver { .. })
            | ShellEvent::ToggleLayoutFlag { .. }
            | ShellEvent::ApplyLayoutPreset { .. }
            | ShellEvent::DeleteLayoutPreset { .. }
            | ShellEvent::SetDefaultLayoutPreset { .. }
            | ShellEvent::ToolDragOverTab { .. } => self.apply_layout_presets(event, cx),
            ShellEvent::Cz(CzEvent::ExtensionIcon { .. })
            | ShellEvent::Cz(CzEvent::StatusBarSnapshot { .. })
            | ShellEvent::Cz(CzEvent::StatusBarUpsert { .. })
            | ShellEvent::Cz(CzEvent::StatusBarRemove { .. })
            | ShellEvent::Cz(CzEvent::ThemesList { .. })
            | ShellEvent::Cz(CzEvent::IconThemesList { .. })
            | ShellEvent::Cz(CzEvent::SetIconTheme { .. })
            | ShellEvent::Cz(CzEvent::IconThemeLoaded { .. })
            | ShellEvent::Cz(CzEvent::UpdateAvailable { .. })
            | ShellEvent::Cz(CzEvent::KeybindingsList { .. })
            | ShellEvent::Cz(CzEvent::CustomizePages { .. })
            | ShellEvent::Tree(TreeEvent::RevealView { .. })
            | ShellEvent::Cz(CzEvent::SetCustomizeContribPage { .. }) => {
                self.apply_customize_pages(event, cx)
            }
            ShellEvent::Cz(CzEvent::ExtensionsLoaded { .. })
            | ShellEvent::Cz(CzEvent::ExtensionsStatus { .. })
            | ShellEvent::Cz(CzEvent::UninstallExtension { .. })
            | ShellEvent::Commands { .. }
            | ShellEvent::Cz(CzEvent::StatusCounts { .. })
            | ShellEvent::Cz(CzEvent::StartUpdateInstall)
            | ShellEvent::Cz(CzEvent::UpdateProgress { .. })
            | ShellEvent::Cz(CzEvent::UpdateInstallFailed { .. })
            | ShellEvent::Cz(CzEvent::GracefulQuit)
            | ShellEvent::SetThemeChoice { .. }
            | ShellEvent::Cz(CzEvent::SetContributedTheme { .. })
            | ShellEvent::RunCommand { .. }
            | ShellEvent::Cz(CzEvent::CheckForUpdates) => self.apply_extensions_updates(event, cx),
            ShellEvent::Term(TermEvent::TermWakeup)
            | ShellEvent::Term(TermEvent::TermInput { .. })
            | ShellEvent::Term(TermEvent::TermTabScroll { .. })
            | ShellEvent::Term(TermEvent::TermSetDefaultShell { .. })
            | ShellEvent::Term(TermEvent::TermNew { .. })
            | ShellEvent::Term(TermEvent::TermScroll { .. })
            | ShellEvent::Term(TermEvent::TermSelect { .. }) => self.apply_terminal(event, cx),
            ShellEvent::Term(TermEvent::WebviewHtml { .. })
            | ShellEvent::Term(TermEvent::WvJs { .. })
            | ShellEvent::Term(TermEvent::WebviewAlive { .. })
            | ShellEvent::Term(TermEvent::ForceViewLoadError { .. })
            | ShellEvent::Term(TermEvent::RetryView { .. }) => self.apply_webview(event, cx),
            ShellEvent::Ed(EdEvent::CopyToClipboard { .. })
            | ShellEvent::Ed(EdEvent::CopyRelativePath { .. })
            | ShellEvent::Cz(CzEvent::DesignSample { .. })
            | ShellEvent::Cz(CzEvent::PushSysLog { .. })
            | ShellEvent::Cz(CzEvent::OutputEvent { .. })
            | ShellEvent::Cz(CzEvent::ClearOutputChannel { .. })
            | ShellEvent::Cz(CzEvent::SelectOutputChannel { .. })
            | ShellEvent::Cz(CzEvent::SystemLog { .. })
            | ShellEvent::Cz(CzEvent::SetSysLogLevel { .. })
            | ShellEvent::Cz(CzEvent::ClearSystemLog) => self.apply_output(event, cx),
            ShellEvent::Workspace { .. }
            | ShellEvent::TitlebarDevtools
            | ShellEvent::Ed(EdEvent::GotoLine { .. })
            | ShellEvent::MoveToolTo { .. }
            | ShellEvent::UnpinTool { .. }
            | ShellEvent::Ed(EdEvent::TabPress { .. })
            | ShellEvent::ToolPress { .. } => self.apply_misc(event, cx),
        }
        needs_frame
    }
}
