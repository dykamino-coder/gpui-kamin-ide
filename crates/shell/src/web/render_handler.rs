//! Приём кадров от CEF: размер вью и готовый буфер пикселей.
//!
//! Хендлер знает только свой идентификатор вью — кадр кладётся в общую карту
//! (`frames.rs`), а рисует его элемент интерфейса на своём кадре.

use cef::rc::*;
use cef::*;

cef::wrap_render_handler! {
    pub(crate) struct ViewRender {
        id: String,
    }
    impl RenderHandler {
        /// Размер вью в логических px. Берём тот, что запросил элемент
        /// интерфейса; пока не запросил — минимальный ненулевой (CEF не
        /// принимает нулевой прямоугольник).
        fn view_rect(&self, _browser: Option<&mut Browser>, rect: Option<&mut Rect>) {
            if let Some(rect) = rect {
                let (w, h) = super::browsers::wanted_size(&self.id).unwrap_or((1, 1));
                // «Танец на 1px»: разово отдаём высоту меньше — сбрасывает
                // залипший hold_resize CEF (см. `browsers::nudge`).
                let h = if super::browsers::take_dance(&self.id) {
                    (h - 1).max(1)
                } else {
                    h
                };
                rect.x = 0;
                rect.y = 0;
                rect.width = w.max(1);
                rect.height = h.max(1);
            }
        }

        /// Точка вью → экран: без неё нативные контекст-меню и попапы CEF
        /// открывались в углу экрана.
        fn screen_point(
            &self,
            _browser: Option<&mut Browser>,
            view_x: ::std::os::raw::c_int,
            view_y: ::std::os::raw::c_int,
            screen_x: Option<&mut ::std::os::raw::c_int>,
            screen_y: Option<&mut ::std::os::raw::c_int>,
        ) -> ::std::os::raw::c_int {
            let Some((sx, sy)) = super::element::screen_point(&self.id, view_x, view_y) else {
                return 0;
            };
            if let Some(x) = screen_x {
                *x = sx;
            }
            if let Some(y) = screen_y {
                *y = sy;
            }
            1
        }

        /// Попап страницы (дропдаун `<select>`): показать/спрятать.
        fn on_popup_show(&self, _browser: Option<&mut Browser>, show: ::std::os::raw::c_int) {
            if show == 0 {
                super::popup::hide(&self.id);
                super::repaint_requested();
            }
        }

        /// Прямоугольник попапа в координатах вью (CSS px).
        fn on_popup_size(&self, _browser: Option<&mut Browser>, rect: Option<&Rect>) {
            if let Some(r) = rect {
                super::popup::set_rect(&self.id, r.x, r.y, r.width, r.height);
            }
        }

        /// Масштаб экрана: без него CEF рисует как при 100 %, и на мониторе
        /// с масштабированием страница выходит мелкой и мыльной.
        fn screen_info(
            &self,
            _browser: Option<&mut Browser>,
            screen_info: Option<&mut ScreenInfo>,
        ) -> ::std::os::raw::c_int {
            let Some(info) = screen_info else { return 0 };
            let (w, h) = super::browsers::wanted_size(&self.id).unwrap_or((1, 1));
            info.device_scale_factor = super::browsers::scale_of(&self.id);
            let rect = Rect { x: 0, y: 0, width: w, height: h };
            info.rect = rect.clone();
            info.available_rect = rect;
            1
        }

        /// Готовый кадр ГОТОВОЙ текстурой: копий нет, приезжает только
        /// общий handle. Основной путь, когда есть GPU.
        fn on_accelerated_paint(
            &self,
            _browser: Option<&mut Browser>,
            type_: PaintElementType,
            _dirty_rects: Option<&[Rect]>,
            info: Option<&AcceleratedPaintInfo>,
        ) {
            let Some(info) = info else { return };
            if type_.as_ref() == &cef::sys::cef_paint_element_type_t::PET_POPUP {
                // Дропдаун `<select>`: отдельная текстура поверх кадра.
                super::popup::put(
                    &self.id,
                    info.shared_texture_handle as isize,
                    info.extra.coded_size.width,
                    info.extra.coded_size.height,
                );
                super::repaint_requested();
                return;
            }
            if type_.as_ref() != &cef::sys::cef_paint_element_type_t::PET_VIEW {
                return;
            }
            // РАЗНЫЕ размеры: `coded_size` — выровненный буфер Chromium (может
            // быть шире и выше), `visible_rect` — та часть, где реально лежит
            // страница. Если рисовать весь буфер, по краям идут ЧЁРНЫЕ ПОЛОСЫ
            // (поймано пользователем на скриншоте ресайза).
            let (w, h) = (info.extra.coded_size.width, info.extra.coded_size.height);
            // СТРАНИЦА живёт в `content_rect` — со смещением внутри буфера
            // (ловили `контент 482×369 @0,64`); остальное прозрачно и в
            // композиции читается чёрным. Показываем только её.
            let c = &info.extra.content_rect;
            let content = (
                c.x.max(0),
                c.y.max(0),
                c.width.max(1).min(w),
                c.height.max(1).min(h),
            );
            static LAST: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let key = ((w as u64) << 32) | (h as u64 & 0xffff_ffff);
            if LAST.swap(key, std::sync::atomic::Ordering::Relaxed) != key {
                // Печатаем размер СТРАНИЦЫ (content_rect): буфер бывает
                // выровнен с запасом, а рисуется и сверяется стендами страница.
                println!("[cef] текстура вью {}: {}×{}", self.id, content.2, content.3);
            }
            super::diag::frame();
            super::shared_texture::put(
                &self.id,
                info.shared_texture_handle as isize,
                w,
                h,
                content,
            );
            super::repaint_requested();
        }

        /// Готовый кадр в оперативке — запасной путь: без GPU (виртуалка,
        /// RDP) Chromium присылает кадр только так. `type_` бывает ещё `Popup` — это выпадающие списки
        /// самой страницы; их рисуем следующим шагом, сейчас берём только
        /// основной слой.
        fn on_paint(
            &self,
            _browser: Option<&mut Browser>,
            type_: PaintElementType,
            dirty_rects: Option<&[Rect]>,
            buffer: *const u8,
            width: ::std::os::raw::c_int,
            height: ::std::os::raw::c_int,
        ) {
            if buffer.is_null() || width <= 0 || height <= 0 {
                return;
            }
            if type_.as_ref() == &cef::sys::cef_paint_element_type_t::PET_POPUP {
                // Дропдаун `<select>` в software-режиме (RDP): буфер попапа
                // отдельным кадром, рисуется поверх основного в element.
                let len = (width as usize) * (height as usize) * 4;
                let pixels = unsafe { std::slice::from_raw_parts(buffer, len) };
                super::popup::put_sw(&self.id, pixels, width, height);
                super::repaint_requested();
                return;
            }
            if type_.as_ref() != &cef::sys::cef_paint_element_type_t::PET_VIEW {
                return;
            }
            // Разовая метка: без неё «кадров нет» не отличить от «кадр есть,
            // но не рисуется».
            // Печатаем только СМЕНУ размера: так видно, догоняет ли браузер
            // панель, и не засоряет лог каждым кадром.
            static LAST: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let key = ((width as u64) << 32) | (height as u64 & 0xffff_ffff);
            if LAST.swap(key, std::sync::atomic::Ordering::Relaxed) != key {
                println!("[cef] кадр вью {}: {width}×{height}", self.id);
            }
            // Кадр — в общий счётчик диагностики: стенды и строка «за
            // секунду» считают кадры одинаково в GPU- и software-режиме.
            super::diag::frame();
            // #76(3): кадр с ПУСТЫМ dirty (CEF прислал, но ничего не менялось
            // — в логах юзера «dirty 0%» регулярен) — копия и перезаливка
            // текстуры впустую, плюс лишний кадр окна. Скип целиком.
            if let Some(rs) = dirty_rects
                && rs
                    .iter()
                    .all(|r| r.width.max(0) as u64 * r.height.max(0) as u64 == 0)
            {
                return;
            }
            let len = (width as usize) * (height as usize) * 4;
            let pixels = unsafe { std::slice::from_raw_parts(buffer, len) };
            // #76, ввод 1.5с: телеметрия sw-пути. Сколько кадра РЕАЛЬНО
            // изменилось (dirty от CEF; при печати это полоска инпута, а
            // перезаливаем мы пока весь кадр), почём копия и как часто кадры.
            let put_started = std::time::Instant::now();
            let dropped = super::frames::put(&self.id, pixels, width, height);
            let full = (width as u64) * (height as u64);
            let dirty: u64 = dirty_rects
                .map(|rs| {
                    rs.iter()
                        .map(|r| (r.width.max(0) as u64) * (r.height.max(0) as u64))
                        .sum()
                })
                .unwrap_or(full);
            let pct = (dirty * 100).checked_div(full).map_or(100, |v| v.min(100) as u32);
            let now_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| (d.as_millis() & 0xffff_ffff) as u32)
                .unwrap_or(0);
            super::diag::sw_put(put_started.elapsed().as_micros() as u32, pct, now_ms);
            // Вытесненный кадр держит текстуру в атласе gpui — освободим её на
            // следующем проходе отрисовки (сюда нас зовут с чужого потока).
            if let Some(old) = dropped {
                super::frames::retire(old);
            }
            super::repaint_requested();
        }
    }
}
