//! Элемент интерфейса «веб-вью»: рисует последний кадр браузера в своих
//! границах и сообщает браузеру, какого размера кадр нужен.
//!
//! Никаких дыр в фоне и отдельных окон: это обычный элемент дерева, поэтому
//! меню и модалки рисуются ПОВЕРХ него просто потому, что идут позже
//! (`plan/101-cef.md`, Ф2).

use gpui::prelude::*;
use gpui::{Bounds, Corners, Pixels, canvas, px};

/// Кадр вью `id`, скруглённый радиусом `radius`.
///
/// Размер браузеру задаётся В ФИЗИЧЕСКИХ пикселях: страница должна быть резкой
/// на экранах с масштабом, поэтому логические границы умножаем на масштаб окна.
/// Фокус-хендлы вью: живут между кадрами, чтобы gpui знал, что фокус
/// «внутри» обёртки — только тогда её on_key_down получает клавиатуру.
static FOCUS_HANDLES: std::sync::LazyLock<
    std::sync::Mutex<std::collections::HashMap<&'static str, gpui::FocusHandle>>,
> = std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashMap::new()));

/// Завести фокус-хендлы всем видимым вью. Зовётся из `sync_panels` (там есть
/// `cx`); сама обёртка хендл только ищет — тянуть `cx` через все колл-сайты
/// рендера не пришлось.
pub fn ensure_focus_handles(cx: &mut gpui::App) {
    let ids: Vec<&'static str> = ORIGINS
        .lock()
        .map(|m| m.keys().copied().collect())
        .unwrap_or_default();
    let mut map = FOCUS_HANDLES.lock().unwrap();
    for id in ids {
        map.entry(id).or_insert_with(|| cx.focus_handle());
    }
}

fn focus_handle_of(id: &'static str) -> Option<gpui::FocusHandle> {
    FOCUS_HANDLES.lock().ok()?.get(id).cloned()
}

