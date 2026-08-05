//! Браузеры CEF по идентификатору вью: создание, размер, закрытие.
//!
//! Каждый вью — отдельный offscreen-браузер. Размер ему задаём МЫ: элемент
//! интерфейса на своём кадре сообщает границы, браузер под них подстраивается
//! (`plan/101-cef.md`, Ф2).
//!
//! **Про блокировки.** CEF отвечает СИНХРОННО и на том же потоке:
//! `was_resized()` тут же спрашивает у нас `view_rect`. Поэтому ни один вызов
//! в CEF не делается с захваченным мьютексом — иначе поток UI берёт свой же
//! мьютекс повторно и встаёт намертво (поймано: окно перестало отвечать
//! probe — `metric` работал, `screenshot` нет).

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use cef::rc::*;
use cef::*;

use super::render_handler::ViewRender;

/// Живые браузеры по id вью.
static BROWSERS: LazyLock<Mutex<HashMap<String, Browser>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Запрошенный размер вью в ЛОГИЧЕСКИХ px и масштаб экрана. ОТДЕЛЬНО от
/// браузеров: эту карту читают `view_rect` и `screen_info` изнутри вызова CEF,
/// когда мьютекс браузеров может быть занят.
///
/// Логические, а не физические: CEF считает размер вью в CSS-пикселях и сам
/// умножает его на масштаб. Отдашь физические — страница сверстается шире
/// панели и выглядит мелкой (поймано глазами на первом же кадре).
/// Размер вью и масштаб экрана.
type ViewSize = (i32, i32, f32);

static SIZES: LazyLock<Mutex<HashMap<String, ViewSize>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

cef::wrap_client! {
    struct ViewClient {
        render: RenderHandler,
        life: LifeSpanHandler,
        display: DisplayHandler,
        download: DownloadHandler,
        context_menu: ContextMenuHandler,
        requests: RequestHandler,
    }
    impl Client {
        fn request_handler(&self) -> Option<RequestHandler> {
            Some(self.requests.clone())
        }
        fn render_handler(&self) -> Option<RenderHandler> {
            Some(self.render.clone())
        }
        fn life_span_handler(&self) -> Option<LifeSpanHandler> {
            Some(self.life.clone())
        }
        fn display_handler(&self) -> Option<DisplayHandler> {
            Some(self.display.clone())
        }
        fn download_handler(&self) -> Option<DownloadHandler> {
            Some(self.download.clone())
        }
        fn context_menu_handler(&self) -> Option<ContextMenuHandler> {
            Some(self.context_menu.clone())
        }
    }
}

cef::wrap_display_handler! {
    struct ViewDisplay {
        id: String,
    }
    impl DisplayHandler {
        /// Форма курсора: в offscreen-режиме окном управляем мы, CEF лишь
        /// сообщает желаемую форму (рука над ссылкой, каретка над текстом).
        fn on_cursor_change(
            &self,
            _browser: Option<&mut Browser>,
            _cursor: cef::sys::HCURSOR,
            type_: CursorType,
            _custom: Option<&CursorInfo>,
        ) -> ::std::os::raw::c_int {
            if super::cursors::set(&self.id, super::cursors::from_cef(*type_.as_ref())) {
                super::repaint_requested();
            }
            1
        }
    }
}

cef::wrap_download_handler! {
    struct ViewDownload {
        id: String,
    }
    impl DownloadHandler {
        /// Скачивание разрешено любым ссылкам и кнопкам страницы.
        fn can_download(
            &self,
            _browser: Option<&mut Browser>,
            _url: Option<&CefString>,
            _request_method: Option<&CefString>,
        ) -> ::std::os::raw::c_int {
            1
        }

        /// Старт скачивания: системный диалог «Сохранить как» показывает сам
        /// CEF (`show_dialog = 1`); путь не навязываем — выберет пользователь.
        fn on_before_download(
            &self,
            _browser: Option<&mut Browser>,
            _item: Option<&mut DownloadItem>,
            _suggested_name: Option<&CefString>,
            callback: Option<&mut BeforeDownloadCallback>,
        ) -> ::std::os::raw::c_int {
            if let Some(cb) = callback {
                cb.cont(None, 1);
            }
            1
        }
    }
}

