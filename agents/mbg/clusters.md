# Backgrounds / images: кластеры baseline

Исходный HEAD `d67cf59`; собственный `wptrun-base.exe`. 179 пар: 8 green, 171 red (background-size-031 проходит при повторе baseline).
Просмотрены 28 пар снимков (из выборки 30; две green не сохраняют снимков).
Сырые снимки/отчёты: внешняя private diagnostics `split-2026-10-06/agents/mbg`.

Это первоначальная карта: помеченные гипотезы и составные группы не выдаются за доказанную единую причину. Перед реализацией группу делить по первой дивергенции.

## Прочее: требуется причинная локализация — 5

- `background-color-animation-in-body.html`
- `background-image-007.html`
- `border-width-pixel-snapping-001-b.html`
- `simple-bg-color.html`
- `multiple-position-color-stop-radial.html`

## Attachment: область local/fixed и прокрутка — 6

- `background-attachment-350.html`
- `background-attachment-fixed-inside-transform-1.html`
- `background-attachment-local-hidden.html`
- `background-attachment-margin-root-002.html`
- `table-cell-background-local-002.html`
- `table-cell-background-local-003.html`

## Border-area: отсутствует маска площади рамки для изображения — 8

- `background-clip-border-area-and-blend-mode.html`
- `clip-border-area-background-geometry.html`
- `clip-border-area-border-on-top.html`
- `clip-border-area-border-shape-background-position.html`
- `clip-border-area-box-decoration-break.html`
- `clip-border-area-multiple-backgrounds.html`
- `clip-border-area-on-body-not-propagated-to-root.html`
- `clip-border-area-text.html`

## Rounded clip: обрезка целого слоя и внутренние радиусы; overflow-клип отдельно — 13

- `background-clip-content-box-with-border-radius-002.html`
- `background-clip-content-box-with-border-radius-003.html`
- `background-clip-padding-box-with-border-radius-002.html`
- `background-clip-padding-box-with-border-radius-003.html`
- `background-image-centered-with-border-radius.html`
- `background-rounded-image-clip-001.html`
- `border-radius-009.xht`
- `border-radius-010.xht`
- `border-radius-clip-001.html`
- `border-radius-clip-002.htm`
- `border-radius-clipping-002.html`
- `border-radius-clipping-with-transform-001.html`
- `css-border-radius-001.html`

## Text clip: отсутствует маска глифов для сложного слоя/потомков — 10

- `background-clip-text-and-blend-mode.html`
- `clip-text-blend-mode.html`
- `clip-text-fragmentation.html`
- `clip-text-multi-line.html`
- `clip-text-multiline-background-image.html`
- `clip-text-out-of-flow-child.html`
- `clip-text-stacking-context-child-002.html`
- `clip-text-stacking-context-child.html`
- `first-letter-space-not-selected.html`
- `linear-gradient-currentcolor-first-line.html`

## Border-area: image source not masked to border area — 1

- `clip-border-area-border-image.html`

## Border-image: source-to-destination scaling (hypothesis) — 3

- `border-image-006.html`
- `border-image-013.html`
- `css3-border-image-source.html`

## Border-image: generated image intrinsic dimensions (hypothesis) — 2

- `border-image-image-type-001.htm`
- `border-image-image-type-003.htm`

## Border-image: whole-image sampling phase lost by cropping tiles (hypothesis) — 14

- `border-image-repeat-002.htm`
- `border-image-repeat-004.htm`
- `border-image-repeat-round-1.html`
- `border-image-repeat-round-2.html`
- `border-image-repeat-space-1.html`
- `border-image-repeat-space-10.html`
- `border-image-repeat-space-3.html`
- `border-image-repeat-space-4.html`
- `border-image-repeat-space-5.html`
- `border-image-repeat-space-6.html`
- `border-image-repeat-space-7.html`
- `border-image-round-and-stretch.html`
- `css3-border-image-repeat-repeat.html`
- `css3-border-image-repeat-stretch.html`

