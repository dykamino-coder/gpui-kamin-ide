# 96 file-tree-children-states — оригинал
Файлы: `kamin-ide/src/renderer/components/file-tree/FileTreeView.tsx:196-222`, `kamin-ide/src/renderer/components/file-tree/FileTreeView.module.css`

## JSX-структура (кратко, вложенность)
```
{expanded && (
  div.children
  ├── {entries === null} → div.loading (style: paddingLeft = indentPx(depth+1)) "Loading…"
  ├── {entries.length === 0} → div.emptyChild (paddingLeft = indentPx(depth+1)) "(empty)"
  ├── entries.slice(0, childCap).map → <FolderNode> | <FileLeaf> (key = path, depth+1)
  └── {entries.length > childCap} → button.showMore (paddingLeft = indentPx(depth+1))
      ├── i.codicon.codicon-ellipsis (aria-hidden)
      └── "Show {min(rest, 200)} more ({rest} hidden)"
)}
```
- Кап: TREE_CHILD_CAP = 100, шаг TREE_CHILD_STEP = 200 (клик по showMore: childCap += 200).
- «Loading…» только при первом листинге (entries === null); при fsRev-рефреше старые entries остаются, спиннер — в chevron строки.
- indentPx(d) = d*12 + 8 px.

## Метрики (ИЗ CSS, точные значения)
`.children`: display: contents.

`.loading`, `.emptyChild` (общее правило):
- font-size: var(--fs-xs); color: var(--text-muted); padding: 2px 0
- padding-left — инлайн (см. выше)

`.showMore`:
- display: flex; align-items: center; gap: 6px; width: 100%
- border: none; background: none; font: inherit; font-size: var(--fs-xs)
- color: var(--text-muted); cursor: pointer; padding: 3px 0 (плюс инлайн padding-left); text-align: left

## Состояния (классы-варианты с метриками)
- `.showMore:hover`: color: var(--text-primary); background: color-mix(in srgb, var(--bg-surface) 55%, transparent)
- `.loading` / `.emptyChild` — статичны, без hover.