cef::wrap_request_handler! {
    struct ViewRequests {
        id: String,
    }
    impl RequestHandler {
        /// Renderer этого вью умер (падение, OOM, убит системой).
        ///
        /// Сам ребёнок в этот момент записать о себе уже ничего не может —
        /// пишем МЫ, и в тот же `crash.log`, что и его перехватчик. Без этого
        /// у человека оставалась только системная модалка «KaminIDE has
        /// stopped working» при живой оболочке (скрин с рабочей машины).
        ///
        /// И сразу лечим: страница после смерти renderer'а мертва навсегда,
        /// пока её не перезагрузить — иначе панель просто застывает.
        fn on_render_process_terminated(
            &self,
            browser: Option<&mut Browser>,
            status: TerminationStatus,
            error_code: ::std::os::raw::c_int,
            error_string: Option<&CefString>,
        ) {
            let reason = error_string.map(|s| s.to_string()).unwrap_or_default();
            kamin_crash::note(&format!(
                "[КРАХ] renderer вью «{}» умер: статус {:?}, код {error_code}, {reason}",
                self.id,
                *status.as_ref()
            ));
            super::diag::renderer_died();
            if let Some(browser) = browser
                && let Some(mut frame) = browser.main_frame()
            {
                use cef::ImplFrame as _;
                let url = CefStringUtf16::from(&frame.url());
                frame.load_url(Some(&url));
            }
            super::repaint_requested();
        }
    }
}

cef::wrap_life_span_handler! {
    struct ViewLife {
        id: String,
    }
    impl LifeSpanHandler {
        /// Браузер готов — только ЗДЕСЬ мы получаем на него ссылку: создание
        /// асинхронное, потому что синхронное разрешено лишь с потока CEF.
        fn on_after_created(&self, browser: Option<&mut Browser>) {
            let Some(browser) = browser else { return };
            // В offscreen-режиме браузер считается скрытым, пока не сказано
            // иначе: скрытый не рисует ни одного кадра.
            if let Some(host) = browser.host() {
                host.was_hidden(0);
            }
            if let Ok(mut map) = BROWSERS.lock() {
                map.insert(self.id.clone(), browser.clone());
            }
            if let Ok(mut set) = STARTING.lock() {
                set.remove(&self.id);
            }
            // Отложенное уведомление о смене scale: пока браузер создавался,
            // DPI мог измениться (RDP-реконнект), а `on_browser` тогда терял
            // задачу молча — CEF оставался в старом масштабе навсегда.
            let pending = NOTIFY_PENDING
                .lock()
                .map(|mut p| p.remove(&self.id))
                .unwrap_or(false);
            if pending && let Some(host) = browser.host() {
                host.notify_screen_info_changed();
                host.was_resized();
            }
        }
    }
}

/// Вью, которым смена scale не доехала (браузер ещё создавался).
static NOTIFY_PENDING: LazyLock<Mutex<std::collections::HashSet<String>>> =
    LazyLock::new(|| Mutex::new(std::collections::HashSet::new()));

/// Вью, чей браузер уже заказан, но ещё не создан. Без этого списка отрисовка
/// заказывала бы его каждый кадр: `open` зовётся из `render`.
static STARTING: LazyLock<Mutex<std::collections::HashSet<String>>> =
    LazyLock::new(|| Mutex::new(std::collections::HashSet::new()));

/// Когда браузер вью был заказан — для ватчдога «вью без кадров».
static OPENED_AT: LazyLock<Mutex<HashMap<String, std::time::Instant>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Давность заказа браузера вью (None — не заказывался).
pub(crate) fn opened_ago(id: &str) -> Option<std::time::Duration> {
    OPENED_AT.lock().ok()?.get(id).map(|t| t.elapsed())
}