pub fn web_view(id: &'static str, radius: f32) -> impl IntoElement {
    let fh = focus_handle_of(id);
    // Кадр рисует canvas, а события ловит обёртка: у canvas обработчиков нет.
    // Координаты переводим в систему вью вычитанием его начала — вью заведено
    // в логических пикселях, как и координаты gpui (`plan/101-cef.md`, Ф3).
    let frame = canvas(
        move |bounds: Bounds<Pixels>, window, _cx| {
            // Размер — ЛОГИЧЕСКИЙ (CSS-пиксели), масштаб отдельно: физический
            // растр CEF сделает сам, и текст будет того же кегля, что в
            // остальном интерфейсе.
            let w = f32::from(bounds.size.width).round() as i32;
            let h = f32::from(bounds.size.height).round() as i32;
            super::browsers::resize(id, w, h, window.scale_factor());
            bounds
        },
        move |bounds: Bounds<Pixels>, _prepaint, window, _cx| {
            // Где вью лежит на экране — нужно вводу. Своё начало у каждого
            // вью: жёсткая привязка к одной области ломала бы всё, кроме
            // браузера (чат, план, консоль — следующие фазы).
            remember_origin(id, &bounds);
            // Устройство окна нужно потоку CEF, чтобы открыть текстуру в
            // момент колбэка; запоминаем его отсюда, с потока отрисовки.
            super::shared_texture::remember_device(window);
            // Плитки атласа выгруженных вью: выбросить можно только отсюда.
            super::shared_texture::drain_dead(window);
            // ОСНОВНОЙ путь: готовая текстура от Chromium — ни копий, ни
            // догона по размеру. Есть GPU — идём сюда.
            let scale_pre = window.scale_factor();
            let want = (
                (f32::from(bounds.size.width) * scale_pre).round() as i32,
                (f32::from(bounds.size.height) * scale_pre).round() as i32,
            );
            let _tex_started = std::time::Instant::now();
            let got = super::shared_texture::texture_for(id, window, want);
            super::diag::tex_took(_tex_started.elapsed().as_micros() as u32);
            if let Some(tex) = got {
                let (tw, th) = tex.size();
                // Кадр не того размера — просим ещё раз. Один запрос при
                // быстром драге иногда теряется, и панель застывает.
                let scale = window.scale_factor();
                let want_w = (f32::from(bounds.size.width) * scale).round() as i32;
                let want_h = (f32::from(bounds.size.height) * scale).round() as i32;
                if (tw - want_w).abs() > 2 || (th - want_h).abs() > 2 {
                    super::diag::cropped();
                    super::browsers::nudge(id);
                    note_mismatch(id);
                } else {
                    clear_mismatch(id);
                }
                // ПОДЛОЖКА цветом панели под всей областью вью. На неё
                // ложится всё, что кадр не покрыл: прозрачные пиксели самого
                // кадра иначе читаются ЧЁРНЫМ (шейдер внешней текстуры пишет
                // цвет как есть).
                window
                    .paint_quad(gpui::fill(bounds, p_bg()).corner_radii(Corners::all(px(radius))));
                // Кадр рисуем 1:1, БЕЗ растяжения, прижатым к левому верхнему
                // углу. Растяжение «плющило» страницу на каждом шаге драга —
                // заметнее любых полос.
                //
                // ИСКЛЮЧЕНИЕ — крупное рассогласование (кадр под другой
                // масштаб/размер: при загрузке под RDP scale доезжает до
                // браузера позже первых кадров): 1:1 рисовал страницу «вдвое
                // меньше в углу» до was_resized (прод-жалоба). Такой кадр
                // растягиваем на панель — секунда мыла вместо сжатия. Порог
                // 10%: шаги драга различаются на единицы процентов и остаются
                // на прежнем 1:1-пути.
                let big_mismatch = (tw - want_w).abs() * 10 > want_w || (th - want_h).abs() * 10 > want_h;
                // Растяжение — АВАРИЙНЫЙ путь, не штатный. При быстром драге
                // сплиттера кадры отстают, расхождение >10% держится десятки
                // миллисекунд — и страница «плющилась» на каждом шаге (жалоба
                // «если быстро ресайзить, растягивается»). Пока CEF догоняет,
                // рисуем кадр 1:1 (остальное закрывает подложка). Растягиваем
                // только если верного размера нет ДОЛЬШЕ порога — это спасает
                // случай «кадр вдвое меньше и таким и останется» (старт/DPI).
                let stale = big_mismatch && mismatch_ms(id) > STRETCH_AFTER_MS;
                if stale {
                    super::diag::stretched();
                    window.paint_external_texture(
                        gpui::Bounds { origin: bounds.origin, size: bounds.size },
                        Corners::all(px(radius)),
                        &tex.cropped(tw, th),
                    );
                    return;
                }
                let mut draw_w = (tw as f32 / scale).min(f32::from(bounds.size.width));
                let mut draw_h = (th as f32 / scale).min(f32::from(bounds.size.height));
                // СНЭП ±1 лог.px: округления «логические ↔ физические» дают
                // кадру размер на пиксель меньше панели, и при драге ширины
                // вью дёргался по ВЫСОТЕ (полоса смазки появлялась/исчезала).
                // Расхождение в пиксель растягиваем — глазу оно невидимо.
                let mut snapped = false;
                if f32::from(bounds.size.width) - draw_w <= 1.0
                    && (f32::from(bounds.size.width) - draw_w).abs() > f32::EPSILON
                {
                    draw_w = f32::from(bounds.size.width);
                    snapped = true;
                }
                if f32::from(bounds.size.height) - draw_h <= 1.0
                    && (f32::from(bounds.size.height) - draw_h).abs() > f32::EPSILON
                {
                    draw_h = f32::from(bounds.size.height);
                    snapped = true;
                }
                // Снэп больше НЕ растягивает: кадр кладётся по ФИЗИЧЕСКИМ
                // границам (`paint_external_texture_px`), поэтому доли пикселя
                // от дробного DPI не «догоняются» ресемплом. Флаг остаётся
                // только как признак того, что зазор в пределах пикселя закрыт
                // самим кадром (а не подложкой).
                let _ = snapped;
                super::diag::exact();
                let draw = gpui::Bounds {
                    origin: bounds.origin,
                    size: gpui::size(px(draw_w), px(draw_h)),
                };
                let crop_w = (draw_w * scale).round() as i32;
                let crop_h = (draw_h * scale).round() as i32;
                let shown = tex.cropped(crop_w, crop_h);
                // Физический прямоугольник кадра: origin округляем ВНИЗ (как
                // это делал gpui в `map_origin(floor)`), размер = ровно размер
                // вырезанного тайла — спрайт 1:1.
                let draw_px = gpui::Bounds {
                    origin: gpui::point(
                        gpui::DevicePixels((f32::from(bounds.origin.x) * scale).floor() as i32),
                        gpui::DevicePixels((f32::from(bounds.origin.y) * scale).floor() as i32),
                    ),
                    size: gpui::size(gpui::DevicePixels(crop_w), gpui::DevicePixels(crop_h)),
                };
                let full_w = f32::from(bounds.size.width);
                let full_h = f32::from(bounds.size.height);
                // Прямой шов разрешён, только если зазор ШИРЕ радиуса: узкая
                // полоска смазки не может нести скругление, и прямой угол
                // кадра «обгонял» скруглённый силуэт панели (жалоба). Узкий
                // зазор → кадр скруглён как панель, сливер закрывает подложка.
                // Силуэт панели не протыкается никогда.
                let right_gap = full_w - draw_w >= radius;
                let bottom_gap = full_h - draw_h >= radius;
                // Скругляем ТОЛЬКО углы, совпадающие с углами ПАНЕЛИ: срез
                // полукругом посреди панели открывал тёмную подложку.
                let c = Corners {
                    top_left: px(radius),
                    top_right: if right_gap { px(0.) } else { px(radius) },
                    bottom_left: if bottom_gap { px(0.) } else { px(radius) },
                    bottom_right: if right_gap || bottom_gap {
                        px(0.)
                    } else {
                        px(radius)
                    },
                };
                let _ = &draw; // логический прямоугольник больше не нужен для кадра
                window.paint_external_texture_px(draw_px, c, &shown);
                // Кадр УЖЕ панели (расширение не догнало) — недостающее
                // закрываем РАСТЯНУТЫМ КРАЕМ кадра, а не цветной полосой:
                // край страницы «тянется» за ручкой, как у нативной
                // перевёрстки, и глаз не видит чужого цвета (запрос «чтобы
                // выглядело нативно»). Так же поступают композиторы
                // (edge-clamp). Живёт это один-два кадра.
                if right_gap {
                    let strip = shown.region(crop_w - 1, 0, 1, crop_h);
                    let rect = gpui::Bounds {
                        origin: gpui::point(bounds.origin.x + px(draw_w), bounds.origin.y),
                        size: gpui::size(px(full_w - draw_w), px(draw_h)),
                    };
                    let c = Corners {
                        top_right: px(radius),
                        bottom_right: px(radius),
                        ..Corners::default()
                    };
                    window.paint_external_texture(rect, c, &strip);
                }
                if bottom_gap {
                    // Полоса на всю ширину панели: закрывает и угол.
                    let strip = shown.region(0, crop_h - 1, crop_w, 1);
                    let rect = gpui::Bounds {
                        origin: gpui::point(bounds.origin.x, bounds.origin.y + px(draw_h)),
                        size: gpui::size(px(full_w), px(full_h - draw_h)),
                    };
                    let c = Corners {
                        bottom_left: px(radius),
                        bottom_right: px(radius),
                        ..Corners::default()
                    };
                    window.paint_external_texture(rect, c, &strip);
                }
                // Попап страницы (дропдаун `<select>`): отдельная текстура
                // CEF, кладётся поверх кадра по прямоугольнику `on_popup_size`.
                if let Some((ptile, (rx, ry, rw, rh))) = super::popup::texture_for(id, window) {
                    let rect = gpui::Bounds {
                        origin: gpui::point(
                            bounds.origin.x + px(rx as f32),
                            bounds.origin.y + px(ry as f32),
                        ),
                        size: gpui::size(px(rw as f32), px(rh as f32)),
                    };
                    window.paint_external_texture(rect, Corners::default(), &ptile);
                }
                super::diag::paint();
                return;
            }
            // SOFTWARE-режим (RDP, виртуалки, WARP): GPU-композитинга у
            // Chromium нет, `on_accelerated_paint` молчит, кадры идут через
            // `on_paint` в оперативке. Рисуем их 1:1 без растяжения — это
            // ОСОЗНАННЫЙ режим для приоритетного RDP-сценария, а не
            // маскировка сбоя GPU-пути: когда есть хоть одна accelerated
            // текстура, ветка выше выигрывает всегда.
            if let Some(image) = super::frames::get(id) {
                let scale = window.scale_factor();
                let isz = image.size(0);
                let (iw, ih) = (isz.width.0 as f32 / scale, isz.height.0 as f32 / scale);
                // АНТИ-ЗАЛИПАНИЕ ДЛЯ SW-ПУТИ (RDP). Раньше проверка «кадр не
                // того размера → nudge» была ТОЛЬКО в GPU-ветке: на RDP
                // застрявший hold_resize у CEF не детектился никогда, и кадр
                // оставался сжатым/растянутым до следующего события размера —
                // это и есть «фриз с недорастянувшейся текстурой».
                let want_w = (f32::from(bounds.size.width) * scale).round() as i32;
                let want_h = (f32::from(bounds.size.height) * scale).round() as i32;
                let (fw, fh_px) = (isz.width.0, isz.height.0);
                if (fw - want_w).abs() > 2 || (fh_px - want_h).abs() > 2 {
                    super::diag::cropped();
                    super::browsers::nudge(id);
                }
                window
                    .paint_quad(gpui::fill(bounds, p_bg()).corner_radii(Corners::all(px(radius))));
                let draw = gpui::Bounds {
                    origin: bounds.origin,
                    size: gpui::size(
                        px(iw.min(f32::from(bounds.size.width))),
                        px(ih.min(f32::from(bounds.size.height))),
                    ),
                };
                // 1:1 БЕЗ растяжения: режем кадр до нарисованного региона
                // (`paint_image_region`, KaminIDE patch в vendored gpui).
                // `paint_image` вписывал ВЕСЬ образ в `draw`, поэтому кадр
                // больше панели всегда плющился по каждой оси отдельно — это и
                // был RDP-фриз с недорастянутой текстурой.
                let reg_w = (f32::from(draw.size.width) * scale).round() as i32;
                let reg_h = (f32::from(draw.size.height) * scale).round() as i32;
                // Пиксели не пересэмплены ни в одном из случаев: либо кадр
                // равен области, либо рисуется его часть 1:1 (остальное
                // закрывает подложка панели).
                super::diag::exact();
                let region = gpui::Bounds {
                    origin: gpui::point(gpui::DevicePixels(0), gpui::DevicePixels(0)),
                    size: gpui::size(gpui::DevicePixels(reg_w), gpui::DevicePixels(reg_h)),
                };
                let _ = window.paint_image_region(
                    draw,
                    Corners::all(px(radius)),
                    image,
                    0,
                    false,
                    region,
                );
                if let Some((pimg, (rx, ry, rw, rh))) = super::popup::sw_frame(id) {
                    // Кадр попапа лежит в ФИЗИЧЕСКИХ px (плюс alignment-паддинг
                    // буфера), а прямоугольник `on_popup_size` — в CSS-px.
                    // Рисовали физический кадр в CSS-рект → на DPI≠100%
                    // дропдаун всегда растянут + видно мусор паддинга. Берём
                    // размер ИЗ КАДРА (÷scale) и режем регион 1:1.
                    let psz = pimg.size(0);
                    let want_w = (rw as f32 * scale).round() as i32;
                    let want_h = (rh as f32 * scale).round() as i32;
                    let reg_w = want_w.min(psz.width.0).max(1);
                    let reg_h = want_h.min(psz.height.0).max(1);
                    let rect = gpui::Bounds {
                        origin: gpui::point(
                            bounds.origin.x + px(rx as f32),
                            bounds.origin.y + px(ry as f32),
                        ),
                        size: gpui::size(px(reg_w as f32 / scale), px(reg_h as f32 / scale)),
                    };
                    let region = gpui::Bounds {
                        origin: gpui::point(gpui::DevicePixels(0), gpui::DevicePixels(0)),
                        size: gpui::size(gpui::DevicePixels(reg_w), gpui::DevicePixels(reg_h)),
                    };
                    let _ = window.paint_image_region(
                        rect,
                        Corners::default(),
                        pimg,
                        0,
                        false,
                        region,
                    );
                }
                super::diag::sw_paint();
                super::diag::paint();
                return;
            }
            static WARNED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);
            if WARNED.swap(false, std::sync::atomic::Ordering::Relaxed) {
                println!("[cef] нет ни GPU-текстуры, ни software-кадра вью {id}");
            }
        },
    )
    .size_full();
    let fh_click = fh.clone();
    gpui::div()
        .id(gpui::SharedString::from(format!("webview:{id}")))
        .relative()
        .size_full()
        // Без track_focus клавиатура НЕ приходит: gpui шлёт key-события
        // только по цепочке от сфокусированного элемента (поймано юзером:
        // стенд webkey работал, живой ввод — нет). Первый кадр вью может
        // пройти без хендла — `sync_panels` заведёт его к следующему.
        // Контекст «webview» глушит Root-биндинги (`main.rs`): Enter и
        // Escape достаются странице, а не действиям шелла.
        .key_context("webview")
        .when_some(fh, |el, fh| el.track_focus(&fh))
        // Форму курсора диктует страница (`on_cursor_change`): рука над
        // ссылкой, каретка над текстом. Смена формы заказывает кадр.
        .cursor(super::cursors::style(id))
        .child(frame)
        .on_mouse_move(move |ev: &gpui::MouseMoveEvent, window, _cx| {
            let (x, y) = local(ev.position, id);
            super::input::mouse_move(
                id,
                x,
                y,
                super::input::modifiers(&ev.modifiers, ev.pressed_button),
                false,
            );
            let _ = window;
        })
        .on_mouse_down(
            gpui::MouseButton::Left,
            move |ev: &gpui::MouseDownEvent, _w, _cx| {
                let (x, y) = local(ev.position, id);
                if let Some(fh) = &fh_click {
                    _w.focus(fh);
                }
                super::input::focus_view(id);
                super::input::click(
                    id,
                    x,
                    y,
                    ev.button,
                    false,
                    ev.click_count as i32,
                    super::input::modifiers(&ev.modifiers, Some(ev.button)),
                );
            },
        )
        .on_mouse_up(
            gpui::MouseButton::Left,
            move |ev: &gpui::MouseUpEvent, _w, _cx| {
                let (x, y) = local(ev.position, id);
                super::input::click(
                    id,
                    x,
                    y,
                    ev.button,
                    true,
                    ev.click_count as i32,
                    super::input::modifiers(&ev.modifiers, None),
                );
            },
        )
        // Правая и средняя кнопки: контекст-меню страницы и middle-click.
        // Левая идёт отдельным обработчиком выше (там же фокус).
        .on_mouse_down(
            gpui::MouseButton::Right,
            move |ev: &gpui::MouseDownEvent, _w, _cx| {
                let (x, y) = local(ev.position, id);
                super::input::click(
                    id,
                    x,
                    y,
                    ev.button,
                    false,
                    ev.click_count as i32,
                    super::input::modifiers(&ev.modifiers, Some(ev.button)),
                );
            },
        )
        .on_mouse_up(
            gpui::MouseButton::Right,
            move |ev: &gpui::MouseUpEvent, _w, _cx| {
                let (x, y) = local(ev.position, id);
                super::input::click(
                    id,
                    x,
                    y,
                    ev.button,
                    true,
                    ev.click_count as i32,
                    super::input::modifiers(&ev.modifiers, None),
                );
            },
        )
        .on_mouse_down(
            gpui::MouseButton::Middle,
            move |ev: &gpui::MouseDownEvent, _w, _cx| {
                let (x, y) = local(ev.position, id);
                super::input::click(
                    id,
                    x,
                    y,
                    ev.button,
                    false,
                    ev.click_count as i32,
                    super::input::modifiers(&ev.modifiers, Some(ev.button)),
                );
            },
        )
        .on_mouse_up(
            gpui::MouseButton::Middle,
            move |ev: &gpui::MouseUpEvent, _w, _cx| {
                let (x, y) = local(ev.position, id);
                super::input::click(
                    id,
                    x,
                    y,
                    ev.button,
                    true,
                    ev.click_count as i32,
                    super::input::modifiers(&ev.modifiers, None),
                );
            },
        )
        .on_key_down(move |ev: &gpui::KeyDownEvent, _w, cx| {
            // Esc при открытом меню страницы закрывает ЕГО, в страницу
            // не идёт (меню — наш слой, страница о нём не знает).
            if ev.keystroke.key == "escape" && super::menu_open() {
                if let Some(tx) = crate::host_link::event_tx() {
                    let _ = tx.try_send(crate::host_link::ShellEvent::WebMenu(None));
                }
                cx.stop_propagation();
                return;
            }
            // Палитра — глобальная, пропускаем наверх; остальное съедает
            // страница, иначе Root-биндинги (enter, ctrl-b…) срабатывали бы
            // параллельно с вводом в чат.
            let ks = &ev.keystroke;
            if ks.modifiers.control && ks.modifiers.shift && ks.key == "p" {
                return;
            }
            super::input::key(id, ks, false);
            cx.stop_propagation();
        })
        .on_key_up(move |ev: &gpui::KeyUpEvent, _w, cx| {
            super::input::key(id, &ev.keystroke, true);
            cx.stop_propagation();
        })
        .on_scroll_wheel(move |ev: &gpui::ScrollWheelEvent, window, _cx| {
            let (x, y) = local(ev.position, id);
            // Колесо gpui приходит либо в строках, либо в пикселях; CEF ждёт
            // пиксели прокрутки.
            let delta = ev.delta.pixel_delta(px(20.0));
            super::input::wheel(
                id,
                x,
                y,
                f32::from(delta.x),
                f32::from(delta.y),
                super::input::modifiers(&ev.modifiers, None),
            );
            let _ = window;
        })
}

