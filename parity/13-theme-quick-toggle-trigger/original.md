# 13 theme-quick-toggle-trigger — оригинал

Файлы:
- %PROJECTS%\kamin-ide\src\renderer\components\titlebar\ThemeQuickToggle.tsx:51-66
- %PROJECTS%\kamin-ide\src\renderer\components\titlebar\ThemeQuickToggle.module.css:1-25

## JSX-структура (кратко, вложенность)
```
<div class=root ref>                       // relative-обёртка, outside-click / blur / Esc
  <button type=button class=trigger data-tooltip="Appearance — themes & icons"
          aria-label="Appearance — themes & icons" aria-haspopup=dialog aria-expanded={open}>
    <i class="fas {fa-circle-half-stroke | fa-sun | fa-moon}" aria-hidden />
  </button>
  {open && <Menu />}                       // элемент 14
</div>
```
Логика иконки: contributed-тема light → fa-sun, dark → fa-moon; без contributed:
choice "system" → fa-circle-half-stroke, иначе по resolvedTheme (light → fa-sun, dark → fa-moon).

## Метрики (ИЗ CSS)
.root:
- position: relative; display:inline-flex; align-items:center; -webkit-app-region: no-drag

.trigger:
- размеры: width: 28px; height: 28px
- отступы: не заданы
- скругления: border-radius: var(--radius-sm)
- шрифт: `.trigger > i { font-size: 12px; line-height: 1; }`
- цвета: background: transparent; color: var(--text-muted)
- hover: background: var(--bg-surface); color: var(--text-primary)
- transition: background var(--transition-fast), color var(--transition-fast)
- позиционирование: display:inline-flex; align-items:center; justify-content:center

## Состояния
Вариантных классов нет; aria-expanded меняется, но css-правила на него в этом модуле нет.
