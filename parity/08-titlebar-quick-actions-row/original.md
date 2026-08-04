# 08 titlebar-quick-actions-row — оригинал

Файлы:
- %PROJECTS%\kamin-ide\src\renderer\components\titlebar\TitlebarQuickActions.tsx:27-51
- %PROJECTS%\kamin-ide\src\renderer\components\titlebar\TitlebarQuickActions.module.css:1-7,35-40

## JSX-структура (кратко, вложенность)
```
<div class=row>
  <ActionBtn title={"Hide sidebar"|"Show sidebar"} active={sidebarVisible}>
    <PanelIcon slot="left" />          // элемент 17
  </ActionBtn>
  {sidebar скрыт && (
    <span class=divider aria-hidden />
    <ActionBtn title={"Close Customize"|"Customize"} active={customizeMode}>
      <i class="fas fa-gear" />
    </ActionBtn>
  )}
</div>
```
(ActionBtn — элемент 09)

## Метрики (ИЗ CSS)
.row:
- размеры: не заданы (по контенту)
- отступы: gap: 1px; padding: 0 var(--space-2)
- скругления: нет
- шрифт: наследуется
- цвета: нет
- hover/active/focus: нет (на контейнере)
- transition: нет
- позиционирование: display:inline-flex; align-items:center; -webkit-app-region: no-drag

.divider:
- размеры: width: 1px; height: 14px
- отступы: margin: 0 var(--space-1)
- цвета: background: var(--bg-surface)

## Состояния
- Шестерёнка + divider рендерятся ТОЛЬКО при скрытом сайдбаре (`!sidebarVisible`).
- Тумблер сайдбара: active = сайдбар видим. Шестерёнка: active = sidebarMode === "customize".
