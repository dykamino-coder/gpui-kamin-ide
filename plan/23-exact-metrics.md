# 23 — Точные per-component метрики (дословно, для 1:1)

Источник: `.module.css` под src/renderer/components + theme/. Значения СКОПИРОВАНЫ, не округлены. Правило №0 (plan/22): в GPUI — ровно эти числа. `component/элемент → prop: value`.

## Токены (резолв)
space 4/8/12/16/20/24/28 · radius xs4/sm8/md12/lg16=xl16 · fs xs11/sm12/md13/lg16/xl22 · lh none1/snug1.3/normal1.4/base1.5/relaxed1.6 · transition-fast 150ms ease · sans Bricolage Grotesque Variable · mono JetBrains Mono
layout: titlebar 42 · activity-bar 48 · sidebar 280(min200) · aux 280 · panel 220 · status 24 · section-header 36 · panel-tabs 32 · icon-btn round28/square22/titlebar36 · editor-tabs 35/tab 30 · palette 640

## AppLayout (layout/AppLayout.module.css)
- .appWrapper → height 100vh, width 100vw, flex column, overflow hidden
- .body → flex row, gap 8px, padding 0 4px, min-height 0
- .mainColumn → flex 1, column, gap 0 (MainBottom владеет своей 10px-ручкой)

## ActivityBar (activity-bar/ActivityBar.module.css)
- .bar → width var(--layout-activity-bar-width,48) [фолбэк в модуле 44 — при загруженных токенах = 48], column, align center, gap 8, padding 12 0
- .list → gap 2, width 100%
- .btn/.picker → 32×32, radius 8, grid place-items center, transition background/color 150ms
- иконка .codicon → font-size 18, lh 1; image 18×18
- .btnActive → bg accent-primary 16%
- .dropPlaceholder → 32×32, radius 8, border 1px dashed
- .menu → min-width 220, radius 12, padding 4, border 1px, gap 1
- .menuLabel → padding 4 12, fs 11, letter-spacing 0.04em, uppercase
- .menuItem → padding 8 12, gap 8, fs 12, radius 8

## BottomTabBar (activity-bar/BottomTabBar.module.css)
- .strip → gap 4, padding 4 8, radius 8
- .tab → height 24, padding 4 10, gap 6, fs 11, weight 500, letter-spacing 0.02em, radius 8; icon 13; image 13×13
- .tabActive → bg accent-primary 16%
- .dropPlaceholder → 36×24, radius 8, border 1px dashed

## Titlebar (titlebar/Titlebar.module.css)
- .titlebar → height 42, fs 12, bg transparent, flex align center
- .brand → 42×42, color accent-primary; brand codicon 18 !important; .brandLogo 26×26
- .tabsSlot → flex 1, padding 0 12
- .welcomeTab → height 28, padding 0 12, gap 8, radius 8 8 0 0, fs 12; codicon 13
- .searchButton → height 26, padding 0 12, margin-right 8, gap 8, border 1px, radius 8, fs 11; codicon 12
- .kbd → mono, fs 10, padding 2 6, radius 4
- .controls → padding-right 4

## TitlebarButton (titlebar/TitlebarButton.module.css)
- .btn → 36×36, margin 0 4, radius 50%, transition 150ms
- .btn > i → 16×16 box, fs 13, lh 1
- .devtools → width auto, padding 0 12, gap 4, radius 12; label fs 12
- .close:hover → bg accent-red

## StatusBar (status-bar/StatusBar.module.css)
- .statusBar → height 24, fs 11, padding 0 8, gap 4, align stretch
- .left/.right → gap 2 (намеренно теснее space-1)
- .item → padding 0 8, gap 4, fs 11, radius 4; codicon 12 !important
- .brand → weight 500; .update → weight 600, bg accent-primary 22%; .progressFill → transition width 120ms linear

## Sidebar (sidebar/Sidebar.module.css)
- .sidebar → transparent, column, flex-shrink 1, position relative
- .resizeHandle → absolute, right -8, width 8, full height, col-resize
- .resizeHandleBar → width 2→3 hover/active, opacity 0→1, transition 0.15s

## FilePanel (file-panel/FilePanel.module.css)
- .filePanel → column, flex-shrink 1, position relative
- .resizeHandle → absolute, left -8, width 8, full height; bar 2→3 hover
- .card → composes glint-surface, radius 16, overflow hidden
- .modeHeader → padding 6 8 0, justify flex-end
- .splitHandle → height 10, row-resize; .splitGrip 32×3, radius 4, opacity 0.7→1

