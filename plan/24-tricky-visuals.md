# 24 — Хитрые визуалы: точный механизм → перенос в GPUI

Ответ на «все тени/градиенты/хитрые бордеры сняты и продумано как перенести?»: **да**. Ниже каждый нетривиальный приём kamin-ide — точное CSS + рецепт в GPUI. GPUI умеет linear/radial/conic градиенты (linear до 4 стопов) и настоящие box-shadow (blur+spread, техника Figma/Evan Wallace) — этого хватает почти на всё. Два честных риска помечены ⚠.

## 1. Glint-surface (фирменный градиент-бордер плавающих панелей) — ГЛАВНОЕ
CSS (global.css:96): `border:1px solid transparent; background: linear-gradient(--bg-mantle,--bg-mantle) padding-box, var(--glint-border) border-box;`
glint-border (dark, dark-theme.css:31): `linear-gradient(135deg, rgba(255,255,255,0.18) 0%, --bg-mantle 22%, --bg-mantle 78%, rgba(255,255,255,0.18) 100%)` — РОВНО 4 стопа. Light (light-theme.css:39): те же 22/78%, углы `rgba(60,40,20,0.18)`, mid `--bg-surface`. Fallback (:root): mid `--bg-base`.
Смысл: mid-стопы = цвет ЗАЛИВКИ панели (не тёмной подложки), поэтому бордер «растворяется» и читается только диагональный блик по углам.
**Перенос (GPUI, точно) — РЕАЛИЗОВАН и проверен скриншотом 2026-07-24 (`crates/shell/src/ui/glint.rs`):** gpui 0.2.2 даёт максимум **2 стопа** на linear-gradient → 4-стоповый glint собран из ДВУХ наложенных 2-стоповых слоёв поверх сплошного mid (слой A: edge@0%→прозрачный@22%; слой B: прозрачный@78%→edge@100%; за пределами стопов — кламп, между 22–78% чистый mid). Внутренний rect inset 1px, radius 15, заливка --bg-mantle. Блики в углах + чистая середина подтверждены кропами скриншота. Компонент `glint_surface` — используется всеми картами (FilePanel/RightPanel/MainContent/MainBottomPanel + sidebar-webview-frame).

## 2. Радиальный фон приложения (AppLayout .appWrapper)
CSS (AppLayout.module.css:12-13): поверх --bg-sidebar два `radial-gradient(ellipse 1200px 600px at 20% 10%, accent-purple 8% → transparent 60%)` и `radial-gradient(ellipse 800px 500px at 90% 90%, accent-primary 6% → transparent 60%)`.
**Перенос:** ⚠ РЕШЕНО спайком 2026-07-24: в gpui **0.2.2 (crates.io) radial-градиентов НЕТ вовсе** (только linear, 2 стопа) — «radial/conic» в фундаменте относится к gpui main. → **бейк**: оба эллипса генерируются один раз в PNG/текстуру (скрипт или рантайм-генерация пикселей при старте/resize-классе) и рисуются img-слоем поверх bg-sidebar. Фон статичен — перерисовки нет. Тинт мягкий (6-8%), воспроизвести обязательно.

## 3. Свечение под лого (WelcomePlaceholder .logoWrap::before + .logo)
CSS: `::before` — 220px круг, `radial-gradient(circle, accent-primary 26% → transparent 68%)`, `filter: blur(6px)`, z 0. `.logo` — `filter: drop-shadow(0 6px 18px rgba(0,0,0,0.35))`.
**Перенос:** GPUI — мягкий radial-gradient quad 220px за лого (blur(6px) на радиале ≈ более широкий мягкий радиал, либо GPUI blur если есть) + drop-shadow лого = box-shadow 0 6px 18px rgba(0,0,0,.35) на sprite. ✅

## 4. Backdrop-blur оверлеев — ⚠ ЕДИНСТВЕННЫЙ реальный гэп
CSS: QuickOpen/FindInFiles backdrop `background: rgba(0,0,0,0.35); backdrop-filter: blur(2px)`. Toasts `.toast background: color-mix(--bg-surface 50%); backdrop-filter: blur(8px)`.
Проблема: backdrop-filter размывает КОНТЕНТ ПОЗАДИ; GPUI такого из коробки, судя по всему, НЕ имеет.
**Перенос:** спайк на старте — проверить, есть ли в gpui blur-behind/backdrop. Если нет:
- backdrop blur(2px) оверлеев → аппроксимация чуть более плотным скримом (rgba(0,0,0,0.42) вместо 0.35) — визуально почти неотличимо на 2px.
- toast blur(8px) заметнее → либо тюнить полупрозрачную поверхность (--bg-surface 62% вместо 50% без блюра), либо, если критично, снапшот-блюр области под тостом (дороже).
Помечено как fidelity-risk; решается тюнингом альфы до визуального совпадения (дифф-тест plan/22).

## 5. Inset box-shadow (drop-target «блок»)
CSS (global.css:65): `box-shadow: inset 0 0 0 2px color-mix(accent-red 60%, transparent)`.
**Перенос:** GPUI inset box-shadow может не поддерживаться → рисовать 2px внутреннюю обводку (inset rounded stroke) accent-red 60%. ✅

