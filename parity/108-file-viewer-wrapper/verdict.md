# 108 — verdict (review cycle 1)
VERDICT: DIVERGES
Нет .viewer (m 0/6/6, bg-mantle, r12) — общая glint-карта; .body 8/0/10+editor-bg
vs mx4/mt4/mb4; нет bodyFlush/retainLayer; сверх оригинала: breadcrumb h24 mono,
кнопка Save. Лимит 12 табов + эвикт — 1:1.

## Цикл 2: DIVERGES
Нет .viewer/.body (m 0/6/6 mantle r12; 8/0/10 editor-bg); +breadcrumb/Save сверх.

## Цикл 5: DIVERGES

Нет `.viewer` (m 0/6/6, bg-mantle, r12) и `.body` (8/0/10, editor-bg): у нас glint-карта с рамкой mx4/mt4/mb4. Сверх оригинала — breadcrumb h24 mono и кнопка «Save Ctrl+S». Нет bodyFlush/retainLayer.

## Цикл 6: DIVERGES

`.viewer`/`.body` не приведены; breadcrumb и «Save Ctrl+S» — сверх оригинала.

## Цикл 7: DIVERGES

Инсет карты: оригинал margin 0 6 6 + padding тела 8/0/10 против наших mx/mt/mb 4 без паддинга (замер: зазор 4.8 против 6). Сверх оригинала — breadcrumb 24 и кнопка Save.

## Цикл 15: DIVERGES

Осталось: обёртка `.viewer` (margin 0/6/6, bg-mantle, r-md), паддинги `.body` 8/0/10, `.bodyFlush`, `.retainLayer`; строка breadcrumb — сверх оригинала.

## Цикл 20: DIVERGES

Осталось: обёртка `.viewer` (margin 0/6/6, bg-mantle, r-md) — у нас инсеты 4.8/4.0; паддинги `.body` 8/0/10; breadcrumb-строка и кнопка «Save Ctrl+S» — сверх оригинала.

## Цикл 23: DIVERGES

Маргины карты закрыты (замер: инсеты 6.4/7.2/6.4). Осталось: инсет висит на `.body`, а не на `.viewer` — таб-стрип вне рамки (пилюля начинается на карта+8 против карта+14 у оригинала); паддингов `.body 8px 0 10px` нет. Брейдкрамб и «Save Ctrl+S» — сверх оригинала.

## Цикл 24: DIVERGES

Закрыто: инсеты карты переехали с тела на `.viewer` — теперь мантия
(`bg-mantle`, `radius-md`, `overflow: hidden`, `margin: 0 6px 6px`) держит И
таб-стрип, И тело, как в оригинале. Замер: колонка 624.8 → карта 630.8 →
регион стрипа 631.2, пилюля = стрип + `.strip{padding:4px space-2}` =
колонка + 14.4 (было + 8). Телу вернули `padding: 8px 0 10px` и `editor-bg`.

Осталось (сверх оригинала, не расхождение метрик): брейдкрамб внутри рамки и
кнопка «Save Ctrl+S» — наши добавки.

## Цикл 26: DIVERGES

Осталось: брейдкрамб-строка и кнопка «Save Ctrl+S» — надстройки сверх
оригинала, у него Ctrl+S висит на хуке и видимой кнопки нет; нет `.bodyFlush`
и `.retainLayer` как следствие отсутствия вебвью-табов (114); `.viewer` описан
через `size_full` вместо `flex: 1`
