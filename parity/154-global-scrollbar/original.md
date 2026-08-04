# 154 global-scrollbar — оригинал
Файлы: kamin-ide/src/renderer/theme/global.css:25-29; kamin-ide/src/renderer/theme/skeleton.css:20-23 (вебвью-вариант)

## Содержание/структура
Сквозной стиль webkit-скроллбара для всего renderer-документа; skeleton.css дублирует его для webview-страниц.

## Метрики
Полные правила (global.css:25-29):
```css
::-webkit-scrollbar { width: 8px; height: 8px; }
::-webkit-scrollbar-track { background: transparent; }
::-webkit-scrollbar-thumb { background: var(--bg-overlay); border-radius: var(--radius-xs); }
::-webkit-scrollbar-thumb:hover { background: var(--text-disabled); }
::-webkit-scrollbar-corner { background: transparent; }
```
skeleton.css:20-23 (отличия: radius захардкожен 4px, нет правила corner):
```css
::-webkit-scrollbar { width: 8px; height: 8px; }
::-webkit-scrollbar-track { background: transparent; }
::-webkit-scrollbar-thumb { background: var(--bg-overlay); border-radius: 4px; }
::-webkit-scrollbar-thumb:hover { background: var(--text-disabled); }
```
Значения (dark): толщина 8×8px; трек и corner прозрачные; thumb `--bg-overlay` #515567, radius 4px; thumb hover `--text-disabled` #60667b.

## Состояния/варианты
default / thumb hover. Исключение по месту: `.tabs` в BottomTabBar прячет скроллбар (`scrollbar-width: none`).

## Дополнение атрибутов (цикл 10)

- отступы: N/A: отступы — псевдоэлементы скроллбара задают только width/height 8px, background и border-radius; ни padding, ни margin в правилах нет (global.css:25-29, skeleton.css:20-23); глобальный сброс `* { margin: 0; padding: 0; box-sizing: border-box }` (global.css:12)
