# 73 — verdict (review cycle 1)
VERDICT: DIVERGES
Архитектурная замена (dyn-tool вместо стека ViewSection); нет ActivityPlaceholder
«No views»; welcome-фоллбек шире оригинала (ИСПРАВЛЕНО в wave1: welcome теперь
только в main-карте — перепроверить в цикле 2).

## Цикл 5: DIVERGES

Архитектурно: один contributed view = один dyn-тул; стека `views.map → ViewSection` внутри `.root` нет, нет `ActivityPlaceholder "No views"`.

## Цикл 6: DIVERGES

Contributed-контейнер: стека вью нет (архитектурно).

## Цикл 13 (баг от пользователя): DIVERGES

Закрыто: панель Bridge Todos вечно висела в скелете с растущим «attempt N».
Визуальные хосты вебвью создавались ТОЛЬКО для трёх статических id
(`KNOWN_WEBVIEWS`: chat/plan/console), а вью contributed-тулов (Todos,
Agents и прочие вклады) хоста не получали вовсе — значит, не подавали
признак жизни, и resolve уходил в пустоту. Теперь хост создаётся и для вью
АКТИВНЫХ contributed-тулов (создавать под каждый зарегистрированный вклад
дорого).

Проверено по логу: `claudeBridgeTodosView` пошёл на связь (10 сообщений
против нуля).

Осталось: стек нескольких вью в одном контейнере; `ActivityPlaceholder`
«No views» у пустого контейнера.

## Цикл 16: DIVERGES

Модель «контейнер → N вью» отсутствует: contributed-вью = один тул, стека `ViewSection` в одной карте нет; нет веток «No views» / «No view».

## Цикл 19: DIVERGES

Модель «контейнер → N вью» отсутствует; веток «No views» / «No view» нет.

## Цикл 22: MATCH

★ Модель «контейнер → N вью» есть. `DynTool` держал ПЕРВОЕ вью контейнера —
теперь `DynTool.views: Vec<DynView>`, а тело панели рисует их стопкой
(`.view { flex: 1; min-height: 0 }`), у каждого свой хедер `.title` с
`title/description/badge` из `createTreeView`. Пустой контейнер →
`ActivityPlaceholder icon="circle-large" label="No views"`; неизвестная
contributed-страница Customize → «No view» (`ContributedViewBody`).

Проверено живьём на фикстуре `helloPanel` (два вью: `helloView` вебвью +
`helloTree` дерево) — оба видны одновременно, делят высоту.

Попутно найдено и починено два бага, из-за которых стопка казалась пустой:
1. вебвью-вью ждал сообщения ОТ страницы (`__kaminWebview`), чтобы снять
   скелет; обычное contributed-вью нашу шину не дёргает и висело до «This
   panel didn't load». Оригинал снимает скелет по приходу html
   (`ContributedContainerBody.tsx:87`) — сравняли;
2. дерево вечно висело на «Loading…»: состояние вью создавала бродкастная
   мета (`kamin:tree:meta`), а «первый показ» определялся по наличию
   состояния — запрос детей не уходил. Теперь явный флаг `root_requested`.
