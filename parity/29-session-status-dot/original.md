# 29 session-status-dot — оригинал
Файлы: `%PROJECTS%\kamin-ide\src\renderer\components\sidebar\SessionItem.tsx` (29-38, 102-107), `SessionItem.module.css`

## JSX-структура (кратко, вложенность)
```
<span .dot data-bridge={bridgeStatus} data-tooltip={statusTip} aria-label={statusTip}/>
```
Источник: `session.metadata.bridgeStatus` / `bridgeWorking` (пишет Claude Bridge VSIX). `bridgeWorking === true` → статус `"working"` (приоритет над bridgeStatus). Тултипы: working→«Working…», connected→«Online», connecting→«Connecting…», error→«Error», disconnected→«Offline», иначе — без тултипа.

## Метрики (ИЗ CSS, точные значения)
- `.dot` (база):
  - `flex-shrink: 0; width: 4px; height: 4px; border-radius: 50%`
  - `background: var(--text-muted)` (серый — сессия без статуса/инактивная)
- `.active .dot`: `background: var(--tab-color)` (цветной только у selected-строки)

## Состояния (классы-варианты с метриками)
Селекторы `.row .dot[data-bridge=…]` (префикс `.row`, чтобы победить `.active .dot`):
- `[data-bridge="connected"]`: `background: var(--accent-green, #3fb950)`
- `[data-bridge="connecting"]`: `background: var(--accent-yellow, #d29922)`
- `[data-bridge="error"]`: `background: var(--accent-red, #f85149)`
- `[data-bridge="disconnected"]`: `background: var(--text-muted)`
- `[data-bridge="working"]`:
  - `width: 6px; height: 6px`
  - `background: var(--accent-blue, #58a6ff)`
  - `animation: bridgeWorkingPulse 1.1s ease-in-out infinite`
- `@keyframes bridgeWorkingPulse`: `0%,100% { opacity: 0.5; transform: scale(1) }` / `50% { opacity: 1; transform: scale(1.5) }`

## Дополнение атрибутов (цикл 10)

- отступы: у `.dot` padding/margin НЕТ (`sidebar/SessionItem.module.css:53-59`) — только `flex-shrink: 0`, бокс 4×4 (`:55-56`), в состоянии `working` 6×6 (`:69-70`); внешние отступы задаёт строка-родитель: `.row { gap: var(--space-2) }` = 8 и `.row { padding: 0 8px 0 16px }` (`SessionItem.module.css:7,11`), то есть точка стоит на 16px от левого края строки и в 8px от лейбла
