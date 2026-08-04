//! Страницы вкладов (чат/plan/todos/agents/console): стор HTML и мост.
//! (чат/plan/todos/agents/console). Мост:
//!   вебвью → расширение: window.ipc.postMessage → ipc-handler →
//!     WS kamin:webview:inbound(viewId, msg)  (viewId кодируем в ipc-body)
//!   расширение → вебвью: kamin:webview:post {batch} →
//!     evaluate_script(window.__kaminDeliver([...]))

use crate::host::events::TermEvent;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use serde_json::Value;

pub const CHAT_VIEW_ID: &str = "claudeBridgeChat";

/// HTML вью по viewId — отдаётся custom-протоколом `kamin://`.
/// (file:// и вставка строкой не годятся: предел в 2МБ и разбор адреса
/// на source-URI без authority; kamin → http://kamin.localhost — валидный.)
/// Вместе с HTML храним НОМЕР ВЕРСИИ: по нему отрисовка узнаёт «страница
/// сменилась», не копируя саму страницу. Раньше сравнивали содержимое, и
/// каждый кадр клонировались все страницы моста — мегабайты копий на кадр
/// (замер: кадр интерфейса 43 мс).
static HTML_STORE: OnceLock<Mutex<HashMap<String, (String, u64)>>> = OnceLock::new();

fn html_store() -> &'static Mutex<HashMap<String, (String, u64)>> {
    HTML_STORE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// HTML вью для composition-моста (WebResourceRequested).
/// URL вью в статик-хосте. ОДИН источник формы пути: и `stage_html`, и
/// догоняющая навигация czShared должны давать одинаковый адрес.
pub fn view_url(view_id: &str) -> String {
    format!("http://kamin.localhost/{view_id}")
}

pub fn stored_html(view_id: &str) -> Option<String> {
    html_store()
        .lock()
        .unwrap()
        .get(view_id)
        .map(|(h, _)| h.clone())
}

/// Версия страницы вью: растёт с каждой новой. Дёшево — без копии страницы.
pub fn html_rev(view_id: &str) -> Option<u64> {
    html_store().lock().unwrap().get(view_id).map(|(_, r)| *r)
}

/// Есть ли загруженный HTML у вью (иначе карта показывает плейсхолдер).
pub fn has_html(view_id: &str) -> bool {
    html_store().lock().unwrap().contains_key(view_id)
}

/// probe: выкинуть HTML вью из стора — панель снова уходит в ветку загрузки.
/// Нужно, чтобы снять кадр карточки терминальной ошибки (досье 71).
pub fn drop_html(view_id: &str) {
    html_store().lock().unwrap().remove(view_id);
}

/// Положить HTML вью в стор; вернуть URL для навигации.
pub fn stage_html(view_id: &str, html: String) -> String {
    {
        let mut store = html_store().lock().unwrap();
        // Тот же HTML — НЕ бампать rev: рост rev перезагружает iframe
        // (`ensure_html_view`), а повторную отдачу того же HTML экстеншен
        // делает на каждый повторный resolve. Живое вью с работающим
        // скриптом перегружать из-за идентичной страницы нельзя.
        if let Some((old, _)) = store.get(view_id)
            && *old == html
        {
            return view_url(view_id);
        }
        let rev = store.get(view_id).map(|(_, r)| r + 1).unwrap_or(1);
        store.insert(view_id.to_string(), (html, rev));
    }
    // Путь = РОВНО `/{view_id}`: бандл Customize определяет свой раздел из
    // `location.pathname`, снимая ведущий «/» и префикс `claudeBridgeCz`
    // (`CustomizeContentPanel.tsx:8-11`). С сегментом `/view/` регекс не
    // совпадал, раздел не распознавался и страница рисовала ПУСТОТУ.
    view_url(view_id)
}

