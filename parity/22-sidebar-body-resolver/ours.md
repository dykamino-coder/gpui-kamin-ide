# 22 sidebar-body-resolver — наша реализация
Файлы: `crates\shell\src\root.rs:5253-5276` (выбор тела), `crates\shell\src\root.rs:133-135,365,729-733` (sidebar_activity)

## Структура (gpui-дерево кратко)
```
gap_wrap(
  if customize_open { customize_nav(...) }   ← элементы 35-37
  else              { sessions_sidebar(...) } ← элементы 23-32
)
```
`sidebar_activity: &'static str` хранится в RootView (дефолт `"projects"`, меняется `ShellEvent::ActivityClicked`), но **телом сайдбара не управляет** — используется только для подсветки плитки в activity-bar (root.rs:5222).

## Метрики (из кода, точные)
- Собственных стилей нет (как и в оригинале — чисто логический выбор).

## Отличия от original.md той же папки
1. **Резолвер по активной активности НЕ РЕАЛИЗОВАН**: оригинал — `getPanelSignal("sidebar").active` → `<ActivityBody id slot="sidebar">`; у нас тело сайдбара всегда `sessions_sidebar` (либо customize_nav), какая бы плитка ни была кликнута.
2. **Фоллбек `ActivityPlaceholder("No tool selected", icon circle-large)` НЕ РЕАЛИЗОВАН** — состояния «активность не выбрана» в сайдбаре нет.
3. Для не-`projects` активностей (tree/terminal и др. в слоте sidebar) тел нет — оригинал резолвит их через общий `ActivityBody`.

## Дополнение атрибутов (цикл 10)

- отступы: N/A: отступы — резолвер выбирает ТЕЛО (sessions/tool_body/placeholder) и своего бокса не рисует (`crates/shell/src/root.rs`, ветка `sidebar_mode`); паддинги принадлежат выбранному телу
- гэпы: N/A: гэпы — у резолвера один ребёнок
- цвета: N/A: цвета — своей поверхности нет, фон и текст берёт выбранное тело (карта bg-mantle #262533 / bg-sidebar #1d1d28)
