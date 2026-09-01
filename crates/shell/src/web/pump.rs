//! Перерисовка по приходу кадра страницы.
//!
//! Очередь CEF мы больше НЕ крутим: он на своём потоке
//! (`multi_threaded_message_loop`, см. `process.rs`). Осталось одно — узнать,
//! что кадр пришёл, и заказать перерисовку.

/// Запустить задачу перерисовки. Зовётся один раз при старте приложения.
pub fn start_pump(cx: &mut gpui::App) {
    // Ждём сигнал по каналу, а не по таймеру: таймерная задача просыпалась
    // редко, и кадры показывались только когда пользователь трогал мышь.
    let (wake_tx, wake_rx) = smol::channel::unbounded::<()>();
    super::set_wake(wake_tx);
    cx.spawn(async move |cx| {
        let Ok(bg) = cx.update(|cx| cx.background_executor().clone()) else {
            return;
        };
        while wake_rx.recv().await.is_ok() {
            // Съедаем всё накопившееся: заказ перерисовки нужен ОДИН на кадр,
            // а вью восемь и каждый шлёт свои кадры.
            while wake_rx.try_recv().is_ok() {}
            super::take_repaint_request();
            // Вытесненные текстуры освобождаем ЗДЕСЬ, между кадрами: внутри
            // отрисовки атлас трогать нельзя.
            if cx.update(super::flush_retired).is_err() {
                return;
            }
            super::diag::refresh();
            // ТОЧЕЧНО, а не `cx.refresh()`. Тот ставит `window.refreshing` и
            // ОТКЛЮЧАЕТ кэш всех вью на кадр (`vendor/gpui/src/view.rs:212`,
            // `window.rs:1386`) — замер показывал «переиграно вью 0» при
            // полностью размеченных компонентах. `notify` помечает грязным
            // только корень, а кэшированные панели переигрываются.
            if let Some(root) = repaint_target()
                && root.update(cx, |_, cx| cx.notify()).is_err()
            {
                return;
            }
            // НЕ ЧАЩЕ ПОЛОВИНЫ КАДРА ДИСПЛЕЯ. Windows отдаёт `WM_PAINT` только когда
            // очередь сообщений окна пуста. Заказывая перерисовку на каждый
            // кадр Chromium (до 300 в секунду на восемь вью), мы очередь не
            // опустошали никогда — и до окна доходило 2-3 `WM_PAINT` из 60
            // тактов vsync (замер patch-счётчиками в vendored gpui). Отсюда и
            // «текстура застыла»: кадры есть, а рисовать их некогда.
            // Отложенные pull'ы страниц — один вызов на вью за тик (см.
            // `web::deliver`): eval на каждую пачку заставлял страницу
            // перевёрстываться сотни раз в секунду.
            super::flush_pending_pulls();
            // Без видеокарты половина кадра дисплея — цена, которую машина не
            // тянет: кадр окна растеризует процессор, и заказы копятся быстрее,
            // чем WARP их отрабатывает (`gpu_mode.rs`).
            bg.timer(super::gpu_mode::repaint_interval()).await;
        }
    })
    .detach();

    // Секундная сводка пути кадра: кадры, отрисовки, заказы (`diag.rs`).
    cx.spawn(async move |cx| {
        let Ok(bg) = cx.update(|cx| cx.background_executor().clone()) else {
            return;
        };
        loop {
            bg.timer(std::time::Duration::from_millis(1000)).await;
            super::diag::report();
            super::reap_hidden();
            super::respawn_stalled();
            crate::probe::registry::paint_watchdog();
        }
    })
    .detach();
}

/// Кому адресовать заказ перерисовки: корневой вью окна.
static TARGET: std::sync::OnceLock<gpui::WeakEntity<crate::state::model::RootView>> =
    std::sync::OnceLock::new();

/// Запомнить корневой вью (зовётся из `main` сразу после создания окна).
pub fn set_repaint_target(root: gpui::WeakEntity<crate::state::model::RootView>) {
    let _ = TARGET.set(root);
}

fn repaint_target() -> Option<gpui::WeakEntity<crate::state::model::RootView>> {
    TARGET.get().cloned()
}
