//! Кто из вью сейчас на экране — и знают ли об этом Chromium и exthost.
//!
//! Раскладка решает состав видимых на каждом кадре (`RootView::sync_panels`),
//! а здесь он превращается в два внешних сигнала.
//!
//! **Chromium.** Offscreen-браузер рисует, пока ему не сказали обратное. Мы
//! говорили `was_hidden(0)` один раз при создании и больше к теме не
//! возвращались — поэтому вью, ушедшее с экрана, продолжало рисовать на полной
//! частоте и не тротлило свои таймеры и анимации. Полевой лог показывал это
//! прямо: блок в 300 секунд, где Chromium слал 56 кадров в секунду при НУЛЕ
//! кадров окна, и зонд из самой страницы, отвечавший `visibilityState=visible`
//! для панели, которой на экране не было. Дороже всего это обходилось чату: он
//! в `NEVER_REAP`, то есть не выгружается никогда, и скрытым жил вечно.
//!
//! **exthost.** Канал `kamin:webview:viewState` он ПРИНИМАЛ, но никто не слал
//! (кроме выгрузки): вью считалось вечно видимым, посты копились в очередь
//! невидимого iframe, а его `resolvedHtml` (~1,6 МБ на вью) держался до
//! dispose, которого после reap не бывает.
//!
//! **Про асимметрию.** Будим СРАЗУ, усыпляем через тик насоса. Состав видимых
//! пересчитывается на каждое событие и на отдельных кадрах «дребезжит» (тул ещё
//! не доехал в реестр); мгновенное усыпление на таком дребезге останавливало бы
//! рендер живой панели, а секунда лишнего рисования не стоит ничего. Опоздать с
//! пробуждением, наоборот, значит показать пользователю застывшую картинку.

use std::collections::HashSet;
use std::sync::{LazyLock, Mutex};

use cef::*;

/// Состав видимых и состав усыплённых — ПОД ОДНИМ замком.
///
/// Раздельные мьютексы давали гонку: тик насоса успевал прочитать «вью скрыто»,
/// UI-поток между этим и записью показывал вью (будить было нечего — усыпить
/// ещё не успели), а тик следом усыплял уже видимое. Пробуждения больше не
/// случалось — вью в составе видимых, менять нечего, — и панель оставалась
/// застывшей до следующего скрытия.
#[derive(Default)]
struct Views {
    /// На экране по решению раскладки.
    visible: HashSet<String>,
    /// Кому уже сказали «ты скрыт». Отдельно от `visible`, потому что
    /// усыпление отложено на тик: состав видимых меняется чаще, чем мы
    /// беспокоим CEF.
    asleep: HashSet<String>,
}

static VIEWS: LazyLock<Mutex<Views>> = LazyLock::new(|| Mutex::new(Views::default()));

/// Сообщить, какие вью сейчас видимы. Зовётся после каждого события.
pub fn mark_visible(ids: Vec<String>) {
    let next: HashSet<String> = ids.into_iter().collect();
    let Ok(mut views) = VIEWS.lock() else {
        return;
    };
    let appeared: Vec<String> = next.difference(&views.visible).cloned().collect();
    views.visible = next;
    wake(&mut views, appeared);
}

/// Расширить множество видимых, не сбрасывая прежнее: для кадров с неполным
/// снапшотом реестра тулов — полный ранний выход замораживал TTL скрытых
/// вью навсегда (они не выгружались), а сброс «скрывал» живые панели.
pub fn mark_visible_union(ids: Vec<String>) {
    let Ok(mut views) = VIEWS.lock() else {
        return;
    };
    let appeared: Vec<String> = ids
        .into_iter()
        .filter(|id| views.visible.insert(id.clone()))
        .collect();
    wake(&mut views, appeared);
}

/// Копия множества видимых — для выгрузки и ватчдога (`web::reap_hidden`,
/// `web::respawn_stalled`).
pub(crate) fn visible_set() -> HashSet<String> {
    VIEWS.lock().map(|v| v.visible.clone()).unwrap_or_default()
}

/// На экране ли вью прямо сейчас.
pub(crate) fn is_visible(id: &str) -> bool {
    VIEWS.lock().is_ok_and(|v| v.visible.contains(id))
}

/// Значение для `was_hidden` при создании браузера: создание асинхронное, и
/// пока оно шло, вью могло уйти с экрана.
pub(crate) fn hidden_flag(id: &str) -> ::std::os::raw::c_int {
    if is_visible(id) { 0 } else { 1 }
}

/// Усыпить всё, что ушло с экрана. Зовётся раз в секунду из насоса — задержка
/// здесь намеренная, см. «про асимметрию» в шапке модуля.
pub(crate) fn sleep_hidden() {
    // Список браузеров берём ДО своего замка: два чужих замка разом — верный
    // способ однажды получить взаимную блокировку.
    let ids = super::browsers::ids();
    let Ok(mut views) = VIEWS.lock() else {
        return;
    };
    for id in ids {
        // `insert` вернёт false, если это вью уже усыплено: повторять нечего,
        // а канал exthost не должен получать одно и то же каждую секунду, пока
        // панель закрыта.
        if views.visible.contains(&id) || !views.asleep.insert(id.clone()) {
            continue;
        }
        set_hidden(&id, true);
        notify_view_state(&id, false);
    }
}

/// Разбудить вью, вернувшиеся на экран.
///
/// Зовётся С захваченным замком, и это намеренно: `set_hidden` не ходит в CEF
/// синхронно, а ставит задачу в очередь его потока. Отпусти мы замок раньше —
/// «усыпить» и «разбудить» могли бы встать в эту очередь в обратном порядке.
fn wake(views: &mut Views, appeared: Vec<String>) {
    for id in appeared {
        // Будим только то, что действительно спало: при первом показе будить
        // нечего, а лишний `was_resized` заставил бы Chromium перевёрстывать
        // страницу впустую.
        if views.asleep.remove(&id) {
            set_hidden(&id, false);
        }
        notify_view_state(&id, true);
    }
}

/// Сказать браузеру вью, скрыт он или показан.
///
/// Разбуженному мало снять флаг: на статичной странице Chromium сам кадра не
/// шлёт, и панель осталась бы с картинкой, застывшей в момент скрытия. Поэтому
/// показ сопровождаем тем же дуплетом, что и `browsers::nudge` — заодно и
/// размер мог поменяться, пока вью спало.
fn set_hidden(id: &str, hidden: bool) {
    super::input::on_browser(id, move |host| {
        host.was_hidden(if hidden { 1 } else { 0 });
        if !hidden {
            host.was_resized();
            host.invalidate(PaintElementType::from(
                cef::sys::cef_paint_element_type_t::PET_VIEW,
            ));
        }
    });
}

/// Сообщить exthost, что вью скрыто/показано (`kamin:webview:viewState`).
pub(crate) fn notify_view_state(id: &str, visible: bool) {
    let id = id.to_string();
    std::thread::spawn(move || {
        if let Some(c) = crate::host_link::client() {
            let _ = c.request(
                "kamin:webview:viewState",
                vec![
                    serde_json::json!(id),
                    serde_json::json!(visible), // active
                    serde_json::json!(visible),
                ],
            );
        }
    });
}
