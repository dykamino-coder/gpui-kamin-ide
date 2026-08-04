# 155 glint-surface-card-ring — оригинал
Файлы: kamin-ide/src/renderer/theme/global.css:88-101; токен `--glint-border` — theme/dark-theme.css:31 (фоллбек variables.css:25)

## Содержание/структура
Фирменный вид «плавающей карточки»: fill `--bg-mantle` + диагональная подсвеченная рамка. Единственный источник рецепта — panel-модули подключают через `composes: glint-surface from global;` (карточки FilePanel / RightPanel / MainContent / MainBottomPanel + рамка sidebar-вебвью); обычный DOM добавляет класс напрямую.

## Метрики
Полное правило (global.css:96-101):
```css
.glint-surface {
  border: 1px solid transparent;
  background:
    linear-gradient(var(--bg-mantle), var(--bg-mantle)) padding-box,
    var(--glint-border) border-box;
}
```
Токен (dark-theme.css:31):
```css
--glint-border: linear-gradient(135deg, rgba(255, 255, 255, 0.18) 0%, var(--bg-mantle) 22%, var(--bg-mantle) 78%, rgba(255, 255, 255, 0.18) 100%);
```
:root-фоллбек в variables.css:25 отличается: mid-стопы `var(--bg-base)` вместо `var(--bg-mantle)`.
Механика: рамка 1px transparent; двухслойный background — сплошной `--bg-mantle` в padding-box, градиент в border-box → рамка видна только в 1px-кольце. Mid-стопы = цвету панели, поэтому рамка «тает» в заливку и читается только диагональный блик (0% и 100% — rgba(255,255,255,0.18)). Значения dark: `--bg-mantle` #262533.

## Состояния/варианты
Состояний нет. Тема меняет `--glint-border` (white-tinted на dark, warm-ink на светлой) — рецепт один.
