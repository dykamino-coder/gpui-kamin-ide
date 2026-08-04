# 83 legacy-bridge-card — наша реализация
Файлы: `crates/shell/src/ui/customize.rs:238-343` (`legacy_bridge_card`), `crates/shell/src/legacy_bridge.rs` (детект footprint + `uninstall_electron_bridge`), `root.rs` (`ModalAction::RemoveLegacyBridge`)

## Структура (gpui-дерево кратко)
```
None при !fp.found, иначе:
div .card  (flex, items_start, gap 12, p 12, r 12, bg-surface, border divider-soft)
├─ div .icon 32×32, r 8, accent-primary, fa-box-archive 16
├─ div .body (flex_1, min_w 0)
│   ├─ «Legacy Electron Bridge detected» — fs 13 / 600 / text-primary
│   └─ desc — mt 4, fs 12, lh 1.5, text-muted; перечисление найденного
└─ обёртка h_full items_center → кнопка «Remove old Bridge»
```
Клик → ConfirmModal (danger, «Remove it») → `ModalAction::RemoveLegacyBridge` → реимпорт сессий → `uninstall_electron_bridge` → re-detect.

## Метрики (из кода, точные)
- `.card`: gap SPACE_3 12, padding 12, radius RADIUS_MD 12, bg `--bg-surface`, рамка 1px divider-soft (text-primary 6%).
- `.icon`: 32×32, radius RADIUS_SM 8, цвет accent-primary, глиф FontAwesome 16.
- `.title`: fs FS_MD 13, weight 600, text-primary. `.desc`: mt SPACE_1 4, fs FS_SM 12, line-height 1.5, text-muted.
- `.remove`: px SPACE_3 12 / py SPACE_1 4, radius 8, рамка 1px accent-red, текст accent-red fs 12 / 600; hover — заливка accent-red + белый текст. `align-self: center` сделан обёрткой (у `Stateful` нет `self_center()`).

## Отличия от original.md той же папки
1. Нет состояния `busy`: кнопка не блокируется и не меняет текст на «Removing…» (opacity .6) на время удаления — удаление уходит в поток без флага в состоянии.
2. Нет CSS-перехода 0.12s (в gpui нет transition).