## FileTreeHeader (file-tree/FileTreeHeader.module.css)
- .header → padding 8 8 8 12, gap 4, align center
- .title → fs 11, weight 500, letter-spacing 0.08em, uppercase, font-feature ss01
- .indexing → gap 4, fs 11, opacity 0.85; codicon 12
- .actions → gap 2
- .btn → 22×22, radius 4, grid place-items center, transition 150ms; codicon 14; disabled opacity 0.4

## FileTreeView (file-tree/FileTreeView.module.css)
- .body → padding 4 6 8, fs 12, overflow auto
- .empty → gap 8, padding 20; .emptyIcon 32; .emptyHint 12
- .openBtn → padding 6 14, radius 8, fs 12, weight 600, border 1px
- .row → height 22, gap 6, padding-right 8, border 1px transparent (резерв), radius 4, fs 12
- .rowSelected → gradient bg (accent 26%→14%), border-color accent 45%
- .dropTarget → outline 1px accent, offset -1
- .treeCheckbox → 14×14, margin-right 4, border 1px, radius 3, fs 11
- .chevron/.chevronSpacer → width 16, fs 13; .icon 16×16
- .badge → fs 11, weight 600, padding-left 6; .showMore fs 11, padding 3 0, gap 6
- .flash → animation treeFlash 0.9s ease-out
- (indent per depth — уточнить indentPx из file-tree-helpers при старте)

## FileViewerTabs (file-viewer/FileViewerTabs.module.css)
- .strip → gap 4, padding 4 8, position relative, overflow hidden
- .overflowBtn → 24×24, radius 8
- .overflowMenu → min-width 200, max-width 360, max-height 60vh, padding 4, radius 12, border 1px, box-shadow 0 6px 24px rgb(0 0 0/30%), top calc(100%+2px)
- .overflowItem → padding 5 8, gap 6, fs 12, radius 8
- .tab → height 24, padding 4 6 4 10, gap 6, fs 11, weight 500, letter-spacing 0.02em, radius 8
- .tabActive → bg accent 16%; .tabIcon 14×14; .dirty accent-orange fs 10; .pinIcon 11, opacity 0.7
- .close → 16×16, radius 4, opacity 0→0.7 (hover 1); close codicon 11
- .dropIndicator → absolute, top/bottom 5, width 2, radius 1, accent-primary

## MainContent (main/MainContent.module.css)
- .main → composes glint-surface, radius 16, margin 0, column, overflow hidden

## RightPanel (right-panel/RightPanel.module.css)
- .column → column, flex-shrink 1, position relative
- .resizeHandle → absolute, left -8, width 8; bar 2→3
- .card → composes glint-surface, radius 16, overflow hidden
- .cardHeader → padding 8 12, fs 11, weight 500, letter-spacing 0.08em, uppercase
- .empty → gap 4, padding 16; icon 24; p fs 12
- .splitHandle → height 10, padding-right 48 (=activity-bar, чтобы центрировать grip на карте); .splitGrip 32×3, radius 4

## MainBottomPanel (main-bottom-panel/MainBottomPanel.module.css)
- .panel → column, flex-shrink 0, position relative
- .resizeHandle → height 10, full width, row-resize, no border/padding
- .resizeHandleBar → 32×3, radius 4, opacity 0.7→1, transition 0.15s
- .card → composes glint-surface, flex 1, radius 16, overflow hidden

## Инварианты (замечено при извлечении)
- Ресайз-ручки Sidebar/FilePanel/RightPanel идентичны: полоса 8px в меж-панельном gap, бар 2px→3px на hover, градиентная заливка.
- Вертикальные split-ручки единообразно 10px высотой с grip 32×3px.
- Все карты панелей = `composes: glint-surface from global` + radius 16 → бордер/тень в `.glint-surface` (global.css); точные border/shadow — оттуда.
- ⚠ ActivityBar фолбэк 44px vs токен 48px — при загруженных токенах рендерится 48; брать 48.

## Отступы дерева (ВНЕ CSS — inline в TSX, критично для 1:1)
- FileTreeView (.row): paddingLeft = depth*12 + 8 (base 8, шаг 12) → d0=8, d1=20, d2=32; loading/emptyChild = indentPx(depth+1); из file-tree-helpers.tsx
- Tree (generic): INDENT_PX=14, paddingLeft = depth*14 (без base) → d0=0, d1=14
- ProblemsPanel: фикс — .row padding 0 8 0 26 (диагностики под иконкой файла); .fileRow padding 0 8; без рекурсии

