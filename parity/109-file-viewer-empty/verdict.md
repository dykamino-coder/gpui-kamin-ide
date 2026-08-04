# 109 — verdict (review cycle 1)
VERDICT: DIVERGES
Глиф slot-рамка vs codicon-file 36 text-disabled; текст другой; нет kbd-чипа Ctrl+P;
лишний заголовок File.

## Цикл 2: DIVERGES
panel_placeholder вместо .empty (codicon-file 36 + kbd Ctrl+P).

## Цикл 5: DIVERGES

Пустое состояние вьюера: у нас слотовый `panel_placeholder` (глиф 39×34 + «File» 16), у оригинала `.empty` = codicon-file 36 text-disabled + текст с `<kbd>Ctrl+P</kbd>`.

## Цикл 6: DIVERGES

Пустое состояние вьюера — слотовый placeholder вместо `.empty` с `<kbd>Ctrl+P</kbd>`.

## Цикл 7: DIVERGES

Пустое состояние: глиф 36 text-disabled, gap 8, pad 20, текст с kbd Ctrl+P → у нас общий panel_placeholder.

## Цикл 12: DIVERGES

Закрыто: своё пустое состояние вместо общего `panel_placeholder` — codicon-file 36
цветом `--text-disabled`, gap 8, padding 20, центрирование, и строка
«Pick a file from the tree, or press [Ctrl+P] to open one by name.» с `kbd`-пилюлей
(2/6, bg-surface, r-xs, моно fs-xs, рамка text-muted 30%).

Осталось: кадр пары.

## Цикл 13: DIVERGES

Закрыто: кегль строки пустого состояния — 16 (дефолт UA для `<p>`; ни `body`,
ни `.viewer/.body/.empty` font-size не задают). Стояло FS_SM 12.

Осталось: пробелы вокруг kbd-чипа сделаны `gap 4` на flex-строке, в оригинале
это обычный пробел внутри абзаца.

### Ц.13 — пересмотр

Элемент недостижим В ОРИГИНАЛЕ: `FilePanel` при `!selectedFile` рисует
`PanelPlaceholder`, до `FileViewer.Empty` управление не доходит
(`FilePanel.tsx:118-127`). Наш порт `.empty` удалён, вместо него — тот же
плейсхолдер (см. 63). Сверять на экране нечего ни с одной стороны.

## Цикл 15: DIVERGES

Осталось: пустое состояние файл-вьювера (codicon-file 36 `text-disabled` + `kbd`-чип) — у нас общий PanelPlaceholder.

## Цикл 20: MATCH

Ревьювер снял претензию: `FilePanel.tsx:118-127` при пустом выборе рисует ТОТ ЖЕ `PanelPlaceholder` с тем же текстом, а `FileViewer.Empty` с `kbd Ctrl+P` — мёртвый код оригинала.
