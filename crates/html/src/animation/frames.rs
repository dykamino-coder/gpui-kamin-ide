//! Кадры анимаций и переходов.
// owner: A

use crate::render::*;

/// Обернуть элемент плавным переходом, если он задан.
///
/// Поддерево пересобирается по доле перехода — иначе смешанный стиль некуда
/// применить: у собранного элемента стиль уже зафиксирован.
pub(crate) fn transitioned(e: &Element, inherited: &Computed, opts: &RenderOpts) -> Option<AnyElement> {
    let seconds = e.style.transition?;
    let hover = e.hover.clone()?;
    let node = e.clone();
    let inherited = inherited.clone();
    let opts = opts.clone();
    let depth = defer_depth();
    let build = std::rc::Rc::new(move |k: f32| {
        let _depth = DepthScope::enter(depth);
        let mut mixed = node.clone();
        mixed.style = node.style.blend(&hover, k);
        // Слой наведения снят: его роль уже сыграла доля перехода, иначе
        // стиль прыгнул бы поверх плавного.
        mixed.hover = None;
        mixed.style.transition = None;
        element(&mixed, &inherited, &opts)
    });
    Some(
        crate::transition::Transition::new(
            gpui::ElementId::Integer(e.node_id as u64),
            seconds,
            build,
        )
        .into_any_element(),
    )
}