## Titlebar sub-controls
- TitlebarQuickActions: .row inline-flex gap 1, padding 0 8; .btn 28×28 radius 8 color text-secondary, transition bg/color 150ms, codicon 14 !important, hover bg bg-surface; .active bg accent-primary 16%; .divider 1×14 margin 0 4 bg bg-surface
- LayoutToggles: .trigger 26×26 grid, radius 12, i 13; hover bg-surface; aria-expanded bg accent-primary 16%; .menu fixed z100 min-width220 bg bg-surface border1 divider-soft radius12 shadow-dropdown padding4 gap1 max-h calc(100vh-16); .menuLabel 4 12 fs11 upper ls0.04em; .menuItem 8 12 gap8 radius8 fs12 hover text-primary10%; .check 16×16 radius3 border1 bg-overlay; codicon12; .itemHint fs11 disabled; .divider h1 margin4 8; .presetApply 8 12 gap8 flex1 fs12; .presetIconBtn 26×26 radius8, i13
- ThemeQuickToggle: .trigger 28×28 radius8, i12; .menu absolute top calc(100%+4) right0, z1000 padding8 gap8 radius12; .header gap12 padding0 4; .title fs12 w600; .sysToggle 4 8 gap8 fs11; .columns grid 3×minmax(140,1fr) gap8; .colTitle fs11 upper ls0.04 padding4 8; .colList max-h320 gap1; .item 8 12 gap8 fs12 radius8; .itemIcon w16 fs12; .itemTick w12 fs10 accent-primary

## Session-tiles
- SessionTab: .tab flex gap6 padding 0 6 0 10, margin 6 1 (→ml2), flex 0 1 180, min44 max240, height28, bg bg-mantle border1 transparent radius12, fs12 text-secondary; first-child ml6; hover bg-surface; .active bg linear-gradient(90deg tab-color 26%,14%) border tab-color45%; .tinted 15/8 (hover22/12); light tinted26/16, active42/26 border60; .leading 16×16; .dot abs 4×4 radius50% text-muted (active=tab-color); .pin fs10 radius4 display none→flex hover (pinned=flex tab-color); dndDragging opacity0.4; sleeping0.55; .label w500 ellipsis; .close 18×18 radius4 fs10 opacity0→1 transition .12s, hover bg text-primary14%; @tab-switching opacity 1↔0.25 1s infinite
- SessionTabs: .strip flex flex1 min0 h100% overflow-x auto (scrollbar none); .dropBar 0 0 2px, 2×22 margin0 1 radius1 bg accent-primary shadow 0 0 4 accent-primary60%; .spacer flex1 min24 drag; .newTab 26×26 margin0 6 radius50% bg bg-surface text-muted, transition 150ms, hover bg accent-primary36%+bg-surface color accent-primary scale1.06, i12; .picker fixed z10001 min200 padding4 radius12 bg bg-surface border1 divider-soft shadow-dropdown; .pickerItem 6 8 gap8 fs12 radius8, codicon14

