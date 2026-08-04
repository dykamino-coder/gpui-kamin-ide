# 12 layout-presets-section — оригинал

Файлы:
- %PROJECTS%\kamin-ide\src\renderer\components\titlebar\LayoutPresetsSection.tsx:98-167
- %PROJECTS%\kamin-ide\src\renderer\components\titlebar\LayoutToggles.module.css:56-62,64-89,113-126,135-192 (общий css с меню)

## JSX-структура (кратко, вложенность)
```
<>                                        // внутри <ul class=menu> элемента 11
  <li class=menuLabel>Layouts</li>
  <li><button class=menuItem> codicon-save             + "Save current layout…"</button></li>
  <li><button class=menuItem> codicon-desktop-download + "Export current layout…"</button></li>
  <li><button class=menuItem> codicon-cloud-upload     + "Import layout…"</button></li>
  {presets.length===0 && <li class=presetEmpty>No saved layouts yet</li>}
  ×N <li class=presetRow onContextMenu=rename>
       <button class=presetApply data-tooltip="Apply this layout · right-click to rename">
         <span class=itemIcon><i class="codicon codicon-layout"></span>
         <span class=itemLabel>{name}</span>
       </button>
       <button class=presetIconBtn> codicon-save-as </button>            // overwrite
       <button class=presetIconBtn> codicon-desktop-download </button>   // export
       <button class=presetIconBtn aria-pressed={default}> codicon-star-full|star-empty </button>
       <button class=presetIconBtn> codicon-trash </button>
     </li>
</>
```

## Метрики (ИЗ CSS)
.menuLabel, .menuItem, .itemIcon, .itemLabel — как в элементе 11 (тот же css).

.presetEmpty:
- padding: var(--space-1) var(--space-3); font-size: var(--fs-xs); color: var(--text-muted)

.presetRow:
- display:flex; align-items:center; gap: 1px

.presetApply:
- flex: 1; min-width: 0; padding: var(--space-2) var(--space-3); gap: var(--space-2)
- background: transparent; border: none; border-radius: var(--radius-sm)
- color: var(--text-primary); font: inherit; font-size: var(--fs-sm); text-align: left; cursor: pointer
- display:flex; align-items:center
- hover: background: color-mix(in srgb, var(--text-primary) 10%, transparent)
- `.presetApply .itemLabel`: overflow:hidden; text-overflow:ellipsis; white-space:nowrap

.presetIconBtn:
- размеры: width: 26px; height: 26px; flex-shrink: 0
- display:grid; place-items:center; background: transparent; border: none
- border-radius: var(--radius-sm); color: var(--text-muted); cursor: pointer
- `> i { font-size: 13px; line-height: 1; }`
- hover: background: color-mix(in srgb, var(--text-primary) 10%, transparent); color: var(--text-primary)
- transition: background var(--transition-fast), color var(--transition-fast)

## Состояния
- `.presetIconBtn[aria-pressed="true"]` (star = default-пресет): color: var(--accent-primary); иконка codicon-star-full (иначе star-empty)
- пустой список: строка .presetEmpty
- right-click по .presetRow → rename-prompt (стилей не меняет)
