# 154 global-scrollbar — наша реализация

Файлы: `crates/shell/src/theme_sync.rs` (блок скроллбаров), рендер — вендорный
`gpui-component` (`vendor/gpui-component/src/scroll/scrollbar.rs`).

> Досье переписано в цикле 13: прежний текст утверждал режим `Hover`
> («в покое скрыт»), thumb с alpha .35 и hover .5 — в коде ничего этого нет
> (ревью зоны 130-159).

## Значения (из кода, точные)
- `theme.colors.scrollbar` — **прозрачный** (трек не красится), как
  `::-webkit-scrollbar-track { background: transparent }`.
- `theme.colors.scrollbar_thumb` — `bg_overlay` **#515567**, СПЛОШНОЙ.
- `theme.colors.scrollbar_thumb_hover` — `text_disabled` **#60667b**.
- `theme.scrollbar_show` — **`ScrollbarShow::Always`** (постоянно видим, как
  у оригинала: `::-webkit-scrollbar { width: 8px }` без hover-гейта).
- Геометрия из вендора при `Always`: ширина **8**, радиус **4**, инсет 4
  внутри 16-px полосы.

## Отличия от original.md той же папки
1. Скроллбар per-container (у каждого прокручиваемого узла свой), а в браузере
   правило глобальное для всех элементов страницы.
2. Вебвью-копия правил (`skeleton.css`) в порт не переносится — внутри
   вебвью работает его собственный CSS.
3. Thumb лежит в 16-px полосе с инсетом 4, у оригинала он вплотную к краю.

## Атрибуты
- цвета: трек прозрачный, thumb bg-overlay #515567, ховер text-disabled #60667b
- скругления: 4 (капсула при ширине 8)
- отступы: инсет 4 внутри полосы 16
- шрифты, гапы: N/A — у скроллбара их нет
- ховер: thumb меняет цвет на text-disabled; трек не реагирует
