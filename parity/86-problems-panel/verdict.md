# 86 — verdict (review cycle 1)
VERDICT: DIVERGES (крупно)
Warning=accent-yellow подтверждён. Нет хедера Problems + .counts/.countBtn
(countActive accent18%+border40%); нет collapse по файлу (chevron 13/w16); нет
TreeIcon 16 и .fileDir; fileRow не h24/gap6/hover surface60%, имя SEMIBOLD vs
normal; счётчик не пилюля min-w16 h16 r9; нет .showMore и капов 100/200;
empty-state другой; лишний p8 у списка.

## Цикл 5: MATCH

Problems-панель 1:1 (хедер 8/8/8/12, countBtn 1/6 r9 + border-резерв, active accent 18%/40%, `.list` pb8 + скролл, fileRow h24 gap6 px8 + chevron 13 в боксе 16 + TreeIcon 16 + dirname ellipsis + пилюля min-w16 h16 px5 r9, капы 100/200 + «Show N more files (M hidden)»). Остаток: у хедера не применён `ss01` (хелпер есть); мы пересортировываем файлы по uri и диагностики по (severity, line), оригинал сохраняет порядок хоста.

## Цикл 6: MATCH

Problems: `ss01` применён; пересортировка по uri/(severity,line) — остаток.

## Цикл 13: DIVERGES

Закрыто: глиф «Show more» 12 → **16** (у `.showMore .codicon` правила нет).

Осталось: шеврон файла 13 → 16 по тому же каскаду (правлю следующим заходом);
порядок файлов/диагностик — у нас сортировка, у оригинала порядок хоста.

## Цикл 14: DIVERGES

Закрыто: шеврон файла 13 → **16** (вердикт ц.13 сам записал это в «осталось»
и не сделал) и глиф счётчика 12 → **16** — правило `.countBtn .codicon`
написано БЕЗ `:global`, в CSS-модуле класс хешируется и с настоящим
`.codicon` не совпадает, значит действует вендорная база. Плюс ховер работает
и у активной пилюли: `.countBtn:hover:not(:disabled)` (0,3,0) перебивает
`.countActive` (0,1,0).

Осталось: порядок файлов и диагностик — у нас сортировка, у оригинала порядок
хоста.

## Цикл 16: MATCH

Problems: хедер 8/8/8/12 + `ss01`, пилюли-счётчики с ховером даже у активной, глиф 16, `.fileRow` h 24, `.fileCount` r 9, «Show N more».

## Цикл 19: MATCH

Problems: хедер, пилюли-счётчики с ховером у активной, глифы 16 по каскаду, `.fileRow` h 24, «Show N more».
