# 25 — Трансляция VS Code-тем и иконок в GPUI (100%)

Цель: любая VSIX-тема (цветовая / файловые иконки / product icons) из маркетплейса применяется к новому приложению ПОЛНОСТЬЮ — не подмножеством, как в kamin-ide. kamin-ide резолвит ~30 chrome-токенов из author-ключей (contributed-theme-resolve, см. 20 §8); здесь — план полного покрытия.

## A. Цветовые темы (color themes)

### Формат источника (VS Code theme JSON)
1. `colors` — workbench-цвета, ~800+ ключей (`editor.background`, `activityBar.*`, `statusBar.*`, `list.*`, `button.*`, `input.*`, `dropdown.*`, `menu.*`, `tab.*`, `panel.*`, `titleBar.*`, `notifications.*`, `terminal.ansi*`, `gitDecoration.*`, `debug*`, `charts.*`, …) + `include` (наследование родительской темы) + альфа `#rrggbbaa`.
2. `tokenColors` — TextMate-правила подсветки: {scope: string|string[], settings:{foreground,background,fontStyle}}; либо строка-путь к .tmTheme.
3. `semanticHighlighting` + `semanticTokenColors` — семантические токены: селекторы `type.modifier:language` → цвет/стиль.
4. `type`/uiTheme (vs/vs-dark/hc-black/hc-light) — база.

### План трансляции — три слоя потребителей

#### A1. Chrome приложения (GPUI-виджеты)
Полная мапа workbench-ключ → GPUI-элемент. Стратегия:
- **Мапа 1-в-1 там, где регион совпадает** (полный словарь строится на имплементации из официального color-reference; здесь — структура):
  - `titleBar.activeBackground/activeForeground/inactive*` → титлбар
  - `activityBar.background/foreground/inactiveForeground/border/activeBorder/badge.*` → активити-бар
  - `sideBar.background/foreground/border`, `sideBarTitle.*`, `sideBarSectionHeader.*` → сайдбар
  - `statusBar.background/foreground/border/debugging*/noFolder*`, `statusBarItem.*` → статус-бар
  - `editorGroupHeader.tabsBackground`, `tab.activeBackground/inactiveBackground/activeForeground/…/activeBorderTop` → табы редактора
  - `panel.background/border`, `panelTitle.*` → нижние панели
  - `list.activeSelectionBackground/hoverBackground/focusOutline/…` → все списки/деревья (файловое дерево, палитра, quickpick)
  - `button.background/foreground/hoverBackground`, `button.secondary*` → кнопки
  - `input.background/foreground/border/placeholderForeground`, `inputOption.*`, `inputValidation.*` → инпуты
  - `dropdown.*`, `menu.*`, `menubar.*` → дропдауны/меню
  - `quickInput.*`, `quickInputList.*`, `pickerGroup.*` → палитра/quick-pick
  - `notifications.*`, `notificationToast.*` → тосты
  - `badge.*`, `progressBar.background`, `focusBorder`, `foreground`, `descriptionForeground`, `errorForeground`, `widget.shadow`, `scrollbar.shadow`, `scrollbarSlider.*`, `selection.background`
  - `terminal.background/foreground/ansi*16/selectionBackground/cursor*` → нативный терминал (полная ANSI-мапа!)
  - `gitDecoration.*` → декорации файлового дерева
  - `editorWidget.*`, `editorHoverWidget.*`, `editorSuggestWidget.*` → ховеры/саджесты редактора
  - `diffEditor.*`, `merge.*`, `minimap.*`, `editorGutter.*`, `editorLineNumber.*`, `editorCursor.*`, `editorWhitespace.*`, `editorIndentGuide.*`, `editorBracket*`, `editorOverviewRuler.*` → редактор
  - `peekView.*` → peek-виджет; `breadcrumb.*`; `keybindingLabel.*`; `checkbox.*`; `toolbar.*`; `banner.*`; `debugToolBar.*`/`debugIcon.*`/`debugConsole.*` (при реализации debug); `settings.*`; `symbolIcon.*` (цвета иконок символов); `charts.*`
- **Фолбэк-цепочки как в VS Code**: у каждого ключа есть default-наследование (напр. `sideBarSectionHeader.background` ← `sideBar.background`); строим тот же граф дефолтов (официальная таблица «defaults» из color-registry VS Code переносится как данные).
- **Не-1-в-1 регионы KaminIDE** (glint-панели, градиент фона, activity-слоты) — сохраняем текущий elevation-ramp алгоритм kamin-ide (NEUTRAL_SURFACE_KEYS, chroma-гейт, шаги) как ДОПОЛНЕНИЕ поверх 1-в-1 мапы: он даёт эстетически-когерентные производные для регионов, которых в VS Code нет.
- **Хранение**: резолвнутая тема = плоская таблица token→Color в GPUI ThemeRegistry; hot-swap при смене темы; кэш последней резолвнутой темы для мгновенного пейнта на старте (как сейчас).