/// Показать в вью нашу страницу (HTML из стора моста).
///
/// HTML отдаём `data:`-ссылкой: свой обработчик схемы — отдельная работа, а
/// страницы моста весят десятки килобайт и в ссылку помещаются.
/// Выполнить JS в странице вью (расширение → страница).
pub(crate) fn execute_script(id: &str, js: &str) {
    let js = js.to_string();
    super::input::on_browser(id, move |host| {
        if let Some(frame) = host.browser().and_then(|b| b.main_frame()) {
            // Адрес скрипта обязателен: с пустым (NULL) местом вызова CEF
            // валит процесс без паники — ловилось на первой же доставке
            // сообщения странице.
            let url: cef::CefString = "kamin://deliver".into();
            frame.execute_java_script(Some(&js.as_str().into()), Some(&url), 0);
        }
    });
}

/// Шаг назад по истории вью.
pub(crate) fn go_back(id: &str) {
    super::input::on_browser(id, |host| {
        if let Some(browser) = host.browser() {
            browser.go_back();
        }
    });
}

/// Шаг вперёд по истории вью.
pub(crate) fn go_forward(id: &str) {
    super::input::on_browser(id, |host| {
        if let Some(browser) = host.browser() {
            browser.go_forward();
        }
    });
}

/// Открыть инструменты разработчика отдельным окном.
pub(crate) fn open_devtools(id: &str) {
    super::input::on_browser(id, |host| {
        let info = cef::WindowInfo {
            bounds: cef::Rect {
                x: 100,
                y: 100,
                width: 1200,
                height: 800,
            },
            ..Default::default()
        };
        host.show_dev_tools(Some(&info), None, None, None);
    });
}

/// Открыть в существующем вью другой адрес.
pub(crate) fn navigate(id: &str, url: &str) {
    let url = url.to_string();
    super::input::on_browser(id, move |host| {
        if let Some(frame) = host.browser().and_then(|b| b.main_frame()) {
            frame.load_url(Some(&url.as_str().into()));
        }
    });
}

/// Перечитать страницу вью: HTML отдаст обработчик схемы.
pub(crate) fn reload(id: &str) {
    super::input::on_browser(id, |host| {
        if let Some(browser) = host.browser() {
            browser.reload_ignore_cache();
        }
    });
}

/// Открыт ли уже браузер этого вью.
pub(crate) fn exists(id: &str) -> bool {
    BROWSERS.lock().map(|m| m.contains_key(id)).unwrap_or(false)
}

/// Заказан ли браузер вью (создан или в процессе создания).
fn claimed(id: &str) -> bool {
    exists(id) || STARTING.lock().map(|s| s.contains(id)).unwrap_or(false)
}

