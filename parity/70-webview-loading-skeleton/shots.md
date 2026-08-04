# Кадры: не хватает кадра ОРИГИНАЛА

`ours.png` снят, `original.png` — нет.

Как снять: прод-KaminIDE (`kaminide.exe`) должен быть запущен с CDP
(`WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS`, скрипт
`kamin-ide/scripts/launch-cdp.mjs`), затем Playwright-attach на 9222 и
открыть contributed-панель и снять первые ~2 с загрузки.

Блокер: сейчас прод-приложение работает БЕЗ CDP и в нём живые сессии
пользователя — перезапуск требует его согласия.