#### A2. Редактор (подсветка)
- **TextMate-путь (основной для паритета)**: kamin-ide уже гоняет vscode-textmate + vscode-oniguruma (monaco-textmate.ts) с contributed-грамматиками. В GPUI: тот же движок — **textmate-грамматики исполняются в kamin-host** (Node, где уже есть oniguruma WASM) → токен-стримы по WS → GPUI-редактор красит спаны. `tokenColors` компилируются в scope-матчер (селекторы TextMate: точность по префиксу, приоритет по специфичности) → стиль {fg,bg,bold,italic,underline}.
- **Tree-sitter путь (нативный, быстрый)**: gpui-component editor красит tree-sitter капчурами. Мапа capture→TextMate-scope (стандартная таблица `@keyword`→`keyword`, `@string`→`string`, …) позволяет применять `tokenColors` к tree-sitter подсветке для языков, где грамматика tree-sitter есть. Правило: tree-sitter для встроенных языков (скорость), textmate — для contributed-грамматик из VSIX (совместимость). Оба питаются ОДНИМ скомпилированным `tokenColors`-матчером.
- **semanticTokenColors**: семантические токены уже идут из LSP через exthost (semantic-tokens-remap). Селекторы `type.modifier:lang` парсим → таблица приоритетов (semantic > textmate) → оверлей-стили. Паритет с VS Code: semanticHighlighting on/off из темы уважается.
- **Editor-цвета из `colors`** (editor.background/foreground/selection/lineHighlight/…) идут в A1-редакторную секцию — единый источник.

#### A3. Вебвью расширений
Без изменений (уже работает): вся `colors`-семья пробрасывается как `--vscode-<key с точками→дефисами>` CSS-переменные в каждый вебвью + `vscode-colors.css`-мост. Сохраняем 100%-проброс сырых ключей (это уже полное покрытие для вебвью).

### Совместимость/крайние случаи
- `include`-цепочки резолвим рекурсивно (с защитой от циклов); `.tmTheme` (plist) поддержать парсером (vscode-textmate умеет).
- hc-black/hc-light: маппятся на dark/light базу + контраст-ключи (`contrastBorder`, `contrastActiveBorder`) применяются к бордерам.
- Альфа-цвета: GPUI рисует rgba нативно — сохранять альфу (сейчас часть терялась в hex-sanitize; sanitize оставить только для строгих потребителей).
- uiTheme из package.json авторитетен для выбора light/dark базы (как сейчас).

## B. Файловые иконко-темы (file icon themes)

### Формат источника
`iconDefinitions` (iconPath | fontCharacter+fontColor+fontId), `fonts` [{id, src(woff/ttf), weight, style, size}], ассоциации: `file`, `folder`, `folderExpanded`, `rootFolder(+Expanded)`, `folderNames/folderNamesExpanded`, `fileExtensions`, `fileNames`, `languageIds`, + отдельные секции `light`/`highContrast`, `hidesExplorerArrows`, `showLanguageModeIcons`.

### Текущее покрытие kamin-ide (частичное)
icon-theme.ts + kamin:iconTheme:load/icon: SVG-iconPath работает; **шрифтовые иконки (fontCharacter) — гэп**; light/hc-секции — проверить; hidesExplorerArrows — нет.

### План (100%)
1. Резолвер ассоциаций — полный порядок VS Code: fileNames > fileExtensions (мульти-суффиксы `.test.ts` длиннее — приоритетнее) > languageIds > file default; для папок folderNames(Expanded) > folder(Expanded); root-варианты; light/hc-подстановка по активной базе.
2. Рендер двух видов определений:
   - iconPath (svg/png) — host отдаёт файл (kamin:iconTheme:icon расширить на произвольные пути темы, root-confined) → GPUI Image.
   - **fontCharacter** — грузить шрифт темы (woff/ttf из VSIX) в GPUI font-registry, рисовать глиф fontCharacter цветом fontColor размера size. Это закрывает Material Icon Theme и большинство популярных.
3. hidesExplorerArrows → скрыть шевроны дерева; showLanguageModeIcons → иконки в статус-баре языка.
4. Фолбэк-цепочка: активная contributed-тема → Catppuccin (встроенная) → codicon generic (как сейчас, async-апгрейд без мерцания).
5. Кэш: резолвнутые data-URL/глифы кэшируются (LS-аналог), мгновенный пейнт.

## C. Product icon themes (иконки интерфейса)
Формат: переопределение codicon-глифов (`iconDefinitions`: id → fontId+fontCharacter, свои шрифты). План: GPUI-иконко-слой читает codicon-id через registry косвенно (id → глиф из активной product-icon темы, дефолт = codicon.ttf). Низкий приоритет (редкие темы), но в архитектуру иконко-резолвера закладываем уровень indirection сразу — тогда поддержка = загрузка шрифта + таблица подмен.

## Чеклист паритета (темы/иконки)
- [ ] Полный словарь workbench-ключей → GPUI-элементы + граф дефолт-фолбэков (данные из color-registry)
- [ ] Elevation-ramp kamin-ide поверх мапы для не-VS Code регионов
- [ ] tokenColors-матчер (TextMate-селекторы) — общий для tree-sitter и textmate путей
- [ ] textmate-грамматики в host → токен-стримы в GPUI-редактор
- [ ] semanticTokenColors + приоритет semantic>textmate
- [ ] include/.tmTheme/hc/альфа/uiTheme-крайние случаи
- [ ] 100%-проброс --vscode-* в вебвью (сохранить)
- [ ] Иконко-резолвер: полный порядок ассоциаций + fontCharacter-рендер + light/hc + hidesExplorerArrows
- [ ] Product-icon indirection слой
- [ ] Кэш мгновенного пейнта тем и иконок