## 6. Rounded-corner маска табов терминала
CSS (TerminalToolbar.module.css:97,101): `radial-gradient(circle at 0 0, transparent 6px, editor-bg 6.5px)` — трюк «вырезать» внешний угол таба.
**Перенос:** в GPUI просто border-radius на элементе — трюк не нужен. ✅

## 7. Градиентные фоны табов/сессий (параметрические по цвету сессии)
CSS (SessionTab/.SessionItem): `.active background: linear-gradient(90deg, --tab-color 26%, --tab-color 14%); border-color --tab-color 45%`. `.tinted`: 15%/8% (hover 22%/12%); light-тема: другие проценты (26/16, active 42/26 border 60).
**Перенос:** GPUI linear-gradient 90° с 2 стопами, --tab-color = цвет сессии (параметр); проценты color-mix предвычислить в альфу per-theme. ✅

## 8. color-mix тинты (везде) — нагрузочные для цвета
Десятки `color-mix(in srgb, <token> N%, transparent)` (hover 10%, btnActive 16%, и т.д. — точные % в plan/23).
**Перенос:** GPUI нет color-mix → предвычислить каждый тинт как rgba(token, N%) на этапе резолва темы (plan 20). Проценты СКОПИРОВАТЬ (нагрузочные для 1:1). ✅

## 9. Тени (все) — точные значения
Токены (plan 20): shadow-lg/card/modal/tab/toast/dropdown/mini/bar/card-popup (точные offset+blur+rgba, dark/light). Плюс локальные: drag-ghost `0 4px 14px rgb(0 0 0/35%)`, session dropBar glow `0 0 4px accent-primary 60%`, overflow-menu `0 6px 24px rgb(0 0 0/30%)`.
**Перенос:** GPUI box_shadow(offset_x, offset_y, blur_radius, spread_radius, color) — 1:1. ✅ (⚠ известный баг GPUI: shadow без blur не рисуется — у нас все с blur, не задевает.)

## 9b. Icon-color filter (light-подстройка Catppuccin)
TreeIcon.module.css: `[data-theme="light"] .img { filter: saturate(3.2) brightness(0.7) }` — Catppuccin-иконки (запечённый цвет в SVG) в светлой теме перекрашиваются фильтром. **Перенос:** GPUI — либо применить saturate/brightness к спрайту (если есть image-фильтры), либо готовить отдельный light-вариант иконок. ✅ (мелочь, но обязательна для 1:1 light-дерева.)

## 9c. Спиннер вебвью
WebviewPanelView `.spinner`: 22×22 radius50%, border 2.5px text-primary16% с top accent-action, `animation: kaminWvSpin 0.7s linear` (rotate360). **Перенос:** GPUI вращающийся arc/ring. ✅ (reduced-motion — спиннеры НЕ гасятся, plan 20 §9.)

## 10. Шрифтовые фичи
`font-feature-settings: ss01` (FileTreeHeader/SessionsMode/CustomizeMode заголовки), глобальный `tabular-nums`. **Перенос:** GPUI FontFeatures (ss01, tnum). ✅

## 11. Анимации-градиенты (шиммеры/скелетоны)
kaminSkShimmer (sweep translateX), ChatSwitchSkeleton radial-gradient + `filter: blur(8px)` + `filter: drop-shadow(0 6px 18px rgba(0,0,0,0.35))` (ChatSwitchSkeleton.module.css:37,48), WebviewLoadingSkeleton (свой spinner/shimmer, 156 строк — снять метрики в plan/23), pulse/breathe. **Перенос:** GPUI анимации + анимируемый gradient-offset/opacity; blur(8px) скелетона → мягкий blurred quad или GPUI-blur (спайк как §4); drop-shadow → box-shadow. ✅ (каталог keyframes — plan 20 §9).

## Итог по вопросу
- Сняты и записаны: glint-рецепт (точные 4 стопа, оба theme + fallback), радиальные фоны (координаты/размеры/%), свечения (blur/drop-shadow), все тени (offset+blur+rgba), inset-обводки, параметрические градиенты табов, все color-mix %, шрифт-фичи, corner-трюки.
- Перенос продуман: GPUI покрывает linear/radial/conic + box-shadow → 90% один-в-один; glint = 2-quad техника (идентична).
- ⚠ Риски (2): (а) backdrop-filter blur — аппроксимация альфой/спайк; (б) ellipse-radial позиционирование — проверить GPUI или запечь. Оба не блокеры, решаются на старте визуального слоя, помечены для дифф-теста.

## Чеклист
- [ ] GlintSurface компонент (2-quad, 4 стопа, оба theme)
- [ ] Радиальный фон окна (эллипс — спайк/бейк)
- [ ] Свечение лого + drop-shadow
- [ ] Backdrop-blur: спайк GPUI → блюр или тюнинг альфы (дифф-тест)
- [ ] Inset-обводка вместо inset-shadow
- [ ] Параметрические градиенты сессий (tab-color, оба theme, точные %)
- [ ] Предвычисление всех color-mix тинтов
- [ ] Все box-shadow токены + локальные 1:1
- [ ] FontFeatures ss01 + tnum
- [ ] Шиммеры/скелетоны