/// Создать браузер вью и загрузить `url`. Повторный вызов игнорируется.
pub(crate) fn open(id: &str, url: &str, width: i32, height: i32, scale: f32) {
    if !super::process::is_live() || claimed(id) {
        return;
    }
    set_wanted(id, width, height, scale);
    if let Ok(mut set) = STARTING.lock() {
        set.insert(id.to_string());
    }
    if let Ok(mut map) = OPENED_AT.lock() {
        map.insert(id.to_string(), std::time::Instant::now());
    }

    let mut client = ViewClient::new(
        ViewRender::new(id.to_string()),
        ViewLife::new(id.to_string()),
        ViewDisplay::new(id.to_string()),
        ViewDownload::new(id.to_string()),
        super::context_menu::ViewContextMenu::new(id.to_string()),
        ViewRequests::new(id.to_string()),
    );
    let window_info = WindowInfo {
        windowless_rendering_enabled: 1,
        // В software-режиме (RDP) общие текстуры выключены: с ними CEF без
        // GPU-композитинга не шлёт кадры вовсе (см. `web::sw_mode`).
        // Кадр — готовой текстурой D3D11, без копий в оперативке
        // (`plan/101-cef.md`, шаг B). Тогда вместо `on_paint` зовётся
        // `on_accelerated_paint`.
        shared_texture_enabled: if super::sw_mode() { 0 } else { 1 },
        // Кадр рисует САМ Chromium — по факту изменений на странице.
        //
        // Пробовал «кадр только по нашему запросу»: запрос уходил из
        // отрисовки, приход кадра просил перерисовку, та — новый запрос.
        // Получился круг на полной скорости, окно захлёбывалось и переставало
        // отзываться на ресайз. Здесь же круга нет: статичная страница кадров
        // не шлёт, а изменения (в том числе ресайз) их порождают сами.
        external_begin_frame_enabled: 0,
        ..Default::default()
    };
    // При внешнем begin-frame `windowless_frame_rate` игнорируется — темп
    // задаём мы (документация CEF, подтверждено демо cef-mixer).
    let settings = BrowserSettings {
        // 120: на ресайзе каждый кадр дорог, а на статичной странице Chromium
        // кадров всё равно не шлёт — простой это не греет. Под RDP кадр = сеть,
        // но требование юзера — плавность скролла чата «как Chrome»: Chrome под
        // RDP рендерит 60 — ставим те же 60, лаг ввода снимают глушённые
        // ФОНОВЫЕ анимации, не кап интерактивных кадров (оценивается при
        // создании вью; сменился транспорт — новые вью получат новый темп).
        windowless_frame_rate: if crate::win_integration::reduce_motion() {
            60
        } else {
            120
        },
        // Непрозрачный фон под цвет карточки. Пока Chromium не перевёрстывал
        // страницу под новый размер, незакрытая часть кадра приходит
        // прозрачной — и рисовалась ЧЁРНОЙ полосой (видно на быстром драге).
        // С этим фоном полоса совпадает с карточкой и в глаза не бросается.
        background_color: 0xFF1E1E2E,
        ..Default::default()
    };
    // АСИНХРОННО: синхронный вариант требует потока CEF, а он теперь свой
    // (`process.rs`). Ссылку на браузер отдаст `on_after_created`.
    let ok = browser_host_create_browser(
        Some(&window_info),
        Some(&mut client),
        Some(&url.into()),
        Some(&settings),
        None,
        None,
    );
    if ok != 1 {
        eprintln!("[cef] заказ браузера вью {id} отклонён");
        if let Ok(mut set) = STARTING.lock() {
            set.remove(id);
        }
    }
}

/// Размер вью в логических px (это читает `view_rect`).
pub(crate) fn wanted_size(id: &str) -> Option<(i32, i32)> {
    SIZES.lock().ok()?.get(id).map(|(w, h, _)| (*w, *h))
}

/// Масштаб экрана для вью (это читает `screen_info`).
pub(crate) fn scale_of(id: &str) -> f32 {
    SIZES
        .lock()
        .ok()
        .and_then(|m| m.get(id).map(|(_, _, s)| *s))
        .unwrap_or(1.0)
}

/// Запомнить желаемый размер и масштаб. `true` — что-то изменилось.
fn set_wanted(id: &str, width: i32, height: i32, scale: f32) -> bool {
    let (w, h) = (width.max(1), height.max(1));
    let Ok(mut sizes) = SIZES.lock() else {
        return false;
    };
    let next = (w, h, scale);
    if sizes.get(id).is_some_and(|cur| *cur == next) {
        return false;
    }
    sizes.insert(id.to_string(), next);
    true
}

/// Ссылка на браузер — чтобы звать CEF уже без мьютекса.
pub(crate) fn handle(id: &str) -> Option<Browser> {
    BROWSERS.lock().ok()?.get(id).cloned()
}

/// Идентификаторы всех живых браузеров.
pub(crate) fn ids() -> Vec<String> {
    BROWSERS
        .lock()
        .map(|m| m.keys().cloned().collect())
        .unwrap_or_default()
}

/// Закрыть браузер вью и забыть всё его состояние. Возврат вью поднимет
/// браузер заново из HTML-стора (`ensure_html_view`) — как при первом открытии.
pub(crate) fn close(id: &str) {
    let browser = BROWSERS.lock().ok().and_then(|mut m| m.remove(id));
    if let Ok(mut m) = SIZES.lock() {
        m.remove(id);
    }
    if let Ok(mut m) = PENDING.lock() {
        m.remove(id);
    }
    if let Ok(mut m) = NUDGES.lock() {
        m.remove(id);
    }
    if let Ok(mut d) = DANCE.lock() {
        d.remove(id);
    }
    if let Ok(mut m) = OPENED_AT.lock() {
        m.remove(id);
    }
    let Some(browser) = browser else { return };
    // Методы браузера живут на потоке CEF; force=1 — без beforeunload-диалогов.
    super::input::run_on_cef(move || {
        if let Some(host) = browser.host() {
            host.close_browser(1);
        }
    });
}