/// Цвет подложки под кадром: фон редактора активной темы. Тема живёт в
/// статике, поэтому берём её прямо здесь — подложка обязана совпадать с
/// карточкой при любой смене темы.
fn p_bg() -> gpui::Rgba {
    crate::colors::rgba(kamin_theme::current_palette().editor_bg)
}

/// Прямоугольник вью: x, y, ширина, высота в логических px окна.
type ViewRect = (f32, f32, f32, f32);

/// Прямоугольник каждого вью, снятый на последней отрисовке.
static ORIGINS: std::sync::LazyLock<
    std::sync::Mutex<std::collections::HashMap<&'static str, ViewRect>>,
> = std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashMap::new()));

fn remember_origin(id: &'static str, bounds: &Bounds<Pixels>) {
    if let Ok(mut map) = ORIGINS.lock() {
        map.insert(
            id,
            (
                f32::from(bounds.origin.x),
                f32::from(bounds.origin.y),
                f32::from(bounds.size.width),
                f32::from(bounds.size.height),
            ),
        );
    }
}

/// Точка вью (CSS px) → ФИЗИЧЕСКИЕ экранные px. Нужна CEF для нативных
/// контекст-меню и попапов: без неё они открывались в углу экрана.
pub(crate) fn screen_point(id: &str, vx: i32, vy: i32) -> Option<(i32, i32)> {
    let (ox, oy, _, _) = ORIGINS.lock().ok()?.get(id).copied()?;
    let scale = super::browsers::scale_of(id);
    let cx = ((ox + vx as f32) * scale).round() as i32;
    let cy = ((oy + vy as f32) * scale).round() as i32;
    #[cfg(windows)]
    unsafe {
        use windows::Win32::Foundation::{HWND, POINT};
        use windows::Win32::Graphics::Gdi::ClientToScreen;
        let hwnd = crate::overlay::main_hwnd_isize();
        if hwnd == 0 {
            return Some((cx, cy));
        }
        let mut pt = POINT { x: cx, y: cy };
        let _ = ClientToScreen(HWND(hwnd as *mut _), &mut pt);
        Some((pt.x, pt.y))
    }
    #[cfg(not(windows))]
    Some((cx, cy))
}

