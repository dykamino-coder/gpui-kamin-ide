# 123 quick-pick-modal — наша реализация
Файлы: %PROJECTS%\gpui-kamin-ide\crates\shell\src\ui\quick_pick.rs:96-245 (quick_pick), 24-88 (QpItem/QuickPickState)

## Структура (gpui-дерево кратко)
```
div#quick-pick: occlude.absolute.top(84).left((vw−640)/2).w(640)
├─ (title) заголовок
├─ (input) ряд: search-codicon 14 + Input(appearance false), border-b
├─ p(4) → список (max_h 420, скролл)
│  ├─ separator-строка (kind=-1): label fs 11 muted
│  └─ row#qp-{i}: [чекбокс multi] + label + description
└─ (canPickMany) футер: кнопка «OK»
```
Single: клик → `QuickPickResolve(req_id, [i])`; multi: клик = toggle, OK резолвит checked. Esc/скрим → resolve(null) (снаружи). Ответ хосту — deferred respond (HostReply::Later).

## Метрики (из кода, точные)
- Бокс: top 84, w 640, rounded 12 (RADIUS_MD), bg p.bg_mantle #262533, border 1 p.bg_surface a=.8
- Title: px 12 (SPACE_3), pt 8, fs 12 (FS_SM), weight 600, p.text_primary
- Input-ряд: px 12, py 4, border-b p.bg_surface a=.6, search "\u{ea6d}" 14px p.text_muted
- Список: max_h 420, gap 1, обёрнут в p 4
- Row: gap 8, px 12, py 4, rounded 8, fs 12, p.text_secondary; hover bg p.text_primary a=.08 + text_primary
- Чекбокс: codicon check \u{eab2} / circle-large \u{eabc} 13px, on = p.accent_primary #89b4fa
- Description: fs 11, p.text_muted, в строку за label
- OK: px 16, py 4, rounded 8, bg p.accent_action #89b4fa, fg #313240, weight 600, hover opacity .9
- Separator: px 12, pt 4, fs 11, p.text_muted

## Отличия от original.md той же папки
1. `detail` парсится, но НЕ РЕНДЕРИТСЯ (у оригинала span.detail mono справа).
2. Prompt-строка (options.prompt) не рендерится.
3. Инпут «в стиле палитры» (transparent, border-b) вместо обрамлённого поля bg-base с focus border accent; иконка search добавлена (в оригинале её нет).
4. Row: fs 12 text_secondary vs fs-md 13 text-primary; hover text_primary 8% vs accent-primary 18%; padding py 4 vs space-2 (8).
5. Separator: без uppercase, letter-spacing и border-top.
6. Multi-футер: только «OK» без счётчика «OK (N)» и без Cancel-кнопки.
7. Фильтр только по label (matchOnDescription/matchOnDetail, alwaysShow — нет); separators фильтр обходят — совпадает.
8. max-h списка 420 фикс vs palette-max-height контейнера; скрима-элемента здесь нет (затемнение рисует main-окно), ignoreFocusOut считан, но на клик скрима не влияет.
