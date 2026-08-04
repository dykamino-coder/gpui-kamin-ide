# 108 file-viewer-wrapper — оригинал
Файлы: `kamin-ide/src/renderer/components/file-viewer/FileViewer.tsx` (22-79), `FileViewer.module.css`

## JSX-структура (кратко, вложенность)
```
div.viewer [data-drop-zone="editor"]
├─ <FileViewerTabs />                                (таб-стрип, №110)
└─ div.body (или .body.bodyFlush при webview)
   ├─ активный слот: WebviewPanelView | MonacoEditor | Empty
   │    (retained-панель активна → слот пустой (null))
   └─ для каждой retainContextWhenHidden-панели:
      div.retainLayer style="display: flex|none"     (по p.id === активный wvId)
         └─ <WebviewPanelView visible={...} />
```
- `openFiles` лимит 12 (`OPEN_FILES_LIMIT`); переполнение выкидывает старейший un-pinned неактивный.
- Позиции табов стабильны, новые добавляются в конец.
- webview-таб = путь `webview://<id>`.

## Метрики (ИЗ CSS, точные значения)
`.viewer`:
- flex: 1; display: flex; flex-direction: column; min-height: 0
- margin: 0 6px 6px (верх 0, бока 6px, низ 6px)
- background: var(--bg-mantle)
- border-radius: var(--radius-md); overflow: hidden

`.body`:
- flex: 1; min-height: 0; display: flex; flex-direction: column
- background: var(--editor-bg)
- border-radius: var(--radius-md); overflow: hidden
- padding: 8px 0 10px (верх 8px, бока 0, низ 10px — гуттеры редактора)

`.bodyFlush` (webview активен): padding: 0

`.retainLayer`: flex: 1; min-height: 0; flex-direction: column (display управляется inline: `flex`/`none`)

## Состояния (классы-варианты с метриками)
- `.body` → `.body.bodyFlush`: только padding 8px 0 10px → 0 (когда показывается webview-панель).
- `.retainLayer[display:none]`: скрытая retained-панель (iframe остаётся в DOM).
- hover/active/focus/transition: отсутствуют на обвязке.