## Sidebar sub-bodies
- SessionItem: .row --tab-color=accent-primary; flex gap8 w100% h24 padding 0 8 0 16 border1 transparent radius4 fs12 text-secondary; hover bg bg-surface55% text-primary; tinted grad 24/13 (hover30/17); active grad26/14 border45%; light tinted26/16(h34/22) active42/26 b60; .dot 4×4 radius50% text-muted; [working] 6×6 accent-blue pulse1.1s; .action none→20×20 radius4 text-muted (i13, pin i10), hover-row display inline-flex opacity0.7; .time fs11 w600 opacity0.7 ml auto; inactive opacity0.6(light0.8); .actionsPop fixed z100 gap2 padding3 radius12 border1 divider-soft, ::before abs top/bottom0 left-10 w10 (hit-bridge); .popAction 24×24 radius4 text-secondary (i13), rename→accent-primary/disconnect→accent-blue/delete→accent-red; .input flex1 border1 accent-primary radius4 padding1 4 fs12
- SessionsMode: .root padding-top8 col flex1; .actions 4 8 8; .action flex gap10 w100% padding6 8 fs13 radius8 text-secondary, i w20 fs16 text-muted; .header 8 8 8 12 fs11 w500 ls0.08 upper ss01; .list 0 4 8; .empty 12 fs12
- ProjectGroup: .header h26; .headerMain flex gap6 flex1 padding0 4 0 6 h100% fs12 w500 text-secondary; .chevron fs13 w16 text-muted; .icon 16×16; .count min16 h16 padding0 5 radius9 fs11 bg bg-surface; .actionsPop fixed z100 gap2 padding3 radius12, ::before abs left-10 w10; .popAction 24×24 radius4 (codicon14) add→accent-primary/delete→accent-red; .sessions gap2; .empty 2 0 2 18 fs11; .inactiveToggle flex gap6 padding3 8 3 18 fs12 (codicon12)
- CustomizeMode: .root padding12 0 gap8 col; .header 8 12 fs11 w500 ls0.08 ss01; .list 0 8 gap2; .item flex gap8 padding8 12 radius8 fs13 (codicon14!), hover bg-surface50%, active accent-primary16%; .chevron fs12! transition transform120ms, open rotate90; .child padding-left 30 (space-3+18)
- SessionContextMenu: .menu fixed z10001 min200 padding4 radius12; .item flex gap8 padding6 8 fs12 radius8 (codicon14); .danger accent-red; .divider h1 margin4 bg divider-soft; .swatches flex gap4 wrap padding6 8; .swatch 16×16 radius50% border2 transparent hover scale1.15, active border text-primary; .swatchClear 18×18 radius50% (codicon13)

## Activity context/ghost
- ActivityContextMenu: .menu/.submenu fixed z100 min180 radius12 padding4 gap1 border1 divider-soft shadow-dropdown; .item 8 12 gap8 fs12 radius8; .chevron fs12 text-muted; move-to expanded accent-primary16%
- ActivityDragGhost: .ghost fixed z9999 translate(-50%,-50%) 28×28 grid radius8 bg accent-primary22%+bg-surface border1 accent-primary50% color accent-primary shadow 0 4 14 rgb(0 0 0/35%) opacity0.92

## Command Palette (CommandPalette.module.css)
- .scrim fixed inset0 bg overlay-modal(rgba0,0,0,.5) flex center padding-top84 z9999 anim fade0.12s
- .palette w640 max calc(100vw-32) max-h60vh bg bg-mantle border1 bg-surface80% radius12 shadow-modal
- .inputRow padding12 16 gap8 border-bottom1 (codicon16! text-muted); .input flex1 fs13 text-primary; .kbd mono fs11 padding2 6 radius4
- .list padding4 gap1 col overflow auto; .row flex baseline space-between gap12 padding8 12 fs13 radius8; hover accent-primary18%; first-child accent-primary12%; .title text-primary flex1; .category text-muted w500; .id mono fs11 text-muted; .empty 12 16 italic; .footer 8 16 border-top1 fs11

