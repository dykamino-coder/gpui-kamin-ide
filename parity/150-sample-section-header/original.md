# 150 sample-section-header — оригинал
Файлы: kamin-ide/src/renderer/components/main/component-samples-extra.tsx:161-178 (inline-стили; общего класса нет — живой стиль в SessionsMode.module.css и CustomizeMode.module.css по отдельности)

## Содержание/структура
`SectionHeaderRow()` в Block «Section header», hint: «Sidebar landmark — uppercase, muted, 0.08em letter-spacing.»
Один `<div>` с текстом «SECTION» и inline-стилем — рецепт лендмарков PROJECTS / CUSTOMIZE в сайдбаре.

## Метрики
Inline-стиль (точный):
```
padding: 8px 12px;
font-size: var(--fs-xs);        /* 11px */
font-weight: 500;
letter-spacing: 0.08em;
color: var(--text-muted);       /* #838aa0 dark */
font-feature-settings: 'ss01';
```

## Состояния/варианты
Статичный, интерактива нет. Текст в разметке уже uppercase («SECTION») — CSS text-transform не используется.
