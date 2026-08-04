# 73 contributed-container-body — оригинал
Файлы: `src/renderer/components/activity-bodies/ContributedContainerBody.tsx` (30-38 — `ContributedContainerBody`, 43-48 — `ContributedViewBody`), `src/renderer/components/activity-bodies/ContributedContainerBody.module.css`

## JSX-структура (кратко, вложенность)
```
ContributedContainerBody({containerId})
├─ views = registry.views.filter(containerId)
├─ если views.length === 0 → <ActivityPlaceholder icon="circle-large" label="No views" />
└─ div.root
   └─ views.map → <ViewSection viewId name type />   (см. 74)

ContributedViewBody({viewId, flush})  — одиночный view без хедера (Customize)
├─ chat без сессии (viewId===CHAT_VIEW_ID && openSessions.length===0) → <WelcomePlaceholder />
├─ view не найден → <ActivityPlaceholder icon="circle-large" label="No view" />
└─ type==="webview" ? <WebviewViewBody viewId flush /> : <TreeViewBody viewId />
```

## Метрики (ИЗ CSS, точные значения)
- `.root`: `display:flex; flex-direction:column; height:100%; min-height:0`
- Отступов/padding/margin/border-radius у `.root` нет; шрифт/цвет не задаются (наследуются)
- hover/active/focus — нет; transition/анимаций — нет; позиционирование — обычный поток

## Состояния (классы-варианты с метриками)
- Пустой контейнер (0 views) → рендерится `ActivityPlaceholder` (элемент 69), не `.root`
- Chat view без открытых сессий → полный `WelcomePlaceholder` (элемент 77) вместо тела

## Дополнение атрибутов (цикл 10)

- цвета: собственных фонов у `.root`/`.view`/`.frame` нет (`ContributedContainerBody.module.css:1-14,52-61`) — просвечивает карта `--bg-mantle` #262533 dark / #fbf7f4 light (`dark-theme.css:12`, `light-theme.css:25`); заголовок вью `.title { color: var(--text-muted) }` #838aa0 / #6e685d (`:23`; `dark-theme.css:37`, `light-theme.css:47`); `.viewBadge { background: var(--accent-primary); color: var(--bg-base) }` = #89b4fa на #313240 dark / #da8343 на #fbf8f1 light (`:41-42`; `light-theme.css:24,109`).
