# 159 legacy-app-shell-css — оригинал
Файлы: kamin-ide/src/renderer/App.module.css:1-22

## Содержание/структура
МЁРТВЫЙ файл: классы `.app`, `.workbench`, `.center` нигде не импортируются (по INVENTORY). В порт gpui не нужен — фиксируется только для полноты.

## Метрики
Полное содержимое файла:
```css
.app {
  display: flex;
  flex-direction: column;
  height: 100vh;
  width: 100vw;
  background: var(--bg-primary);
  color: var(--text-primary);
  overflow: hidden;
}

.workbench {
  flex: 1;
  display: flex;
  min-height: 0;
}

.center {
  flex: 1;
  display: flex;
  flex-direction: column;
  min-width: 0;
}
```

## Состояния/варианты
Нет. Кандидат на удаление в исходном репо; в gpui-порт не переносится.

## Дополнение атрибутов (цикл 10)

- отступы: N/A: отступы — в `App.module.css` нет ни padding, ни margin: только display/flex-direction, height 100vh / width 100vw, flex 1, min-height 0, min-width 0, overflow hidden (App.module.css:1-22); обнуление приходит глобально из `* { margin: 0; padding: 0 }` (global.css:12)
