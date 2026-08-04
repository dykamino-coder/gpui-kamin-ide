# 02 titlebar-left-cluster — наша реализация

Файлы: crates/shell/src/ui/titlebar.rs (`.leftCluster` — обёртка brand +
quick-actions), crates/shell/src/root.rs (передача `sidebar_width`).

## Структура (gpui-дерево кратко)
```
div.left-cluster (h_full, flex, items_center, flex_shrink_0, overflow_hidden,
                  w = sidebar_width при видимом сайдбаре ИЛИ Customize)
  div.brand 42×42 (лого kaminoid 26)
  div.quick-actions (gap 1, px 8)
```

## Метрики (из кода, точные)
- ширина: `w(state.sidebar_width)` при `sidebar_visible || customize_open`,
  иначе не задаётся (= auto по контенту)
- высота 100 %, `flex_shrink_0`, `overflow_hidden`, своих отступов нет
- собственных фона и цвета нет — наследуются от титлбара

## Отличия от original.md той же папки
Нет: обёртка, пиннинг к ширине сайдбара и режим Customize воспроизведены
(ц.35). Замер `probe metric left-cluster` при сайдбаре 256 физ. px —
204.8 логических, стрип начинается на 216.8 (кластер + padding слота 12).
