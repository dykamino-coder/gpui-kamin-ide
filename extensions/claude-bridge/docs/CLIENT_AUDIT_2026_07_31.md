# Аудит клиента Claude Bridge — 2026-07-31 (4 исследователя)

Запрос: «мёртвый код / плохие алгоритмы / корнер-кейсы / плавность — архитектура,
скорость, только нужный код». Область: webview/src + extension/src.
Статус: [ ] = не применено. Применять по одному с проверкой.

## A. Мёртвый код (высокая уверенность)
- [x] A1 (снят удалением мёртвого Titlebar.tsx в A7): `webview/src/components/titlebar/Titlebar.tsx:47-55,85-109`
      — каналы window:minimize/maximize/close/devtools НЕ обработаны в extension
      (bridge-host.ts:367 emit в пустоту). Удалить блок .controls.
- [x] A2 Chrome-mic-bridge (вырезан из живых файлов: useVoiceInput PWA-ветка, InputBar micConnected, WhisperSettings Test-Mic переписан на getUserMedia+AnalyserNode; Titlebar MicStatus уйдёт с A7) (Electron-эпоха) целиком: Titlebar.tsx:14-45,83 (MicStatus),
      WhisperSettings.tsx:34-91,139 (Test-Mic UI), useVoiceInput.ts:173-190 (PWA-ветка),
      InputBar.tsx:52-54 — mic-* каналы никем не слушаются, micConnected всегда falsy.
      Оставить только native getUserMedia путь.
- [x] A3 getDroppedFilePath (ветки ОЖИВЛЕНЫ: байты → save-dropped-file → temp-путь с исходным именем; стаб удалён из shim/types) = "" (bridge-shim.ts:33) → мёртвые ветки не-image drop/paste:
      useDragDrop.ts:79-86, useClipboardPaste.ts:47-56. Удалить/пометить явно.
- [x] A4 toast-window.ts:60-61 closeToast/closeToastsForTab — ноль вызовов. Удалить.
- [x] A5 Дубль createSession-логики (в живом коде один NewSessionButton; дубль в TitlebarQuickActions уйдёт с удалением мёртвого дерева A7): NewSessionButton.tsx:13-34 ≡ TitlebarQuickActions.tsx:10-33
      → общий hook useCreateSession(); + удалить `declare const electronBridge` (NewSessionButton.tsx:4).
- [x] A6 signals/jsonl.ts:718-743 activeQueue/getQueueFromJsonl — потребителей нет. Удалить.
- [x] A7 УДАЛЕНО (18 файлов: App/AppLayout/Titlebar+5/TabsBar/HeaderViewToggles/Sidebar+8; tsc чист, 151 тест, шареные css-модули живы): → AppLayout/Titlebar/TabsBar/Sidebar/SessionItem НЕ импортируется
      ни одним entry (chat/tools/customize). Кандидат на удаление (подтвердить у юзера).

## B. Алгоритмы/скорость (ранжировано)
- [x] B1 compact-segments.ts:133-162 — полный пересчёт ВСЕХ сегментов на каждый batch append
      (structureVersion бампается каждым аппендом). Десятки-сотни мс на батч у компактированных.
      Фикс: per-segment мемо по boundary ts; на append пересчитывать только последний сегмент.
- [x] B2 order-entries.ts:63-68 — Date.parse всего стора на каждый батч. Фикс: кэш `_tsMs`
      на записи при ингесте (append/prepend/replaceWindow), записи иммутабельны.
- [x] B3 jsonl.ts:468 appendJsonlEntries findIndex с фронта (цель у хвоста) — миллионы
      сравнений на реплей. Фикс: обратный скан с хвостовым окном (см. STUB_SCAN_TAIL) или Map<msgId,idx>.
- [x] B4 jsonl.ts:227-233 scheduleStreamFlush — клон ВСЕГО стора каждый rAF ради 1-2 стабов.
      Фикс: slice() + патч хвостовых индексов.
- [x] B5 jsonl.ts:253 applyStreamingDelta findIndex с фронта 30р/с (+ :163). Фикс: обратный скан.
- [~] B6 (СДЕЛАНО: NOISE_TAGS на модуль, extractTag Map-кэш; Date→B2) jsonl-viewer/utils.ts: NOISE_TAGS литерал на каждый вызов (:487) → на модуль;
      extractTag new RegExp per call (:29) → Map-кэш; Date в merge (:417) → _tsMs из B2.
- [x] B7 AssistantEntry.tsx:73-79 — классификация ошибок регэкспами на каждый кадр живого
      хвоста. Фикс: скип при `__streaming !== undefined`.
- [x] B8 transcript-mirror.ts:189→266-274 — sidecar writeFile+rename на каждый батч. Дебаунс ~1с.
- [x] B9 handle-server-message.ts:77-87 — `cached.push(...kept)` spread ~20k аргументов —
      близко к лимиту стека V8. Заменить на присваивание/цикл.

## C. Корнер-кейсы/гонки (HIGH → LOW)
- [x] C1 HIGH connection-manager.ts:746-756/833-836 — mirror НЕ перенацеливается при смене
      conversationId (resumeNotFound/ghost-breaker → fresh session с новым id): транскрипт B
      пишется в файл A.jsonl, у B нет зеркала, будущий resume A отдаст чужой head/pos.
      Фикс: в setConversationId при id!==старому — close+null mirror/mirrorHead перед ensureMirror.