## Border-image: shorthand source/slice resolution (hypothesis) — 2

- `border-image-shorthand-002.htm`
- `border-image-shorthand-003.htm`

## Border-image: overlapping slice or width geometry (hypothesis) — 3

- `border-image-slice-005.htm`
- `border-image-slice-007.htm`
- `border-image-width-007.xht`

## Gradients: interpolation-space selection / repeating sampling (hypothesis) — 4

- `background-gradient-interpolation-003.html`
- `css-color-4-colors-default-to-oklab-gradient.html`
- `legacy-color-gradient.html`
- `repeating-gradient-hsl-and-oklch.html`

## Float layout: float background area differs before gradient painting (outside this pilot) — 1

- `background-gradient-subpixel-fills-area.html`

## Gradients: native fill and tiled background stacking (hypothesis) — 2

- `bg-color-with-gradient.html`
- `gradients-with-border.html`

## Gradients: stop animation after style resolution (hypothesis) — 1

- `gradient-move-stops.html`

## Gradients: sampling/quantization mismatch against bitmap references (hypothesis) — 5

- `display-p3-linear-gradient.html`
- `oklab-gradient.html`
- `srgb-gradient.html`
- `srgb-linear-gradient.html`
- `xyz-gradient.html`

## Gradients: missing component carry-forward before interpolation (hypothesis) — 2

- `gradient-analogous-missing-components-003.html`
- `gradient-none-interpolation.html`

## Gradients: premature gamut mapping of intermediate color-mix stops (confirmed for five gains) — 9

- `gradient-decreasing-hue-lch.html`
- `gradient-increasing-hue-lch.html`
- `gradient-longer-hue-lch-001.html`
- `gradient-longer-hue-lch-002.html`
- `gradient-longer-hue-lch-003.html`
- `gradient-longer-hue-lch-004.html`
- `gradient-longer-hue-lch-005.html`
- `gradient-longer-hue-lch-006.html`
- `gradient-longer-hue-lch-007.html`

## Gradients: stop range normalization / degenerate line (hypothesis) — 4

- `gradient-eval-001.html`
- `gradient-eval-004.html`
- `gradient-infinity-003.html`
- `normalization-linear-degenerate.html`

## Gradients: relative/currentColor stop resolution (hypothesis) — 1

- `linear-gradient-relative-currentcolor-stop.html`

## Gradients: keyword corner direction resolved without aspect ratio (confirmed in code) — 1

- `linear-gradient-non-square.html`

## Gradients: raster path uses linear projection for a radial image (confirmed in code) — 3

- `normalization-radial-4.html`
- `normalization-radial-degenerate.html`
- `tiled-radial-gradients.html`

## Таблица: перенос многослойного фона группы в ячейки — 1

- `background-multiple-layers-table-cell.html`

## Геометрия плиток: positioning/paint areas и space fallback — 8

- `origin-border-box_with_position.html`
- `background-position-calc-minmax-001.html`
- `background-position-negative-percentage-comparison-002.html`
- `background-position-negative-percentage-comparison.html`
- `background-position-subpixel-at-border.tentative.html`
- `background-repeat-space-2.html`
- `background-repeat-space-8.html`
- `css3-background-origin-border-box.html`

## Уже зелёные на исходном HEAD — 8

- `background-size-031.html`
- `origin-border-box_with_radius.html`
- `origin-content-box_with_radius.html`
- `origin-padding-box_with_radius.html`
- `border-image-repeat-round-003.html`
- `border-image-repeat-round-stretch-001.html`
- `border-image-repeat-stretch-round-001.html`
- `svg-script-is-ignored.html`

## Размер SVG-плитки: intrinsic ratio, viewport и round — 9

- `background-size-029.html`
- `background-size-contain-svg-view.html`
- `background-size-cover-svg.html`
- `background-size-vector-004.html`
- `background-size-vector-006.html`
- `diagonal-percentage-vector-background.html`
- `wide--12px-auto--nonpercent-width-omitted-height.html`
- `wide--12px-auto--nonpercent-width-percent-height.html`
- `wide--cover--nonpercent-width-nonpercent-height.html`

