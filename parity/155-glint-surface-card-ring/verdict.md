# 155 glint-surface-card-ring — verdict (review cycle 1)
VERDICT: MATCH
Вопрос light glint-mid ЗАКРЫТ: light-theme.css:39 использует var(--bg-surface)
(«cream paper needs the border tone») — наш LIGHT.glint_mid #e6e1d4 = bg_surface
ВЕРЕН (palette.rs:125, тест palette.rs:140-143). Кольцо 1px padding-box, 135deg,
стопы 0/22/78/100 через 2 оверлея, edge .18 обеих тем — 1:1. glint_surface()
без вызовов (все сайты — glint_surface_wv_holed с теми же пикселями).

## Цикл 5: MATCH

Glint-ring: 135°, стопы 0/22/78/100 двумя слоями, edge α .18 в обеих темах, кромка 1px, внешний радиус 16 / внутренний 15; `glint_mid` dark #262533 (= bg-mantle), light #e6e1d4 (= bg-surface) — закрыто тестом.

## Цикл 15: MATCH

Glint-кромка: 135°, стопы 0/22/78/100 двумя слоями, edge α .18, внешний r16 / внутренний 15.

## Цикл 18: MATCH

Glint-кромка: 135°, стопы 0/22/78/100, edge α .18, r16/15; `glint_mid` совпадает с токенами обеих тем.