/// След подталкиваний вью: (начало серии, последнее подталкивание).
/// Серия рвётся, когда подталкиваний не было дольше 100 мс.
static NUDGES: LazyLock<Mutex<HashMap<String, (std::time::Instant, std::time::Instant)>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// «Танец на 1px» взведён: следующий `view_rect` отдаст высоту на пиксель
/// меньше — это сбрасывает залипший `hold_resize` внутри CEF.
static DANCE: LazyLock<Mutex<std::collections::HashSet<String>>> =
    LazyLock::new(|| Mutex::new(std::collections::HashSet::new()));

/// Съесть флаг танца (зовёт `view_rect`).
pub(crate) fn take_dance(id: &str) -> bool {
    DANCE.lock().map(|mut d| d.remove(id)).unwrap_or(false)
}

/// Повторить запрос кадра нужного размера.
///
/// Chromium рисует по изменениям, и один запрос иногда пропадает на быстром
/// драге: панель остаётся с кадром прежнего размера (ловится стендом
/// `scripts/check_web_resize.py`). Дёргаем только при реальном расхождении.
///
/// Если расхождение живёт дольше [`STUCK_AFTER`] — это залипший `hold_resize`
/// CEF (известный баг cefsharp#4953: после серии `WasResized` с несовпадающими
/// кадрами новые вызовы игнорируются). Обход оттуда же — «танец на 1px»: один
/// `view_rect` отвечает высотой на пиксель меньше, следующий — настоящей, и
/// цепочка ресайза заводится заново.
pub(crate) fn nudge(id: &str) {
    let dance = {
        let now = std::time::Instant::now();
        let Ok(mut map) = NUDGES.lock() else {
            return;
        };
        let entry = map
            .entry(id.to_string())
            .or_insert((now, now - std::time::Duration::from_secs(1)));
        // Пауза в подталкиваниях — расхождение успевало сходиться; новая серия.
        if now.duration_since(entry.1) > std::time::Duration::from_millis(100) {
            entry.0 = now;
        }
        let stuck = now.duration_since(entry.0) >= STUCK_AFTER;
        entry.1 = now;
        if stuck {
            entry.0 = now;
            if let Ok(mut d) = DANCE.lock() {
                d.insert(id.to_string());
            }
        }
        stuck
    };
    super::input::on_browser(id, move |host| {
        host.was_resized();
        if dance {
            // Второй вызов — уже с настоящим размером (флаг съел первый).
            host.was_resized();
        }
        // Одного `was_resized` мало: Chromium пересчитывает раскладку, но на
        // статичной странице кадр не шлёт — панель остаётся с прежним.
        host.invalidate(PaintElementType::from(
            cef::sys::cef_paint_element_type_t::PET_VIEW,
        ));
    });
}

/// Вью, которым CEF ещё не сказали новый размер, и когда сказали в прошлый раз.
static PENDING: LazyLock<Mutex<HashMap<String, (bool, std::time::Instant)>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Как часто вообще беспокоим CEF ресайзом. Каждый кадр нельзя: у CEF есть
/// баг залипания (`hold_resize`) — при потоке `WasResized` с несовпадающими
/// кадрами дальнейшие вызовы игнорируются (issue cefsharp#4953; мы ловили
/// «просили 480, он спросил 638 и замер»). 33 мс — два кадра дисплея:
/// быстрее прежних 50 мс, но всё ещё серия, а не поток.
const RESIZE_EVERY: std::time::Duration = std::time::Duration::from_millis(33);

/// Если размер завис дольше этого — применяем «танец на 1px» (см. [`nudge`]).
const STUCK_AFTER: std::time::Duration = std::time::Duration::from_millis(250);

