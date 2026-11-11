# Спецификация: браузерный bridge-клиент для Студии

> **Статус (перенесено из kamin-bridge-webclient PR #1).** Реализовано как
> `@kaminide/bridge-browser-client` в этой папке; описание API — в
> [README.md](README.md). Отличия от исходного предложения:
>
> - фабрика называется `createBridgeClient()`, интерфейс — `BridgeClient`;
> - превью — отдельный вход `@kaminide/bridge-browser-client/preview`:
>   `createPreviewRenderer()` (React 19 и runtime Module Federation передаёт
>   встраивающее приложение) и `createPreviewView()`, который сам следует за
>   состоянием хоста и отдаёт готовый `HTMLElement` со статусом;
> - состояние превью приходит событием `workspace:preview-state` по `/bridge`
>   (`client.preview.onState`), опрос `GET /preview/state` не нужен;
>   `preview.fetchState()` оставлен для старых хостов;
> - пакет публикуется ассетом GitHub Release
>   `kaminide-bridge-browser-client-<version>.tgz`; в корпоративный nexus его
>   загружает потребитель из этого ассета;
> - браузерный UI KaminIDE сам использует этот транспорт, сокет и рендерер превью;
> - сверх задания есть типизированные методы для разрешений, AskUserQuestion,
>   elicitation, модели, effort, режима разрешений, вложений, субагентов,
>   статусной строки и мониторов плагинов. Их проверили живым прогоном через
>   пример встраивания. Правила чата для встраивающего UI описаны в README.
>
> Ниже — исходный текст задания без изменений.

> Этот документ — задание. Его автор строит **Студию** (CMS-приложение ATON) и ему нужен из
> этого репозитория отдельный **framework-agnostic браузерный клиент** к воркспейсу. Ниже —
> контекст, точный контракт API и что откуда вынести. Реализацию допиливает агент на стороне
> kaminide, зная задачу.

## Контекст

Студия — приложение для разработки веб-приложений непрограммистами: у пользователя в браузере
живой чат с агентом и hot-reload превью. Под каждого пользователя поднимается dev-контейнер,
в котором крутится **движок этого репозитория** (`workspace` + `extension`/`BridgeHost`),
подключённый к Bridge-серверу. Движок переиспользуется как есть.

Морда Студии пишется **заново** на React 19 + дизайн-системе ЦМС (`@cms-mf/design-system`) и
живёт как микрофронт в шелле ЦМС. Морда движка (`webview`, Preact) в Студии не используется.
Поэтому Студии нужен не UI, а **транспортный клиент**: как браузеру говорить с воркспейсом по
тем же кадрам, что и `webview`, но без Preact и без визуальных кусков.

## Что нужно вынести

Сейчас транспорт и клиент превью вшиты в `webview`. Нужен отдельный пакет (имя/scope — ниже),
собранный из уже существующей логики, без визуального слоя:

| Источник в `webview/src` | Что берём |
|---|---|
| `lib/bridge-transport.ts` | `inv` / `snd` / `sub` — запрос-ответ, fire-and-forget, подписка на события. |
| `browser/browser-socket.ts` | Открытие `/bridge` WebSocket и прокачка кадров в транспорт. |
| `browser/browser-entry.ts` (часть) | Сборка URL `/bridge`, старт соединения, `workspace:sync`. |
| `browser/preview-panel.ts`, `preview-renderer.ts`, `preview-hot.ts` | Клиент превью: `GET /preview/state`, монтирование MF-remote, hot-reload WS `/preview/remotes/<name>/ws`, `POST /preview/unmount`. |
| `lib/bridge-channels.ts` | Карты каналов INVOKE/SEND/EVENT — основа типизированного фасада. |

Визуальное (Preact-компоненты, codemirror, xterm, CSS) — **не** брать.

## Контракт API

Клиент должен отдавать framework-agnostic объект (события + методы), на который сверху
навешиваются React-хуки Студии. TypeScript:

```ts
export type TabId = string
export type Dispose = () => void

export interface StudioBridgeClient {
  // Соединение
  connect(): Promise<void>            // открывает /bridge WS (same-origin, cookie-сессия уже есть)
  disconnect(): void

  // Сессии (вкладки движка)
  sync(): Promise<{ tabs: Tab[]; active: TabId | null }>   // send 'workspace:sync' → снапшот
  createTab(opts?: CreateTabOptions): Promise<TabId>       // invoke 'tab:create'
  resumeSession(conversationId: string): Promise<TabId>    // invoke 'session:resume'
  switchTab(id: TabId): void                               // send 'tab:switch'
  activeTabId(): TabId | null

  // Чат
  submitPrompt(tab: TabId, text: string): void             // send 'pty-submit-text'
  interrupt(tab: TabId): void                              // send 'session:interrupt'

  // Поток транскрипта (события через sub)
  onJsonlEntries(cb: (tab: TabId, entries: JsonlEntry[]) => void): Dispose   // 'jsonl-entries'
  onStreaming(cb: (tab: TabId, entry: JsonlEntry) => void): Dispose          // 'streaming-entry'
  onStatus(cb: (tab: TabId, status: JsonlStatus) => void): Dispose           // 'jsonl-status'
  onTabsChanged(cb: (tabs: Tab[], active: TabId | null) => void): Dispose    // 'tab:list-changed' / 'tab:switched'

  // Превью
  previewState(): Promise<PreviewDescriptor | null>        // GET /preview/state
  mountPreview(d: PreviewDescriptor): PreviewHandle        // MF-remote + hot-reload WS
  onPreviewRemount(cb: () => void): Dispose
  unmountPreview(): Promise<void>                          // POST /preview/unmount
}

export interface PreviewHandle {
  readonly element: HTMLElement        // контейнер смонтированного MF-remote
  readonly status: 'mounting' | 'ready' | 'reloading' | 'error'
  onStatus(cb: (s: PreviewHandle['status']) => void): Dispose
  unmount(): void
}
```

Типы `Tab`, `CreateTabOptions`, `JsonlEntry`, `JsonlStatus`, `PreviewDescriptor` — из движка;
**реэкспортнуть их из пакета**, чтобы морда не лезла в приватные пути.

Требования:
- Без зависимости от DOM-фреймворка; превью отдаёт готовый `HTMLElement`, монтирование делает морда.
- `connect()` рассчитывает, что cookie-сессия воркспейса уже выставлена (вход делает сайт Студии).
- Отключение/реконнект: `inv` при обрыве реджектится (как сейчас в `bridge-transport.ts`),
  события после реконнекта восстанавливаются снапшотом — поведение не менять.
- Единица времени и форматы кадров (`invoke`/`send`/`event`, `invoke-diagnostic`) — как сейчас.

## Публикация

Морда Студии собирается rspack в репозитории CMS и зависит через корпоративный **nexus**
(как `@cms-mf/*`). Публиковать клиент в nexus; исходник остаётся в GitHub. Scope пакета —
на усмотрение: `@kaminide/bridge-browser-client` либо `@cms-mf/bridge-client` (если удобнее
резолвить одним реестром с остальными CMS-пакетами).

## Как это потребляет Студия

```ts
import { createStudioBridgeClient } from '@kaminide/bridge-browser-client'
const client = createStudioBridgeClient()      // same-origin, /bridge
await client.connect()
const { active } = await client.sync()
const tab = active ?? await client.createTab()
client.onStreaming((t, e) => { /* рендер в DS-чат */ })
client.submitPrompt(tab, 'сделай лендинг с формой')
```

До публикации морда строится против этого интерфейса (контракт), с dev-заглушкой только для
локальной отрисовки. После публикации импорт подменяется на пакет, заглушка удаляется.
