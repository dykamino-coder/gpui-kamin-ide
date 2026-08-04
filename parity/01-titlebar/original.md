# 01 Titlebar — оригинал (KaminIDE 0.2.87, host renderer)

Файлы: src/renderer/components/titlebar/Titlebar.tsx (+ .module.css),
TitlebarButton.*, TitlebarQuickActions.*, LayoutToggles.*, ThemeQuickToggle.*,
PanelIcon.tsx, LayoutPresetsSection.tsx.

## Структура (Titlebar.tsx)
.titlebar (flex row, height var(--layout-titlebar-height)=42px, bg transparent,
drag-region, fs var(--fs-sm)=12px, color var(--text-muted))
 ├ .brand 42×42 (flex center, no-drag, color var(--accent-primary));
 │   .brandLogo 26×26 (лого-марка), codicon 18px
 ├ .leftCluster (flex, ширина = сайдбар, flex-shrink 0, overflow hidden, h 100%)
 ├ .tabsSlot (flex:1, min-width:0) — таб-стрип сессий
 └ правый кластер: quick-actions (поиск-команда, layout-toggles, theme toggle,
   DevTools), затем window controls (min/max/close)

## Computed (живой прод, ВНИМАНИЕ: снято в contributed-теме GitHub-dark)
titlebar: height 42px; font 12px "Bricolage Grotesque Variable"; weight 400;
color = var(--text-muted); bg transparent; flex; align-items center.
brand: 42×42; flex center; color var(--accent-primary).

## Тема-независимые метрики (сверять)
- высота 42px; brand-слот 42×42; лого 26×26; codicon 18px
- fs 12px (fs-sm), font Bricolage Grotesque, weight 400
- leftCluster ширина = ширине сайдбара; tabsSlot flex:1 min-width:0
- корневые токены (дефолт-тема сверяется по kamin_theme::DARK):
  fs-xs 11 / fs-sm 12 / fs-md 13; radius xs4 sm8 md12 lg16; space 4/8/12/16

## Скрин
original.png (contributed-тема; при цветовой сверке переключить прод в
дефолтную тёмную тему или сверять цвета по палитре, не по пикселям скрина)

## Метрики .titlebar (ИЗ CSS, Titlebar.module.css:5-16 — дополнение)
- размеры: height: var(--layout-titlebar-height); width — нет (flex-строка на всю ширину)
- отступы: нет padding/margin/gap на корне
- скругления: нет
- шрифт: font-size: var(--fs-sm); family/weight/letter-spacing не заданы (наследуются)
- цвета: color: var(--text-muted); background: transparent
- hover/active/focus: нет (корень не интерактивен)
- transition/анимации: нет
- позиционирование: display:flex; align-items:center; position:relative;
  z-index: var(--z-toast-lower); flex-shrink:0; -webkit-app-region: drag

## Состояния
Нет вариантных классов у корня. Drag-region сплошной; no-drag выставляют
дети (.brand, .tabsSlot, .controls, кнопки).

## Мёртвые классы в том же css (в JSX Titlebar.tsx не используются)
`.welcomeTab`, `.kbd` — присутствуют в Titlebar.module.css:66-117, не рендерятся.
