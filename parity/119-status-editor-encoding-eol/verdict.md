# 119 — verdict (review cycle 1)
VERDICT: DIVERGES
Тултип File encoding vs Encoding; гейт по eol (UTF-8 пропадает) vs по selectedFile.

## Цикл 2: MATCH

## Цикл 5: MATCH

Encoding/EOL: тултипы «Encoding»/«End of line», метрики 1:1. (EOL статичен с момента открытия — поведенческое, не визуальное.)

## Цикл 6: MATCH

Encoding/EOL 1:1.

## Цикл 15: DIVERGES

Закрыто: `UTF-8` рисуется при любом открытом файле, EOL — отдельно.

## Цикл 17: MATCH

Проверено живьём: при открытом файле справа `UTF-8` и `LF` с тултипами; кодировка завязана на наличие файла, EOL — отдельно.

## Цикл 20: MATCH

`UTF-8` по открытому файлу, `LF` по известному EOL, тултипы — подтверждено кадром.