/// Сколько кадр может быть «не того размера», прежде чем мы сдадимся и
/// растянем его. Ниже порога рисуем 1:1: при быстром драге сплиттера CEF
/// отстаёт на десятки мс, и растяжение на каждом шаге было заметнее, чем
/// незакрытая полоса подложки.
const STRETCH_AFTER_MS: u128 = 400;

/// Когда у вью начался период «кадр не того размера» (None — размер верный).
static MISMATCH_SINCE: std::sync::LazyLock<
    std::sync::Mutex<std::collections::HashMap<&'static str, std::time::Instant>>,
> = std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashMap::new()));

fn note_mismatch(id: &'static str) {
    if let Ok(mut m) = MISMATCH_SINCE.lock() {
        m.entry(id).or_insert_with(std::time::Instant::now);
    }
}

fn clear_mismatch(id: &'static str) {
    if let Ok(mut m) = MISMATCH_SINCE.lock() {
        m.remove(id);
    }
}

/// Сколько миллисекунд у вью держится неверный размер кадра.
fn mismatch_ms(id: &'static str) -> u128 {
    MISMATCH_SINCE
        .lock()
        .ok()
        .and_then(|m| m.get(id).map(|t| t.elapsed().as_millis()))
        .unwrap_or(0)
}

/// Последний известный РАЗМЕР вью (лог. px) с прошлой отрисовки — чтобы
/// поднимать браузер сразу в нужном размере вместо жёстких 800×600 (иначе
/// первые кадры приходят чужого размера и растягиваются: `big_mismatch`).
pub(crate) fn last_size(id: &str) -> Option<(f32, f32)> {
    let (_, _, w, h) = ORIGINS.lock().ok()?.get(id).copied()?;
    (w > 1.0 && h > 1.0).then_some((w, h))
}

/// Точка вью (CSS px) → ЛОГИЧЕСКИЕ px окна (для слоя оверлеев).
pub(crate) fn window_point(id: &str, vx: i32, vy: i32) -> Option<(f32, f32)> {
    let (ox, oy, _, _) = ORIGINS.lock().ok()?.get(id).copied()?;
    Some((ox + vx as f32, oy + vy as f32))
}

/// Лежит ли точка окна (лог. px) внутри какого-нибудь вью.
pub(crate) fn hit_any_view(x: f32, y: f32) -> bool {
    ORIGINS.lock().is_ok_and(|m| {
        m.values()
            .any(|(ox, oy, w, h)| x >= *ox && y >= *oy && x < ox + w && y < oy + h)
    })
}

/// Координаты курсора в системе вью: экранные минус начало его прямоугольника.
fn local(pos: gpui::Point<Pixels>, id: &'static str) -> (f32, f32) {
    let (ox, oy, _, _) = ORIGINS
        .lock()
        .ok()
        .and_then(|m| m.get(id).copied())
        .unwrap_or((0.0, 0.0, 0.0, 0.0));
    (f32::from(pos.x) - ox, f32::from(pos.y) - oy)
}