## Overlays
- ConfirmModal: .overlay fixed inset0 z9999 bg overlay-deep(rgba0,0,0,.6) flex center fadeIn0.12s; .dialog bg bg-primary border1 bg-surface radius12 padding20 min320 max480 shadow-modal; .title 0 0 12 fs13 w600; .body 0 0 16 fs12 text-secondary lh1.3; .actions flex gap8 end; .cancel/.confirm padding4 16 radius8 fs12 transition bg150ms; cancel border1 bg-overlay transparent hover bg-surface; confirm bg accent-action color accent-action-fg w600 hover accent-action-hover; danger bg accent-red hover accent-maroon
- PromptModal: .overlay = как Confirm; .dialog min360 max520; .input w100% padding8 12 border1 bg-surface radius8 bg bg-base fs13 transition border-color150ms, focus accent-primary, invalid accent-red; .error mt8 fs11 accent-red; .actions flex gap8 end mt16; confirm:disabled opacity0.5
- FindInFiles: .backdrop fixed inset0 z1000 flex center start padding-top10vh bg rgba(0,0,0,0.35) backdrop-blur2px; .box w min(720,100vw-32) max-h76vh bg bg-mantle radius12 border1 bg-surface60% shadow-dropdown; .input w100% padding12 14 border-bottom1 fs13; .status 6 14 fs11 text-muted; .list 0 0 8; .item 6 14 col gap2 radius4, active accent-primary14%; .itemHeader flex baseline gap4 fs11 text-muted; .itemLine tnum; .itemSnippet mono fs11 text-secondary; .match bg accent-orange35% radius2
- QuickOpen: .backdrop = как Find (padding-top12vh); .box w min(640,100vw-32); .input padding12 14 border-bottom1 fs13; .list padding4 0 max-h min(50vh,480); .item flex baseline gap8 padding6 14, active accent-primary14% (light bg accent-primary color accent-action-fg); .itemName fs12 w500 text-primary; .itemPath flex1 fs11 text-muted right ellipsis; .empty 12 14 fs12 center
- QuickPickModal: .overlay bg overlay-modal flex center padding-top84 z9999 qpFade0.12s; .panel w640 max-h60vh bg bg-mantle radius12 shadow-modal border1 bg-surface80%; .title 8 16 fs12 w600 border-bottom1; .input margin8 12 0 padding8 12 bg bg-base border1 bg-surface70% radius8 fs13 focus accent-primary; .list padding4 gap1 col; .item flex baseline gap8 padding8 12 radius8 fs13 hover accent-primary18%; .check center fs13 accent-primary; .description text-muted fs12; .detail ml auto fs11 mono ellipsis; .empty 12 16 italic; .prompt 4 16 0 fs12 text-secondary; .separator 4 12 mt4 fs11 upper ls0.04 border-top1; .actions flex end gap8 8 12 border-top1; cancel/ok padding4 12 radius8 border1 transparent fs12; ok bg accent-primary color #fff hover accent-action-hover
- Toasts: .stack fixed bottom36 right16 z99998 col gap8 pointer-none max360; .toast flex start gap12 padding12 16 border1 bg-surface70% radius12 bg bg-surface50% backdrop-blur8px shadow-card-popup fs12 anim slide0.18s (translateX8→0 opacity0→1); leaving slideOut0.18s (0→12); .icon mt2 fs13; .title w600 mb2; .message text-secondary break; .actions flex gap8 mt8 wrap; .actionBtn padding2 12 radius4 border1 accent-primary40% fs11 transition bg150ms; .dismiss 16×16 grid fs11 text-disabled; info/success/warning/error icon → accent-blue/green/yellow/red
- Tooltip: .tooltip fixed pointer-none z99999 bg bg-surface color text-primary padding4 8 radius4 fs11 lh1.3 nowrap ellipsis max min(640,100vw-16) shadow-mini(0 2 8 rgba0,0,0,.3) transition opacity0.1s

## Прочее
- tool-icon: НЕТ CSS-модуля — размер задаёт вызывающий (18 в activity, 13-14 в табах/меню, 12 в статус-баре); глиф = SVG currentColor (tool-icon-paths.ts)
- font-family: наследуется (font:inherit) везде, кроме .kbd/.id/.itemSnippet/.detail (mono); база — глобальное body-правило
- Панель/карта высоты/ширины — inline-стили от resize-состояния (mainSplit/rightPanelSplit/rightPanelWidth/mainBottomHeight) + layout-токены
- Источники хитрых визуалов (glint/радиалы/тени/blur) — plan/24

## Батч 2 — остальные модули (итого 60/60 .module.css сняты)

