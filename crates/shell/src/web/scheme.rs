//! Отдача нашего HTML браузеру: `http://kamin.localhost/<id>` → стор страниц.
//!
//! Чат Bridge и вью расширений грузятся не из сети, а из памяти приложения.
//! У WebView2 это делал перехват запросов; у CEF роль ту же играет фабрика
//! обработчиков схемы, зарегистрированная на хост `kamin.localhost`.
//!
//! Через неё же идёт обратный канал «страница → приложение»: страница шлёт
//! POST на `/__ipc` своему же хосту, и отдельный мост не нужен.

use cef::rc::*;
use cef::*;

/// Прозрачная точка 1×1 — ответ на запрос значка вкладки.
const TRANSPARENT_DOT: &[u8] = &[
    0x47, 0x49, 0x46, 0x38, 0x39, 0x61, 0x01, 0x00, 0x01, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00,
    0xFF, 0xFF, 0xFF, 0x21, 0xF9, 0x04, 0x01, 0x00, 0x00, 0x00, 0x00, 0x2C, 0x00, 0x00, 0x00, 0x00,
    0x01, 0x00, 0x01, 0x00, 0x00, 0x02, 0x02, 0x44, 0x01, 0x00, 0x3B,
];

/// Зарегистрировать обработчик. Зовётся один раз после инициализации CEF.
pub(crate) fn register() {
    let factory = KaminScheme::new();
    let ok = register_scheme_handler_factory(
        Some(&"http".into()),
        Some(&"kamin.localhost".into()),
        Some(&mut factory.clone()),
    );
    if ok != 1 {
        println!("[cef] обработчик kamin.localhost не зарегистрирован");
    }
}

/// Ответ страницей из стора. Тема-блок вставляется ЗДЕСЬ, при каждой отдаче:
/// стор хранит страницу без темы, поэтому перезагрузка вью всегда получает
/// АКТИВНУЮ палитру (смена темы в рантайме — `webview_theme`).
///
/// Тем же швом заходит и глушение бесконечных анимаций без видеокарты
/// (`gpu_mode.rs`): через этот обработчик идут ВСЕ наши страницы — и чат, и
/// вью расширений, — поэтому одна вставка покрывает их разом, а перезагрузка
/// вью не может её потерять.
fn html_reply(view_id: &str) -> Option<ResourceHandler> {
    let html = crate::ui::chat_webview::stored_html(view_id).unwrap_or_else(|| {
        println!("[cef] HTML вью {view_id} ещё не готов");
        String::new()
    });
    let theme = crate::ui::webview_theme::theme_block(
        kamin_theme::current_palette(),
        kamin_theme::current_is_light(),
    );
    let motion = super::gpu_mode::reduced_motion_block();
    reply("text/html", format!("{theme}{motion}{html}").into_bytes())
}

/// Ответ произвольным телом.
///
/// Тело ВСЕГДА непустое: на пустом потоке ответ не завершался, запросы
/// копились и упирались в предел одновременных соединений — страница после
/// десятка сообщений замолкала совсем.
fn reply(mime: &str, body: Vec<u8>) -> Option<ResourceHandler> {
    let mut handler = cef::wrapper::byte_read_handler::ByteReadHandler::new(std::sync::Arc::new(
        std::sync::Mutex::new(cef::wrapper::byte_read_handler::ByteStream::new(body)),
    ));
    let stream = stream_reader_create_for_handler(Some(&mut handler))?;
    Some(
        cef::wrapper::stream_resource_handler::StreamResourceHandler::new_with_stream(
            mime.to_string(),
            stream,
        ),
    )
}

/// Собрать тело POST-запроса.
fn post_body(request: &Request) -> Option<String> {
    let data = request.post_data()?;
    let count = data.element_count();
    let mut elements: Vec<Option<PostDataElement>> = vec![None; count];
    data.elements(Some(&mut elements));
    let mut out = Vec::new();
    for element in elements.into_iter().flatten() {
        let size = element.bytes_count();
        if size == 0 {
            continue;
        }
        let mut buf = vec![0u8; size];
        let got = element.bytes(size, buf.as_mut_ptr());
        buf.truncate(got);
        out.extend_from_slice(&buf);
    }
    String::from_utf8(out).ok()
}

wrap_scheme_handler_factory! {
    struct KaminScheme;
    impl SchemeHandlerFactory {
        fn create(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            _scheme_name: Option<&CefStringUtf16>,
            request: Option<&mut Request>,
        ) -> Option<ResourceHandler> {
            let request = request?;
            let url = CefStringUtf16::from(&request.url()).to_string();
            // Путь без хоста; id вью — единственный сегмент (`/{view_id}`),
            // как в `stage_html`.
            let path = url.split_once("kamin.localhost/").map(|(_, p)| p).unwrap_or_default();
            let path = path.split('?').next().unwrap_or_default();
            // Всё накопленное для вью: `/__pull/{вью}`.
            if let Some(view_id) = path.strip_prefix("__pull/") {
                let body = super::outbox::take_all(view_id);
                return reply("application/json", body.into_bytes());
            }
            match path {
                // Сообщение страницы расширению.
                "__ipc" => {
                    if let Some(body) = post_body(request) {
                        crate::ui::chat_webview::handle_inbound(body);
                    }
                    reply("text/plain", b"ok".to_vec())
                }
                // Своего значка у наших страниц нет — отдаём прозрачную точку,
                // иначе браузер каждый раз пишет в лог «страницы нет».
                "favicon.ico" => reply("image/gif", TRANSPARENT_DOT.to_vec()),
                view_id => html_reply(view_id),
            }
        }
    }
}
