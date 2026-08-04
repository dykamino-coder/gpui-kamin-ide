# 20 — Тема и визуальная система (1:1)

Источник: `src/renderer/theme/` (variables.css, dark-theme.css, light-theme.css, layout-tokens.css, global.css, vscode-colors.css, skeleton.css), `signals/theme.ts`, `signals/contributed-theme.ts` + `contributed-theme-resolve.ts`, `utils/color.ts`.

Модель в GPUI: два статических palette-структа (Dark/Light) + runtime-переопределяемая мапа для contributed-тем. `color-mix` в GPUI нет — все тинты предвычисляются альфа-блендом на этапе резолва темы.

## 1. Токены цвета (token → dark → light)

### Фоны (elevation ramp)
| Токен | Dark | Light | Роль |
|---|---|---|---|
| bg-primary | #313240 | #f6efeb | базовый — фон app, input/row |
| bg-base | #313240 | #fbf8f1 | алиас primary |
| bg-mantle | #262533 | #fbf7f4 | заливка панелей (glint-surface) |
| bg-sidebar | #1d1d28 | #f4f1ea | самый тёмный — сайдбар + подложка app |
| bg-surface | #3d3f51 | #e6e1d4 | бордеры, hover, разделители |
| bg-overlay | #515567 | #d6d0c0 | вторичная поверхность, scrollbar thumb |

### Редактор/терминал
| Токен | Dark | Light |
|---|---|---|
| editor-bg | #1d1c25 | #fcfaf6 |
| editor-fg | #dcdce4 | #48433c |
| editor-cursor | #a0a0d0 | #48433c |