/// Разобрать сообщение страницы и передать расширению.
///
/// Страница шлёт POST на `/__ipc`, его принимает `web/scheme.rs`.
pub fn handle_inbound(body: String) {
    // Поток страницы отпускаем сразу: отправка расширению блокирующая.
    std::thread::spawn(move || {
        let Ok(v) = serde_json::from_str::<Value>(&body) else {
            return;
        };
        let view_id = v
            .get("__view")
            .and_then(Value::as_str)
            .unwrap_or(CHAT_VIEW_ID)
            .to_string();
        if v.get("__kaminWebview").and_then(Value::as_bool) != Some(true) {
            // __kaminState: персист getState/setState — след. итерация
            return;
        }
        // Первый inbound = скрипт вью жив → снять load-cover
        if let Some(tx) = crate::host_link::event_tx() {
            let _ = tx.try_send(crate::host_link::ShellEvent::Term(TermEvent::WebviewAlive(
                view_id.clone(),
            )));
        }
        let msg = v.get("msg").cloned().unwrap_or(Value::Null);
        // `chat:bound` — чат отчитался, ЧТО он сейчас реально показывает
        // (боундится только на активный таб). Быстрый локальный сигнал для
        // шторки переключения: полный круг через bridgeShowing-метаданные
        // (вебвью → расширение → host → снапшот) занимает 0.7-1.5 с и держал
        // шторку над УЖЕ правильным контентом; этот путь укладывается в грейс.
        if view_id == CHAT_VIEW_ID
            && msg.get("channel").and_then(Value::as_str) == Some("chat:bound")
        {
            eprintln!(
                "[wv:bound] {} t+{:.2}s",
                msg.get("args")
                    .and_then(|a| a.get(0))
                    .and_then(Value::as_str)
                    .unwrap_or("?"),
                crate::host_link::t0().elapsed().as_secs_f32()
            );
            if let Some(tx) = crate::host_link::event_tx() {
                let _ = tx.try_send(crate::host_link::ShellEvent::ChatBound);
            }
        }
        // Парная строка к `[wv:post]`: по двум видно, что обмен идёт в обе
        // стороны — на это и опирается стенд `scripts/check_chat_ipc.py`.
        eprintln!("[wv:inbound] {view_id}");
        if let Some(client) = crate::host_link::client() {
            // Только send: ответ придёт отдельным `kamin:webview:post`.
            client.send(
                "kamin:webview:inbound",
                vec![serde_json::json!(view_id), msg],
            );
        }
    });
}

/// Шим acquireVsCodeApi. `view_id` вшивается в каждый вью, чтобы мост знал
/// адресата. __kaminDeliver — вход для расширение→вебвью (MessageEvent).
pub fn vscode_api_shim(view_id: &str) -> String {
    // JSON-строка id (кавычки/эскейп) — безопасная вставка в JS
    let v = serde_json::Value::String(view_id.to_string()).to_string();
    let body = concat!(
        "<script>(function(){var s=window.__kaminInitialState;var V=__VIEW__;",
        // Отправка — обычный POST на свой же хост: страницу отдал
        // `web/scheme.rs`, так что запрос свой и отдельный канал не нужен.
        "function P(o){o.__view=V;var s=JSON.stringify(o);",
        "fetch('/__ipc',{method:'POST',body:s})}",
        "var api={postMessage:function(m){P({__kaminWebview:true,msg:m})},",
        "getState:function(){return s},",
        "setState:function(v){s=v;P({__kaminState:true,state:v});return v}};",
        "window.acquireVsCodeApi=function(){return api};",
        // Доставка ДО подписки страницы теряется: ответы на стартовые invoke
        // (skills:list, get-config, tab:list) приходят, пока бандл ещё грузится,
        // и страница ждёт их вечно — Skills висел скелетонами. Копим до
        // DOMContentLoaded, потом отдаём в исходном порядке.
        "var q=[],ready=document.readyState!=='loading';",
        "function D(m){window.dispatchEvent(new MessageEvent('message',{data:m}))}",
        "if(!ready){document.addEventListener('DOMContentLoaded',function(){",
        "ready=true;for(var j=0;j<q.length;j++)D(q[j]);q.length=0})}",
        "window.__kaminDeliver=function(b){for(var i=0;i<b.length;i++){",
        "if(ready){D(b[i])}else{q.push(b[i])}}};",
        // Забор пачки по номеру (путь CEF). Цепочка обещаний держит ПОРЯДОК:
        // запросы уходят параллельно, а разбирать их вразнобой нельзя —
        // сообщения чата перемешаются.
        "window.__kaminPull=function(){window.__kaminChain=(window.__kaminChain||",
        "Promise.resolve()).then(function(){return fetch('/__pull/'+V)",
        ".then(function(r){return r.json()}).then(function(b){if(b.length){window.__kaminDeliver(b)}})",
        ".catch(function(){})})};",
        "})();</script>"
    );
    body.replace("__VIEW__", &v)
}
