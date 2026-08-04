# 120 status-version-update — наша реализация
Файлы: %PROJECTS%\gpui-kamin-ide\crates\shell\src\ui\status_bar.rs:216-241 (бренд), 275-307 (update-пилюля), 175 и 308 (взаимоисключение по `has_update`); crates\shell\src\root.rs:1448-1450 (`UpdateAvailable`), 2296-2330 (`CheckForUpdates` → host RPC `kamin:updater:check` → тост), 6414-6422 (передача `env!("CARGO_PKG_VERSION")`)

## Структура/содержание
ДВА взаимоисключающих состояния вместо трёх (нет `downloading`):
```
1) update_available → div#update-pill: flex.items_center.gap(3).px(8).py(1).rounded(4)
   ├─ codicon cloud-download "\u{ea9a}" 12
   └─ «Update {ver}»
   tooltip «Update to KaminIDE {ver} — you have {version}»
   клик → cmd /c start "" {url}  (внешний браузер)
2) иначе → div#status-brand: flex.items_center.px(8).rounded(4)
   └─ «KaminIDE {version}»
   tooltip «Check for updates»
   клик → ShellEvent::CheckForUpdates → kamin:updater:check → тост
          «Update available: KaminIDE {v}» / «You are up to date» / «Update check failed: {e}»
```
Взаимоисключение: `.when(!has_update, |row| row.child(brand))` (status_bar.rs:308). Порядок правой группы: contributed → UTF-8/EOL → update | brand.

## Метрики (из кода, точные)
- отступы: update-пилюля px 8 (SPACE_2) + py 1; бренд px 8 (SPACE_2), py нет; высота обоих 24 (растяжка по бару)
- гэпы: update-пилюля gap 3 (глиф ↔ текст); между элементами правой группы gap 2
- цвета: update — bg p.accent_primary #89b4fa α 0.22, текст p.accent_primary #89b4fa; бренд — текст p.accent_primary #89b4fa
- скругления: обе пилюли rounded 4 (RADIUS_XS)
- шрифты: update — font-size 11 (наследует FS_XS от бара), font-weight 600 SEMIBOLD, глиф codicon 12; бренд — font-size 11 (FS_XS, задан явно), font-weight 500 MEDIUM
- фоны по ховеру: update — p.accent_primary α 0.34; бренд — p.bg_surface #3d3f51 α 0.6

## Отличия от original.md той же папки
1. Состояние `downloading` НЕ РЕАЛИЗОВАНО целиком: `role=progressbar` + `aria-valuenow`, `.progressFill` (accent-primary 32%, absolute, `transition: width 120ms linear`, indeterminate = width 100% / opacity 0.5), `.progressLabel` (gap 6) и тексты «Updating {pct}%» / «Updating {N.n} MB». Скачивание уходит во внешний браузер (`cmd /c start`), прогресса внутри приложения нет.
2. Бренд КЛИКАБЕЛЕН: cursor pointer, hover `bg-surface 60%`, тултип «Check for updates» — совпадает с оригиналом. Результат проверки показывается тостом, а не переходом item'а в состояние «update available» тем же кликом.
3. Бренд и update-пилюля взаимоисключающие — совпадает с оригиналом.
4. Совпадают: fill accent 22%, hover accent 34%, weight 600, radius-xs, глиф cloud-download 12, текст «Update {ver}», тултип «Update to KaminIDE {v} — you have {cur}», `.brand` weight 500 + accent-primary.
5. У update-пилюли добавлен `py 1` (в оригинале `.item { padding: 0 var(--space-2) }`) и gap 3 вместо 4.
6. Версия — `env!("CARGO_PKG_VERSION")` на билд-тайме; фоллбека `version || "0.0.1"` нет.
7. `.update:hover` в оригинале ещё и фиксирует `color: accent-primary` (чтобы generic `.item:hover` не перебил) — у нас generic-hover'а на этом элементе нет, поведение совпадает без явного правила.
