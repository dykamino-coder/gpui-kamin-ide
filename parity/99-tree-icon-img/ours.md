# 99 tree-icon-img — наша реализация
Файлы: `crates/shell/src/icon_theme.rs:119-138` (`file_img`/`folder_img`), `:37-109` (резолв contributed icon-темы), `crates/shell/src/cat_icons.rs:2489+, 4759+` (Catppuccin-маппинг ext/name/folder, сгенерирован), `crates/shell/src/ui/file_list.rs:281-289` (бокс 16×16)

## Структура (gpui-дерево кратко)
```
gpui::img(src) .flex_shrink_0 .w(16) .h(16)     // бокс задаёт вызывающая сторона
src:
  ACTIVE contributed icon-theme (kamin:iconTheme:load, iconPath абсолютные)
    → resolve_file: fileNames → цепочка суффиксов после каждой точки → file-дефолт
    → resolve_folder: folderNames(Expanded) → folder/folderExpanded (взаимный фолбэк)
  иначе → cat_icons::file_icon(name) | folder_icon(name, open)  // embedded SVG-ассеты
```
Тема — глобальный `static ACTIVE: Mutex<Option<IconTheme>>`; SVG contributed-темы читаются gpui напрямую с диска.

## Метрики (из кода, точные)
- 16×16, flex_shrink_0 (у вызывающего) ✓; сами SVG несут цвета Catppuccin.
- fontCharacter-дефиниции тем не поддержаны → фолбэк на Catppuccin (гэп совпадает с оригиналом, plan/25).

## Отличия от original.md той же папки
1. **Порядок резолва инвертирован без визуального дефекта**: у нас contributed-тема резолвится СИНХРОННО первой (SVG с диска), фолбэк Catppuccin; в оригинале синхронно Catppuccin + асинхронный апгрейд до темы. «Мигания» нет в обоих, но у нас нет промежуточного кадра Catppuccin.
2. **Light-фильтр НЕ реализован**: `[data-theme="light"] .img { filter: saturate(3.2) brightness(0.7) }` — в light-теме Catppuccin-пастель останется блеклой на светлых панелях.
3. **`isRoot` не поддержан** — карты `rootFolder*` contributed-темы игнорируются (корень получает обычную folder-иконку).
4. Резолв-порядок расширений: у нас цепочка суффиксов слева направо после каждой точки (длинный суффикс первым — как VS Code) — совпадает с оригиналом; регистронезависимость ✓.

## Дополнение атрибутов (цикл 10)

- цвета: `file_img`/`folder_img` отдают `gpui::img` без тонирования (`icon_theme.rs:119-138`) — цвет внутри Catppuccin-SVG (`cat_icons.rs`), currentColor строки (text_secondary #adb3c7 dark / #524c43 light, `file_list.rs:221`, `palette.rs:64,102`) на картинку не влияет; светлотемного фильтра `saturate(3.2) brightness(0.7)` у нас НЕТ (grep по `crates/shell/src` пуст) — в light-теме иконки бледнее оригинала на панели bg_mantle #fbf7f4 (`palette.rs:93`).
- отступы: у иконки padding/margin нет — только фикс-бокс 16×16 (`file_list.rs:332-334`); зазор до имени даёт строка `gap 6` (`file_list.rs:211`), правый край `pr SPACE_2 8` (`:214`), отступ уровня `pl = depth*12 + 8` (`:213`).