## Тени: контур и область/порядок окраски; фрагменты требуют отдельного владельца — 14

- `box-shadow-041.html`
- `box-shadow-042.html`
- `box-shadow-border-radius-001.html`
- `box-shadow-currentcolor.html`
- `box-shadow-multiple-001.html`
- `box-shadow-overlapping-001.html`
- `box-shadow-radius-001.html`
- `slice-block-fragmentation-001.html`
- `slice-block-fragmentation-002.html`
- `slice-block-fragmentation-003.html`
- `slice-inline-fragmentation-001.html`
- `slice-inline-fragmentation-002.html`
- `slice-inline-fragmentation-003.html`
- `inset-box-shadow-stacking-context-scroll.html`

## Conic: разрешение calc/шрифтовых координат и углов (часть пар уже green) — 8

- `conic-gradient-calc-angle-percentage.html`
- `conic-gradient-center.html`
- `conic-gradient-line-height-relative-units-001.html`
- `conic-gradient-line-height-relative-units-002.html`
- `conic-gradient-line-height-relative-units-003.html`
- `conic-gradient-line-height-relative-units-004.html`
- `conic-gradient-001.html`
- `normalization-conic-degenerate.html`

## Cross-fade: размеры входов, нормализация весов и квантизация alpha — 5

- `cross-fade-basic.html`
- `cross-fade-cross-origin-orientation.sub.html`
- `cross-fade-natural-size.html`
- `cross-fade-premultiplied-alpha.html`
- `cross-fade-target-alpha.html`

## image(): отсутствует обработка fallback и направления — 5

- `css-image-fallbacks-and-annotations.html`
- `css-image-fallbacks-and-annotations002.html`
- `css-image-fallbacks-and-annotations003.html`
- `css-image-fallbacks-and-annotations004.html`
- `css-image-fallbacks-and-annotations005.html`

## Ориентация: согласование природного размера и фильтрации растра (гипотеза) — 15

- `image-orientation-background-image.html`
- `image-orientation-background-position.html`
- `image-orientation-background-properties-border-radius.html`
- `image-orientation-background-properties.html`
- `image-orientation-default.html`
- `image-orientation-exif-png.html`
- `image-orientation-from-image-composited.html`
- `image-orientation-from-image-content-images.html`
- `image-orientation-from-image-embedded-content.html`
- `image-orientation-from-image.html`
- `image-orientation-img-object-fit.html`
- `image-orientation-mask-image.html`
- `image-orientation-none-content-images.html`
- `image-orientation-none-cross-origin-border-image.sub.html`
- `image-orientation-none.html`

## image-set(): выбор/адрес/разрешение кандидата — 4

- `image-set-content-rendering.html`
- `image-set-no-url-rendering.html`
- `image-set-resolution-002.html`
- `image-set-resolution-003.html`

## object-view-box: область источника и intrinsic sizing — 2

- `object-view-box-contain-intrinsic-size.html`
- `object-view-box-size-containment.html`

## Pilot 1

20 gradient targets + 40 greens: +5/-0. Only delayed mapping of color-mix stops was changed. Other hue pairs require separate diagnosis.

References followed: CSS Color 4 Color Interpolation / Gamut Mapping; CSS Color 5 color-mix retains its mixing space; Blink core/css/css_gradient_value.cc:395-408 retains resolved stop colors until gradient construction.

Full gate 1: 1686 unique pairs, targets +5/-0; 166 of 179 remain red. Broad 735/735 baseline greens kept; counters-010 and counter-reset-reversed-siblings-003 are identically red on own baseline (known contradictions in agent rules). Family 813/813 kept. gate_fast.sh cannot recheck >150 suspects; the complete saved baseline target reports and a separate two-pair baseline slice establish no losses without altering the runner.

Baseline retry: background-size-031 is green on the original exe; exclude its initial capture flake from gains. Stable mechanism gain is +5/-0; 166 reds remain.
