# 31 session-rename-input — оригинал
Файлы: `%PROJECTS%\kamin-ide\src\renderer\components\sidebar\SessionItem.tsx` (73-86), `SessionItem.module.css`

## JSX-структура (кратко, вложенность)
```
// при renamingSessionId === session.id вместо обычной строки:
<div .row[.active][.tinted][.inactive].editing style="--tab-color:…">
  <span .dot aria-hidden/>
  <input .input ref={inputRef} defaultValue={session.name}
         onKeyDown={Enter→commit; Escape→endRename}
         onBlur={commit}/>
</div>
```
`commit()` = `renameSession(id, input.value)` + `endRename()`. На входе в режим — `focus()` + `select()` (useEffect). Триггеры входа: dblclick по строке, F2, «Rename» в попапе/меню.

## Метрики (ИЗ CSS, точные значения)
- `.editing` (модификатор к `.row`): `background: var(--bg-surface)`
- `.input`:
  - `flex: 1; min-width: 0`
  - `background: var(--bg-base)`
  - `border: 1px solid var(--accent-primary); border-radius: var(--radius-xs)`
  - `color: var(--text-primary)`
  - `font: inherit; font-size: var(--fs-sm)`
  - `padding: 1px 4px`
  - `outline: none`
- Габариты контейнера — как у `.row` (height 24px, padding 0 8px 0 16px, gap var(--space-2)).

## Состояния (классы-варианты с метриками)
- Enter — commit; Escape — cancel (`endRename` без записи); blur — commit.
- hover/focus-стилей у `.input` сверх постоянной accent-рамки нет.