- [x] C2 HIGH tab-manager.ts:121-127/388-394 — closeTab во время triggerSync → connect() для
      удалённого таба → orphan CLI-сессия на сервере до 30-мин рипера. Фикс: guard tabs.has(tabId).
- [x] C3 HIGH connection-manager.ts:806-821 — таймер requestJsonlDownload не отменяется:
      просроченный таймаут загрузки A убивает загрузку B. Фикс: хранить/чистить handle или токен.
- [x] C4 HIGH useBridgeListeners.ts:659-664 + ipc/sessions.ts:201-226 — bridge:reconnected
      реплеит ВСЕ табы во все панели, per-connection дебаунс → шторм при рестарте сервера
      с warm pool. Фикс: tabId в bridge:reconnected + глобальный дебаунс нонса.
- [x] C5 MED connection-manager.ts:555/278 — reconnectAttempt=0 в disconnect() → вечный
      1s-цикл при «session:exit при живом сокете». Фикс: не сбрасывать в internal disconnect.
- [x] C6 MED sessions-bridge.ts:235-246 — деактивация: disconnectTab оставляет zombie-tab
      (кэш 20k записей в памяти) + реактивация плодит второй таб. Фикс: closeTab.
- [x] C7 MED useBridgeListeners.ts:73/703 — hookDrivenTabs/stuckIdleTimers пересоздаются на
      реконнекте → OSC-эвристика снова рулит Stop. Фикс: useRef/модульный уровень.
- [x] C8 MED useInit.ts:139-140 — init пишет activeTabId мимо seq-guard → затирает switch,
      прилетевший во время init. Фикс: applyTabSwitch(id, 0).
- [x] C9 MED transcript-mirror.ts:56-60 — sidecar-валидация только size==0; краш mid-write →
      постоянная дыра истории. Фикс: писать/сверять размер файла в sidecar.
- [x] C10 MED transcript-mirror.ts:81-95 — запись крупнее окна чтения → ложное «начало файла»,
      scroll-up навсегда останавливается. Фикс: удвоение окна 2-3 итерации при пустом результате.
- [x] C11 (resolve-deny в disconnect + фильтр activeWidgets в onTabClosed) MED connection-manager.ts:553-554 — pendingPermissions.clear() без resolve →
      вечно висящий handleMcpCall; activeWidgets закрытого таба не чистятся (onTabClosed).
      Фикс: resolve('deny') перед clear; фильтр виджетов по tabId.
- [x] C12 (синтетический replayComplete для prefill без кэша статуса, гейт на неаутентифицированное соединение) — prefill пустого warm-таба оставляет replay-mode
      без replayComplete. C13 ОТЛОЖЕН осознанно: гонка live/compacted — территория THE WIPE LOOP (0.2.51), править только с полным контекстом протокола компакции (release_kaminide_0_2_x).
      C14 [x] idleTracker.dispose() в disconnect(endSession).
      C15 [x] respondMcp/denyMcp адресуются владельцу requestId (hasPendingMcp). C16 [x] session:error без close (авто-сброс в connected через 5с при живом сокете) → 'error'
      навсегда. C17 [x] settle-окно idle-трекера держится до replayComplete (кап 60с + грация 1.5с). C18 мелочи (stale tabToKamin,
      requestJsonlReplay(tabId) [x] тип выровнен, static lastTree на все соединения).

## D. Плавность рендера (ранжировано)
- [x] D1 ChatRoot.tsx:59,110 — per-frame сигналы (activeThinkingPreview, tabActivity) в КОРНЕ →
      ререндер всего дерева каждый кадр стрима и на каждом OSC-тике любой вкладки.
      Фикс: вынести <ActivityStrip/>; корень подписан только на activeTabId/tabs/initializing.
- [x] D2 SessionStats.tsx:42-66 — O(N) computeContextStats на каждый кадр. Фикс: мемо по
      (tabId, structureVersion, segIdx) — как segCache.
- [x] D3 AssistantEntry.tsx:97-101 — каждый бабл подписан на широкий tabs; безусловные записи
      tabs.value (useBridgeListeners 175/193/288/514) ререндерят все ~150 баблов.
      Фикс: computed activeSessionColor + писать tabs.value только при фактическом изменении.
- [x] D4 ToolCounterToast.tsx:45-53 — скан хода каждый кадр → мемо по structureVersion.
- [x] D5 JsonlViewer.tsx:356 — подписка на tabs ради __coverDiag → tabs.peek().
- [x] D6 JsonlTextBlock (span per-delta чанк: дифф prev-текста, ключ по старту, окно 400 как было).tsx:70-100 — до 400 per-char span-vnode на кадр → span per-delta чанк.
- [x] D7 tabActivity гранулярность: разделить tabWorking (boolean) и tabActivityTitle.
- [x] D8 JsonlViewer.tsx:541/511 — index-фолбэк в key → ремаунт при prepend. Ключ по _ord.

## Проверено и НЕ трогать
Прошлые фиксы на месте: vnode-cache+LRU, windowed store, rAF-коалессация, derived-cache,
segCache, tool-result-cache, mirror `_pos>=cursor`, чанкование 150/IPC, passive scroll,
MutationObserver-автоскролл (единственный рабочий детектор), guard `this.ws!==sock`,
trimJsonlCache по _ord, seq+localIntent в applyTabSwitch.