/// Сообщить браузеру новый размер. Зовётся КАЖДЫЙ кадр из отрисовки панели:
/// сначала запоминаем цель, потом, не чаще `RESIZE_EVERY`, говорим CEF.
/// Досылка гарантирована: флаг снимается только после отправки, а звать нас
/// продолжают каждый кадр.
/// Шаг квантования размера ПОД ДРАГОМ ручки. Каждый `was_resized` — полная
/// перевёрстка страницы в renderer Chromium (30-150мс на RDP). Слали ~30
/// команд/с, и каждая ОТМЕНЯЛА незавершённую перевёрстку: CEF не «не
/// успевал», он бесконечно начинал заново, а кадры приходили чужого размера.
/// С квантованием за драг уходит 3-4 команды, каждая доводится до конца, и
/// кадр совпадает с панелью — растягивать нечего.
const DRAG_STEP_PX: i32 = 32;

pub(crate) fn resize(id: &str, width: i32, height: i32, scale: f32) {
    // Точный размер — по отпусканию ручки (драг закончился); под драгом
    // округляем ВВЕРХ до шага, чтобы кадр был не меньше панели.
    let (width, height) = if crate::state::drag_flag::is_dragging() {
        let q = |v: i32| ((v + DRAG_STEP_PX - 1) / DRAG_STEP_PX) * DRAG_STEP_PX;
        (q(width), q(height))
    } else {
        (width, height)
    };
    // Смена МАСШТАБА (не размера) — DPI доехал позже создания браузера
    // (загрузка под RDP): одного was_resized мало, CEF перечитывает scale
    // только по notify_screen_info_changed. Без него страница жила в чужом
    // масштабе до случайного пересоздания («вебвью вдвое меньше», прод).
    // Уведомление — строго ПОСЛЕ set_wanted: колбэк screen_info читает SIZES.
    let scale_changed = wanted_size(id).is_some() && scale_of(id) != scale;
    let changed = set_wanted(id, width, height, scale);
    if scale_changed {
        // Вне дебаунса ниже: его «отложили» съел бы одноразовый флаг — на
        // следующем вызове scale_of уже новый и уведомление не ушло бы никогда.
        //
        // Если браузер ещё STARTING, `on_browser` МОЛЧА теряет задачу — CEF
        // остаётся в старом scale навсегда, и все кадры приходят в чужом
        // физическом размере («вебвью вдвое меньше», вечный big_mismatch).
        // Ставим флаг и досылаем после создания (`on_after_created`).
        if handle(id).is_some() {
            super::input::on_browser(id, |host| {
                host.notify_screen_info_changed();
                host.was_resized();
            });
        } else if let Ok(mut p) = NOTIFY_PENDING.lock() {
            p.insert(id.to_string());
        }
    }
    if changed && let Ok(mut pending) = PENDING.lock() {
        {
            pending
                .entry(id.to_string())
                .and_modify(|(dirty, _)| *dirty = true)
                .or_insert((true, std::time::Instant::now() - RESIZE_EVERY));
        }
    }
    let send = match PENDING.lock() {
        Ok(mut pending) => match pending.get_mut(id) {
            Some((dirty, at)) if *dirty && at.elapsed() >= RESIZE_EVERY => {
                *dirty = false;
                *at = std::time::Instant::now();
                true
            }
            _ => false,
        },
        Err(_) => false,
    };
    if !send {
        // Отложили — значит нужен ЕЩЁ ОДИН кадр, иначе досылать будет некому:
        // размер мы отправляем только из отрисовки панели, а отрисовка
        // случается лишь по заказу. После драга заказов нет (статичная
        // страница кадров не шлёт), и панель навсегда оставалась с чужим
        // размером (замер стендом: просили 480, кадр остался 689).
        if PENDING
            .lock()
            .map(|p| p.get(id).is_some_and(|(dirty, _)| *dirty))
            .unwrap_or(false)
        {
            super::repaint_requested();
        }
        return;
    }
    // ВАЖНО: `was_resized` — метод потока CEF. Вызов из отрисовки (наш поток)
    // то срабатывал, то нет, и панель оставалась со старым кадром — ровно
    // та же ошибка, что валила приложение на вводе.
    super::input::on_browser(id, |host| {
        host.was_resized();
    });
}
