
## Цикл 3: MATCH

Ховер строк actions (bg-surface 60% + text-primary, иконка красится) 1:1.

## Цикл 4: DIVERGES

`Sidebar.tsx:81-85` диспатчит ТЕЛО по активному тулу слота (иначе placeholder «No tool selected»); у нас всегда `sessions_sidebar`, а `sidebar_activity` влияет только на подсветку плитки. Волна 8.

## Цикл 8: DIVERGES

Диспатч тела по активному тулу — **закрыто**. **Волной 15 закрыты и два остатка**: единый источник истины (активный тул берётся из восстановленной модели, а не хардкодом) и ветка `projects` в `tool_body` (Projects теперь работает в любом слоте).

## Цикл 9: MATCH

Оба остатка закрыты: ветка `"projects"` в `tool_body` (`root.rs:3636`), `sidebar_activity: restored_sidebar_tool` из модели слоя (`root.rs:437`), не хардкод.

## Цикл 16: MATCH

Резолвер тела; плейсхолдер `circle-large` / «No tool selected»; ветка Customize берётся на уровне сайдбара, как `Sidebar.tsx:63`.

## Цикл 20: MATCH

Резолвер тела сайдбара: Customize → нав, `projects` → сессии, иначе тело тула с плейсхолдером «No tool selected».
