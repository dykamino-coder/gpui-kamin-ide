# Кадры: не хватает кадра ОРИГИНАЛА

`ours.png` снят, `original.png` — нет.

Как снять: прод-KaminIDE (`kaminide.exe`) должен быть запущен с CDP
(`WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS`, скрипт
`kamin-ide/scripts/launch-cdp.mjs`), затем Playwright-attach на 9222 и
выбрать тул в пустом слоте до готовности рендерера.

Блокер: сейчас прод-приложение работает БЕЗ CDP и в нём живые сессии
пользователя — перезапуск требует его согласия.

## Снято (ц.15)

`original.png` — кадр `ActivityPlaceholder` из семпла Design-панели
(«Empty / active panel placeholders»): это ТОТ ЖЕ компонент, что рисуется в
пустом слоте, снят с прод-KaminIDE под CDP.