### terminal/TerminalToolbar
- .bar flex align-end gap4 padding0 25 min-h30; .tabs flex align-end gap2 flex1 overflow-x auto (scrollbar none); .scrollBtn 22×30 grid radius4 text-secondary, hover bg-surface, disabled opacity0.35, codicon12; .tab inline-flex gap6 padding0 10 h30 radius 8 8 0 0 text-secondary fs11 w500 ls0.02 min80 max220, codicon12, hover bg-surface50%, active bg editor-bg; .tabActive::before/::after 6×6 радиал-угол (transparent6→editor-bg6.5) left-6/right-6; .tabLabel max160 ellipsis; .close 16×16 radius4 opacity0→(hover0.7), hover bg-overlay60%, codicon11; .addBtn 28×28 self-center radius50% text-secondary hover bg-surface, expanded accent-primary14%, codicon15; .menu fixed z100 min200 bg-surface border1 divider-soft radius12 shadow-dropdown padding4 gap1; .menuItem flex gap8 padding8 12 radius8 fs12 hover text-primary10%; .itemIcon w16 text-muted; .defaultTag fs11 text-muted upper ls0.04; .starBtn 24×24 radius8 text-muted (codicon12), starOn accent-primary
### terminal/TerminalView
- .root flex1 col margin0 6 6 bg bg-mantle radius12 overflow hidden; .body flex1 rel bg editor-bg radius12; .session abs top8 right22 bottom10 left14 col; .empty abs inset0 col center gap8 text-muted (codicon28 opacity0.6, p fs12)
### problems/ProblemsPanel (full)
- .root col h100%; .header flex space-between padding8 8 8 12 fs11 w500 upper ls0.08 ss01 text-muted; .counts inline-flex gap4; .countBtn inline-flex gap3 padding1 6 border1 transparent radius9 text-muted fs11, hover bg-surface70%, active bg accent-primary18% border accent-primary40%, codicon12; errIcon accent-red/warnIcon accent-yellow; .list flex1 overflow auto padding0 0 8 fs12; .empty h100% col center padding20 text-muted fs12; .fileRow flex gap6 w100% h24 padding0 8 text-secondary fs12 hover bg-surface60%; .chevron fs13 w16 text-muted; .fileIcon 16×16; .fileName text-primary; .fileDir flex1 ellipsis text-muted fs11; .fileCount min16 h16 padding0 5 radius9 bg bg-surface text-muted fs11; .row flex gap6 w100% min-h22 padding0 8 0 26 text-secondary fs12 hover bg-surface60%; .sevIcon fs14 (error red/warning yellow/info blue/hint muted); .message flex1 ellipsis; .origin/.location text-muted fs11; .showMore flex gap6 fs11 text-muted padding6 10
### main/DesignPanel + design-sections
- DesignPanel: .root col gap24 padding-bottom24; .section col gap12; .sectionTitle fs16 w600; .sectionSubtitle fs12 text-muted lh1.3; .sectionBody border1 bg-surface60% radius12 bg bg-mantle padding16
- design-sections: .swatches grid auto-fill minmax(180,1fr) gap8; .swatch flex gap8 padding8 bg bg-surface30% radius4; .swatchChip 28×28 radius4 border1 text-primary12%; .typoRow/.spaceRow grid 90 60 1fr gap12; .spaceBar h16 bg accent-primary radius4; .radiusBox 80×80 bg bg-surface border1 accent-primary50%; .shadowBox 100×64 bg bg-primary radius8; .btnPrimary padding4 16 radius8 fs12 bg accent-action color accent-action-fg w600; .btnSecondary border1 bg-overlay; .btnDanger bg accent-red; .btnGhost transparent; .input w100% max360 padding8 12 border1 bg-surface radius8 bg bg-base fs13 focus accent-primary; .listItem flex gap8 padding8 12 radius8 fs13 (codicon14), active accent-primary14% (light: bg accent-primary color fff w600); .dropdownMenu abs top calc(100%+4) min220 bg bg-mantle radius12 shadow-dropdown padding4 z100; .chip inline-flex gap4 padding1 8 radius4 fs11 (green14%/muted/danger варианты); .kbd mono fs11 padding2 6 radius4 border1; .badge min18 h18 padding0 6 radius9 fs11 w600 bg accent-red
### main/WelcomePlaceholder (см. также plan/24 §3)
- .welcome flex1 col center gap16 padding24 overflow auto; .logoWrap rel grid mb4; ::before 220×220 radius50% radial(accent-primary26%→transp68%) blur6px z0; .logo rel z1 112×112 drop-shadow(0 6 18 rgba0,0,0,.35); .title fs 2.4rem w700 ls-0.02 lh1.05 text-primary; .version padding2 10 radius999 bg accent-primary14% fs11 tnum; .tagline max30rem fs13 lh1.3 text-muted; .actions flex wrap gap12 center mt8; .primary/.secondary inline-flex gap8 padding8 16 radius8 fs12 w600 transition bg+transform; primary bg accent-primary color #fff, hover translateY(-1px); secondary bg text-primary6% border1 divider-soft; .features flex wrap gap8 20 center mt12 max34rem; .feature inline-flex gap8 fs12 text-muted (i accent-primary fs13)
### main/LogsPanel
- .layout grid 220 1fr gap12 h100%; .list col gap2 overflow auto; .item col gap2 padding8 12 border1 transparent radius8 text-secondary hover bg-surface50%, active accent-primary14% border accent-primary35%; .itemName fs12 w500; .itemExt fs11 text-muted mono; .right grid auto 1fr gap8; .toolbar flex gap8; .search flex1 padding4 8 bg bg-base border1 bg-surface radius8 fs12 focus accent-primary; .toolBtn 26×26 grid radius8 text-secondary hover bg-surface (codicon14, disabled opacity0.4); .body bg bg-base border1 bg-surface radius8 padding12 mono fs11 pre-wrap lh1.3; .empty col center gap8 text-muted padding20 (i32 opacity0.6)
### main/SystemLogPanel
- .layout col h100%; .toolbar flex gap8 padding0 0 8; .search flex1 h28 padding0 10 bg bg-base border1 divider-soft radius8 fs12 focus accent-primary; .levels flex gap2; .levelBtn padding4 10 border1 transparent radius8 text-muted fs11 capitalize, hover text-primary8%, active accent-primary22%; .clear 28×28 grid radius8 text-muted; .list flex1 overflow-y auto mono fs11; .row grid 16 max-content 1fr max-content gap8 padding3 8 border-bottom divider-soft50%; .icon fs13 (error red/warning yellow/info blue); .source text-muted nowrap; .message text-primary pre-wrap; .time text-muted fs11; .empty col center gap8 text-muted padding16 (i24 opacity0.5)
### main/CustomizePanel
- .panel flex1 col overflow hidden; .header padding20 24 12 border-bottom bg-overlay30%; .title fs22 w600; .subtitle mt4 fs13 text-muted; .body flex1 overflow-y auto padding16 24; .bodyFlush flex1 col min0 overflow hidden; .placeholder col center gap8 padding28 text-muted (i32 opacity0.5)
### panel-placeholder/WebviewLoadingSkeleton
- .wrap abs inset0 col gap14 padding16 18 bg bg-surface overflow hidden; .bar flex gap10; .rows col gap14; .row flex gap12; .lines col gap7 flex1; .sk rel overflow hidden radius6 bg text-primary8%, ::after sweep-градиент anim kaminSkShimmer 1.25s (100%{translateX100%}); .pill 84×22 radius8; .search flex1 h22 radius8; .icon 30×30 radius8; .line h11 w var(--sk-row); .lineDim h9 opacity0.6 w62%; строки-циклы w 90/70/80/60/75/50%; .errIcon fs22 accent-yellow opacity0.85; .errTitle fs13 w600; .errHint fs12 text-muted max280 lh1.4; .retry inline-flex gap6 padding6 16 radius8 border1 divider-soft bg text-primary6% fs12; .waitNote mt12 fs11 text-disabled tnum
### panel-placeholder/ChatSwitchSkeleton (см. plan/24 §11)
- .wrap abs inset0 col center gap18 padding24 bg editor-bg; .brand rel grid 96×96; .glow abs 150×150 radius50% radial(accent-primary28%→transp66%) blur8px anim kaminSwitchBreathe2.4s; .logo rel z1 64×64 drop-shadow(0 6 18 .35) anim kaminSwitchFloat2.4s; .caption fs12 text-muted; .bar rel 180×3 radius999 overflow hidden bg text-primary8%; .barFill abs inset0 linear(transp→accent-primary→transp) translateX(-100%) anim kaminSwitchSweep1.15s; reduced-motion → anim none
### panel-placeholder/PanelPlaceholder + ActivityPlaceholder
- PanelPlaceholder: .placeholder flex1 col center gap8 padding20 text-muted; .glyph svg 28×24; .label fs16 w600; .hint fs12 text-muted lh1.3; .trigger inline-flex gap8 padding4 12 bg accent-primary16% radius8 fs12 mt4 hover 26%, i fs10
- ActivityPlaceholder: .placeholder col center gap8 padding20 text-muted; .glyph fs36 text-disabled; .label fs13 w600; .hint fs11 text-muted lh1.3 max240
### extensions/ExtensionsPanel
- .root col h100%; .header flex space-between gap8 padding4 8 4 12 fs11 upper ls0.04 text-muted; .installBtn inline-flex gap4 padding3 8 fs11 radius8 border1 accent-primary40% bg accent-primary14%, hover26%, codicon12; .list flex1 overflow auto padding0 8 8; .empty padding12 text-muted fs12; .groupHeader padding8 8 4 fs11 w600 upper ls0.04 text-muted; .row flex gap8 padding8 radius8 hover bg-surface60%; .icon/.iconFallback 26×26 radius4 (fallback grid fs16 text-muted); .disabled opacity0.55; .meta flex1 col; .name fs12 text-primary ellipsis; .sub fs11 text-muted; .toggle padding2 10 fs11 radius8 border1 text-muted30% bg bg-surface hover bg-overlay; .uninstall 24×22 grid radius8 text-muted, hover bg accent-red16% color accent-red
### file-viewer/FileViewer + WebviewPanelView + MonacoEditor
- FileViewer: .viewer flex1 col margin0 6 6 bg bg-mantle radius12 overflow hidden; .body flex1 col bg editor-bg radius12 padding8 0 10; .bodyFlush padding0; .empty col center gap8 text-muted padding20 (codicon36 text-disabled; kbd padding2 6 bg bg-surface radius4 mono fs11 border1)
- WebviewPanelView: .container rel 100%; .frame w/h100% border none bg transparent; .loader abs inset0 flex center bg bg-surface opacity1 transition180ms z2, loaderHidden opacity0; .spinner 22×22 radius50% border2.5px text-primary16% top accent-action anim kaminWvSpin0.7s (rotate360)
- MonacoEditor: .host flex1; slider radius4; .error flex1 center padding20 accent-red mono fs12
### file-tree/FileContextMenu + TreeIcon
- FileContextMenu: .menu fixed z100 min180 bg-surface border1 divider-soft radius12 shadow-dropdown padding4 gap1 max-h/w calc(100v-16); .item flex gap8 padding8 12 radius8 fs12 hover text-primary10%; .itemIcon w16 fs12 text-muted; .chevron fs12 text-muted ml8; .danger accent-danger #e5484d (hover bg16%); .separator h1 margin4 8 bg divider-soft
- TreeIcon: .img block; ⚠ [light] .img filter saturate(3.2) brightness(0.7) — light-подстройка Catppuccin-иконок (см. plan/24)
### file-panel/BrowserPane + FilePanelModeTabs
- BrowserPane: .pane col flex1; .navbar flex gap4 padding4 6; .navBtn 26×26 radius8 text-secondary hover bg-surface-hover; .addr flex1 h26 padding0 10 border1 divider-soft radius8 bg bg-base fs12 focus accent-primary; .viewport flex1 margin0 6 6 radius12
- FilePanelModeTabs: .switcher inline-flex; .tab inline-flex gap5 h24 padding0 10 border1 divider-soft bg bg-surface text-secondary fs12; .left radius 12 0 0 12 border-right none; .right radius 0 12 12 0; .active linear-gradient(90deg accent-primary26%,14%) border accent-primary45% (см. plan/24 §7)
### activity-bodies/ContributedContainerBody
- .root col h100%; .view col flex1; .title flex padding4 12 fs11 upper ls0.04 text-muted; .viewDescription ml8 w400 opacity0.55; .viewBadge ml auto min18 padding0 5 radius9 bg accent-primary color bg-base fs0.75em lh16; .frame flex1 margin0 8 8 overflow hidden radius16 rel; .frameFlush flex1 rel
### settings/SettingsPanel + LegacyBridgeCard
- SettingsPanel: .root col gap16; .section col gap8; .sectionTitle fs11 w600 ls0.06 upper text-muted; .row flex align-start gap10 padding4 0 fs13; .rowDesc mt2 fs11 lh1.5 text-muted; .placeholder col center gap8 padding24 0 (i32 opacity0.5)
- LegacyBridgeCard: .card flex align-start gap12 padding12 bg bg-surface border1 divider-soft radius12; .icon 32×32 radius8 accent-primary fs16; .title fs13 w600; .desc mt4 fs12 lh1.5 text-muted; .remove padding4 12 border1 accent-red radius8 transparent accent-red fs12 w600, hover bg accent-red color fff, disabled opacity0.6
### tree/Tree (generic)
- .row inline-flex gap8 w100% padding4 8 border1 transparent radius4 text-primary fs12 hover bg-surface55%, .selected linear-gradient(90deg accent-primary26%,14%) border accent-primary45% (см. plan/24 §7); .chevron w14 grid fs10 text-muted (hidden→visibility hidden); .iconDir accent-yellow fs12; .iconFile text-muted fs12; .label flex1 ellipsis; .meta mono fs11 text-muted; INDENT_PX=14 (padding-left depth*14)

Итог: **все 60 .module.css** + indent-константы + inline-размеры + хитрые визуалы (plan/24) сняты дословно. Основа metrics-модуля GPUI готова.