/// Интерполяция стиля между кадрами анимации.
///
/// Интерполируются те свойства, которые анимируют на практике и которые можно
/// подменить у уже собранного элемента: прозрачность, заливка, цвет текста,
/// сдвиг и размеры. Всё остальное берётся с ближайшего кадра — перестраивать
/// поддерево каждый кадр нельзя, это стоило бы дороже самой анимации.
pub(crate) fn frame_at(frames: &[(f32, Computed)], t: f32) -> Computed {
    let t = t.clamp(0.0, 1.0);
    let mut prev = &frames[0];
    let mut next = &frames[frames.len() - 1];
    for pair in frames.windows(2) {
        if t >= pair[0].0 && t <= pair[1].0 {
            prev = &pair[0];
            next = &pair[1];
            break;
        }
    }
    let span = (next.0 - prev.0).max(0.0001);
    let k = ((t - prev.0) / span).clamp(0.0, 1.0);
    let mut out = prev.1.clone();
    let lerp = |a: f32, b: f32| a + (b - a) * k;
    if let (Some(a), Some(b)) = (prev.1.opacity, next.1.opacity) {
        out.opacity = Some(lerp(a, b));
    }
    let mix = |a: crate::value::Color, b: crate::value::Color| crate::value::Color {
        r: lerp(a.r, b.r),
        g: lerp(a.g, b.g),
        b: lerp(a.b, b.b),
        a: lerp(a.a, b.a),
    };
    if let (Some(a), Some(b)) = (prev.1.background, next.1.background) {
        out.background = Some(mix(a, b));
    }
    if let (Some(a), Some(b)) = (prev.1.color, next.1.color) {
        out.color = Some(mix(a, b));
    }
    let len = |a: Option<Len>, b: Option<Len>| -> Option<Len> {
        match (a?, b?) {
            (Len::Px(x), Len::Px(y)) => Some(Len::Px(lerp(x, y))),
            (Len::Pct(x), Len::Pct(y)) => Some(Len::Pct(lerp(x, y))),
            // Разные природы (`0px → 200vw`, `0% → 200vw`) — покомпонентно,
            // как `calc()`; несводимая смесь — ближайший кадр.
            (x, y) => Some(crate::value::lerp_len(x, y, k).unwrap_or(x)),
        }
    };
    out.width = len(prev.1.width, next.1.width).or(out.width);
    out.height = len(prev.1.height, next.1.height).or(out.height);
    // Фильтры интерполируются покомпонентно; `none` = нейтральный
    // (filter-effects-1 §Interpolation, css-filters-animation-*).
    if prev.1.filter.is_some() || next.1.filter.is_some() {
        let a = prev
            .1
            .filter
            .unwrap_or_else(crate::computed::Filter::neutral);
        let b = next
            .1
            .filter
            .unwrap_or_else(crate::computed::Filter::neutral);
        out.filter = Some(crate::computed::Filter {
            grayscale: lerp(a.grayscale, b.grayscale),
            brightness: lerp(a.brightness, b.brightness),
            saturate: lerp(a.saturate, b.saturate),
            invert: lerp(a.invert, b.invert),
            sepia: lerp(a.sepia, b.sepia),
            opacity: lerp(a.opacity, b.opacity),
            hue_rotate: lerp(a.hue_rotate, b.hue_rotate),
            contrast: lerp(a.contrast, b.contrast),
            blur: lerp(a.blur, b.blur),
        });
    }
    if prev.1.backdrop_blur.is_some() || next.1.backdrop_blur.is_some() {
        out.backdrop_blur = Some(lerp(
            prev.1.backdrop_blur.unwrap_or(0.0),
            next.1.backdrop_blur.unwrap_or(0.0),
        ));
    }
    // Цветовые функции подложки — покомпонентно, как у `filter`: эталоны
    // `css-backdrop-filters-animation-*` — статический `backdrop-filter`
    // середины, и без интерполяции тест разошёлся бы с ним.
    if prev.1.backdrop_color.is_some() || next.1.backdrop_color.is_some() {
        let a = prev
            .1
            .backdrop_color
            .unwrap_or_else(crate::computed::Filter::neutral);
        let b = next
            .1
            .backdrop_color
            .unwrap_or_else(crate::computed::Filter::neutral);
        out.backdrop_color = Some(crate::computed::Filter {
            grayscale: lerp(a.grayscale, b.grayscale),
            brightness: lerp(a.brightness, b.brightness),
            saturate: lerp(a.saturate, b.saturate),
            invert: lerp(a.invert, b.invert),
            sepia: lerp(a.sepia, b.sepia),
            opacity: lerp(a.opacity, b.opacity),
            hue_rotate: lerp(a.hue_rotate, b.hue_rotate),
            contrast: lerp(a.contrast, b.contrast),
            blur: 0.0,
        });
    }
    // `drop-shadow()` — покомпонентно; у `none` — тень с нулевыми длинами и
    // цветом `transparent` (filter-effects-1 Overview.bs:3460-3463, :418).
    // Цвет — в умноженном на альфу виде: `black → transparent` на середине
    // даёт `rgba(0,0,0,0.5)` (`css-filters-animation-drop-shadow`).
    if prev.1.drop_shadow.is_some() || next.1.drop_shadow.is_some() {
        let none = crate::computed::Shadow {
            x: 0.0,
            y: 0.0,
            blur: 0.0,
            spread: 0.0,
            color: crate::value::Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.0,
            },
        };
        let a = prev.1.drop_shadow.unwrap_or(none);
        let b = next.1.drop_shadow.unwrap_or(none);
        let alpha = lerp(a.color.a, b.color.a);
        let pm = |x: f32, y: f32| {
            if alpha > 0.0 {
                lerp(x * a.color.a, y * b.color.a) / alpha
            } else {
                0.0
            }
        };
        out.drop_shadow = Some(crate::computed::Shadow {
            x: lerp(a.x, b.x),
            y: lerp(a.y, b.y),
            blur: lerp(a.blur, b.blur),
            spread: lerp(a.spread, b.spread),
            color: crate::value::Color {
                r: pm(a.color.r, b.color.r),
                g: pm(a.color.g, b.color.g),
                b: pm(a.color.b, b.color.b),
                a: alpha,
            },
        });
    }
    // `translate`/`rotate`/`scale`: `none` с одной стороны заменяется
    // тождеством (css-transforms-2 §individual-transforms: 0px, 0deg, 1).
    // Сдвиг раньше смешивался только при ОБОИХ значениях, и `to { translate:
    // … }` над стилем без сдвига стоял на месте.
    if prev.1.translate.is_some() || next.1.translate.is_some() {
        let zero = (Len::Px(0.0), Len::Px(0.0));
        let (a, b) = (
            prev.1.translate.unwrap_or(zero),
            next.1.translate.unwrap_or(zero),
        );
        out.translate = Some((
            len(Some(a.0), Some(b.0)).unwrap_or(a.0),
            len(Some(a.1), Some(b.1)).unwrap_or(a.1),
        ));
    }
    if prev.1.rotate_prop.is_some() || next.1.rotate_prop.is_some() {
        out.rotate_prop = Some(lerp(
            prev.1.rotate_prop.unwrap_or(0.0),
            next.1.rotate_prop.unwrap_or(0.0),
        ));
    }
    if prev.1.scale_prop.is_some() || next.1.scale_prop.is_some() {
        let (a, b) = (
            prev.1.scale_prop.unwrap_or((1.0, 1.0)),
            next.1.scale_prop.unwrap_or((1.0, 1.0)),
        );
        out.scale_prop = Some((lerp(a.0, b.0), lerp(a.1, b.1)));
    }
    // `transform` — разложенными матрицами (`Transform::lerp_2d`,
    // css-transforms-1 §matrix-interpolation); `none` — тождество. Объёмный
    // список или необратимая сторона — дискретно, ближайшим кадром (как было).
    if prev.1.transform.is_some() || next.1.transform.is_some() {
        let a = prev.1.transform.unwrap_or_default();
        let b = next.1.transform.unwrap_or_default();
        if (a.lin != b.lin || a.tr != b.tr)
            && !a.has_3d
            && !b.has_3d
            && let Some(m) = a.lerp_2d(&b, k)
        {
            out.transform = Some(m);
        }
    }
    // Отдельное свойство — тоже преобразование: коробка с ним несёт
    // `transform` (так делает и разбор `rotate`/`scale`), иначе
    // `transformed()` вышел бы раньше свёртки.
    if (out.rotate_prop.is_some() || out.scale_prop.is_some()) && out.transform.is_none() {
        out.transform = Some(Default::default());
    }
    out
}

/// Остановленная анимация (`AnimSpec::frozen`), запечённая в копию элемента:
/// кадр `(-delay)/duration` подставляется прямо в стиль, и дальше работает
/// весь обычный конвейер. `transforms` — нести ли и `rotate`/`scale`/
/// `transform`: их матрицу строит `transformed()` СНАРУЖИ элемента, от стиля,
/// который ему передан. Блочный путь (`transformed(animated(e), &e.style)`)
/// их не берёт: `!important` у нас кадры не перекрывает, а обязан
/// (css-cascade-5 §cascade-origin) — `translation-animation-on-important-
/// property` с `transform: none !important` уехала бы на середину пути.
pub(crate) fn bake_frozen(e: &Element, transforms: bool) -> Option<Element> {
    let (Some(frames), Some(spec)) = (e.anim.as_ref(), e.style.animation.as_ref()) else {
        return None;
    };
    if !spec.frozen() {
        return None;
    }
    Some(animation_frame::sample(e, &frame_at(frames, spec.frozen_t()), transforms))
}