(Monaco built-ins используют editor.background = editor-bg; term-bg отличается: #1d1d28.)

### Текст
| Токен | Dark | Light |
|---|---|---|
| text-primary | #cfd4e2 | #322e28 |
| text-subtext | #afb6ca | #463f37 |
| text-secondary | #adb3c7 | #524c43 |
| text-muted | #838aa0 | #6e685d |
| text-disabled | #60667b | #938e82 |
| text-muted-2 | #7f849c | #524c43 |
| text-muted-light | #acb2d2 | #524c43 |

### Акценты (dark = Catppuccin Mocha; light = смягчённый Latte)
| Токен | Dark | Light |
|---|---|---|
| accent-blue | #89b4fa | #3b6fc4 |
| accent-sapphire | #74c7ec | #3a8aa3 |
| accent-red | #f38ba8 | #ca3939 |
| accent-maroon | #eba0ac | #d35a5a |
| accent-green | #a6e3a1 | #5e9855 |
| accent-yellow | #f9e2af | #c89a3f |
| accent-pink | #f5c2e7 | #c46598 |
| accent-purple | #cba6f7 | #8a5fc8 |
| accent-orange | #fab387 | #da8343 |
| accent-teal | #94e2d5 | #4a9999 |
| accent-rosewater | #f5e0dc | #c08571 |

### Варианты акцентов
| Токен | Dark | Light |
|---|---|---|
| accent-blue-soft | #b4d0fb | #2c6cdc |
| accent-blue-soft-2 | #b4befe | #3d59c0 |
| accent-blue-soft-3 | #c0d3ff | #5773ce |
| accent-purple-soft | #b48bef | #8225ec |
| accent-green-soft | #94d899 | #2ba517 |
| accent-red-dark | #e06c8a | #8a0023 |
| accent-red-dark-2 | #e06c88 | #8a0023 |
| accent-red-dark-3 | #e87c99 | #a3002a |
| accent-orange-dark | #f9b36d | #b16527 |
| accent-yellow-dark | #8a7a2e | #8a5000 |

### Семантика действия (КЛЮЧЕВОЕ: dark=синий, light=ОРАНЖЕВЫЙ)
| Токен | Dark | Light |
|---|---|---|
| accent-action | accent-blue | accent-orange #da8343 |
| accent-action-hover | accent-sapphire | #b16527 |
| accent-action-fg | bg-primary | #ffffff |
| accent-primary | accent-blue | accent-orange |
| accent-primary-soft | accent-blue-soft | #c97338 |
| accent-primary-soft-2 | accent-blue-soft-2 | #b76530 |
| accent-primary-soft-3 | accent-blue-soft-3 | #934f25 |

### Состояния/тинт-фоны
| Токен | Dark | Light |
|---|---|---|
| bg-surface-hover | #3b3b52 | #d8d4c4 |
| bg-overlay-hover | #3e3e56 | #c2bcab |
| bg-tint-red | #2e1e22 | #fce4ea |
| bg-tint-red-soft | #45283b | #f4c2cf |
| bg-tint-green(+soft) | #1e2e1e | #e3f2dd |
| bg-tint-orange | #2e1e1e | #f6e3cb |
| bg-tint-blue | #1a1a27 | #dde7fb |

### Скримы оверлеев
| Токен | Dark | Light |
|---|---|---|
| overlay-modal | rgba(0,0,0,.5) | rgba(27,26,22,.28) |
| overlay-soft | rgba(0,0,0,.35) | rgba(27,26,22,.14) |
| overlay-deep | rgba(0,0,0,.6) | rgba(27,26,22,.40) |

### Производные тинты (в CSS — color-mix; в GPUI предвычислить)
`tint-{color}-{soft|medium|strong|border|border-strong}` = accent-X @ N% на transparent:
- red: soft 10 / soft-2 8 / medium 18 / border 30 / border-strong 40
- blue: soft 6 / medium 12 / strong 25 / border 25 / border-strong 50
- yellow: 8/12/18/30 · green: 8/14/18/40 · purple: 8/12/25 · orange soft 14 · muted soft 8 / medium 18
- divider-soft = text-primary @ 6%
- tint-surface-soft 40% / -medium 55% от bg-surface
- tint-overlay-scrim 70% / -heavy 92% от bg-sidebar; tint-overlay-medium 50% / -strong 80% от bg-overlay
- light-тема пере-объявляет tint-primary-* от accent-orange (6/12/25/25/50%)

### Терминальная палитра (нативный терминал обязан использовать её же)
| Токен | Dark | Light |
|---|---|---|
| term-bg | #1d1d28 | #f4f1ea |
| term-fg | #cfd4e2 | #48433c |
| term-cursor | #f5e0dc | #c27100 |
| term-selection | #515567 | #d6d0c0 |
| term-black | #515567 | #1b1a16 |
| term-red | #f38ba8 | #a40020 |
| term-green | #a6e3a1 | #0a6e00 |
| term-yellow | #f9e2af | #8a5000 |
| term-blue | #89b4fa | #003fa8 |
| term-magenta | #f5c2e7 | #9c0078 |
| term-cyan | #94e2d5 | #035e63 |
| term-white | #afb6ca | #3d3a33 |
| term-bright-black | #60667b | #77736a |
| term-bright-white | #adb3c7 | #1b1a16 |

## 2. Фирменный вид: glint-surface + градиент фона

### Glint-border (подсвеченная кромка плавающих панелей)
- Dark: `linear-gradient(135deg, rgba(255,255,255,.18) 0%, bg-mantle 22%, bg-mantle 78%, rgba(255,255,255,.18) 100%)`
- Light: `linear-gradient(135deg, rgba(60,40,20,.18) 0%, bg-surface 22%, bg-surface 78%, rgba(60,40,20,.18) 100%)`
- Рецепт: 1px border; заливка bg-mantle в padding-box, градиент в border-box. В GPUI: 1px градиентная обводка (135°, светлые/чернильные 0.18-альфа углы 0%/100%, панельные средние стопы) + fill bg-mantle + radius-lg.
- Панели уровня 0 (FilePanel/RightPanel/MainContent/MainBottomPanel/terminal): glint-surface + radius 16px.

### Фон приложения (AppLayout .appWrapper)
Два мягких радиальных градиента поверх bg-sidebar:
- ellipse 1200×600 @ 20% 10%: accent-purple @ 8% → transparent 60%
- ellipse 800×500 @ 90% 90%: accent-primary @ 6% → transparent 60%

## 3. Радиусы, тени, ховеры, фокус, скроллбары

- Радиусы: xs 4 (чипы/бейджи/inline-code) · sm 8 (код-блоки/таблицы) · md 12 (пузыри чата/капсулы) · lg 16 = xl (панели уровня 0). Правило: внешний = внутренний + паддинг.
- Тени (dark / light):
  - lg: 0 8 16 rgba(0,0,0,.3) / 0 6 14 rgba(27,26,22,.10)
  - card: 0 0 6 .2 / 0 0 4 .08 · modal: 0 8 32 .5 / .18 · tab: 0 6 18 .45 / .14
  - toast: 0 10 40 .4 / .16 · dropdown: 0 4 16 .5 / .16 · mini: 0 2 8 .3 / .10
  - bar: 0 -4 12 .4 / .10 · card-popup: 0 8 24 .5 / .16
- Hover-конвенция (меню/кнопки): text-primary @ 10%; мягкие ряды 5–8% или bg-surface 55%.
- Focus ring (только клавиатура): outline 2px accent-primary, offset 2px.
- Скроллбары: 8px, track прозрачный, thumb bg-overlay radius 4, hover text-disabled.
- Drop-target: bg accent-primary 10% + dashed outline accent-primary 60% offset -2; blocked: accent-red 12% + inset red 60%.

## 4. Шрифты

- UI: **Bricolage Grotesque Variable** (бандлится; в GPUI — вложить ttf variable). tabular-nums глобально.
- Mono: НЕ бандлится в kamin-ide — стек `JetBrains Mono, Fira Code, Cascadia Code, Consolas`. Для GPUI: вложить JetBrains Mono или системный фолбэк. Размер редактора/терминала 13px.
- Шкала: fs-xs 11 / sm 12 / md 13 / lg 16 / xl 22. Line-heights: 1 / 1.3 / 1.4 / 1.5 / 1.6.
- Codicon-шрифт 16px lh 1 (нужен для extension-контрибуций).

## 5. Иконки — четыре системы

| Система | Где | Реализация в GPUI |
|---|---|---|
| FontAwesome Free (solid only) | chrome хоста: титлбар, сайдбар, статус-бар, session-tiles | вложить FA solid ttf ИЛИ вендорить SVG-пути используемых глифов |
| Phosphor (regular) через ToolIcon | tool/activity глифы (folders, tree-view, search, warning, terminal, gear) | уже вендорено как SVG `d`-строки (viewBox 256, currentColor) — портировать таблицу tool-icon-paths.ts как есть |
| @vscode/codicons | contributions расширений (ThemeIcon), фолбэк ToolIcon | вложить codicon.ttf, мапа id→глиф |
| Catppuccin file icons | дерево файлов (TreeIcon → file-icons.ts + vendor мапы name/ext/lang) | SVG-данные из @iconify-json/catppuccin; цвета запечены в SVG; async-апгрейд на contributed icon theme, фолбэк Catppuccin |

Приоритет ToolIcon: image URL → Phosphor path → codicon.

## 6. Размеры регионов (layout-tokens)

| Токен | Значение |
|---|---|
| titlebar-height | 42px |
| activity-bar-width | 48px |
| primary-sidebar-width | 280px (min 200px) |
| auxiliary-bar-width | 280px |
| panel-height | 220px |
| status-bar-height | 24px |
| section-header-height | 36px |
| panel-tabs-height | 32px |
| palette-width | 640px; top-offset 84px; max-height 60vh |
| icon-button: round 28 / square 22 / titlebar 36 |
| editor-tabs-height 35 / editor-tab-height 30 |

Spacing: 4/8/12/16/20/24/28. Меж-панельный зазор — ТОЛЬКО `.body { gap: 8px; padding: 0 4px }` (дети не добавляют горизонтальных маргинов). mainColumn без vertical gap (MainBottom владеет 10px ресайз-ручкой сверху).

Z-шкала: base 1 · resize-handle 5 · dropdown 100 · sticky 200 · overlay 1000 · modal 9999 · toast-lower 10000 · titlebar-popover 10001 · toast 99998 · tooltip 99999.

## 7. Переключение темы

- themeChoice: dark|light|system, персист localStorage `kamin.themeChoice` (в GPUI → свой конфиг-стор), default dark.
- systemDark: из nativeTheme ОС (не prefers-color-scheme). В GPUI: системная тема Windows напрямую.
- contributedThemeType переопределяет resolvedTheme (light-VSIX-тема форсит light-базу до наложения override'ов).
- Мгновенный пейнт на старте: кэш последней contributed-темы (localStorage `kamin.contributedThemeData`) применяется ДО загрузки расширений. В GPUI: сериализовать резолвнутую палитру в конфиг и применять на старте.
- Смена темы пушит палитру внешним тостам (external-toast) — сохранить канал.

## 8. Contributed themes (VSIX перекрашивает весь IDE)

Портировать резолвер `contributed-theme-resolve.ts` в Rust 1:1:
- Две семьи переменных: raw `--vscode-*` (для вебвью расширений — остаётся в вебвью-слое) + резолвнутые chrome-токены (KAMIN_TOKEN_MAP: первый существующий author-key).
- Elevation ramp из NEUTRAL_SURFACE_KEYS: фильтр chroma ≤ 0.25, только opaque, сортировка по lightness; darkest → bg-sidebar; panel — шаг от editor; caps: SURFACE_MAX_STEP 0.09, OVERLAY_MAX_STEP 0.13.
- Accent = самый насыщенный из ACCENT_CANDIDATES (activityBarBadge, progressBar, button.background, focusBorder…).
- Синтез отсутствующих: muted = fg→bg blend t=0.42, disabled t=0.62, divider t=0.16.
- uiTheme из package.json авторитетен для dark/light (vs-dark/hc-black → dark), НЕ поле type из JSON темы.
- Hex sanitize (для строгих потребителей): `^#([0-9a-fA-F]+)$`, #rgb/#rgba → дублирование, только 6/8 цифр, иначе drop.
- Дебаунс снятия темы при реинсталле VSIX (REGISTRY_SETTLE_MS 2500).

## 9. Анимации

- Токены: fast 150ms ease, normal 250ms ease.
- prefers-reduced-motion: всё в 0.01ms, КРОМЕ спиннеров (1.1s infinite — чтобы замерший спиннер не читался как зависание).
- Каталог keyframes для порта: toast slide-in (translateX 120%→0 + opacity) + shrink (прогресс-полоска); модалки fadeIn/fade/qpFade; скелетоны стриминга kaminSkShimmer (sweep), kaminSwitchBreathe/Float/Sweep; спиннер kaminWvSpin (rotate 360); tab-switching/SessionTab; treeFlash (подсветка в дереве); bridgeWorkingPulse (working-состояние сессии).
- 113 использований transition/animation в 50 файлах — при сверке компонентов (40-components) помечать анимируемые состояния.

## Чеклист паритета (тема)
- [ ] Палитры Dark/Light как статические структы — все таблицы выше
- [ ] Предвычисление всех color-mix тинтов
- [ ] Glint-border рецепт + радиальный градиент фона
- [ ] Радиусы/тени/ховер/фокус/скроллбары
- [ ] Bricolage + JetBrains Mono + codicons + FA + Phosphor + Catppuccin
- [ ] Размеры регионов и z-порядок
- [ ] theme switching (dark/light/system + contributed override + мгновенный пейнт)
- [ ] Rust-порт contributed-theme-resolve (ramp/accent/синтез/sanitize)
- [ ] Анимации по каталогу + reduced-motion поведение
