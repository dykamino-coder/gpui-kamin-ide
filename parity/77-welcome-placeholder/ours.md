# 77 welcome-placeholder — наша реализация
Файлы: `crates/shell/src/ui/welcome.rs:42-185` (welcome), `welcome.rs:28-38` (feature-чип), `crates/shell/src/root.rs:4013-4043` (вызов + нативный folder-пикер), `root.rs:290-291,454` (запечённый glow-спрайт), `crates/shell/src/ui/radial_bg.rs` (bake_glow)

## Структура (gpui-дерево кратко)
```
div#welcome (size_full, flex-col, items_center, justify_center, gap 16, p 24, overflow_hidden)
├─ div.relative 112×112 (mb 4)
│  ├─ img glow-спрайт 240×240 absolute (left/top −64) — запечённый radial (в gpui radial-градиента нет)
│  └─ img icons/kaminoid.svg 112×112 relative
├─ div «KaminIDE» — 38px, Bold, text_primary #cfd4e2
├─ div версия-пилюля — px 10, py 2, rounded 999, bg accent_primary 14% (#89b4fa @0.14), FS_XS(11), text_primary
├─ div tagline — max_w 480, FS_MD(13), line-height 13×1.4=18.2, text_muted #838aa0
├─ div actions (flex, wrap, gap 12, justify_center, mt 8)
│  ├─ #welcome-folder: fa folder-open(f07c) 13 + «New session in folder…» — px 16, py 8, rounded 8,
│  │    bg accent_primary #89b4fa, text accent_action_fg #313240, FS_SM(12), Semibold; hover opacity 0.9
│  └─ #welcome-empty: fa plus 13 + «Empty session» — px 16, py 8, rounded 8,
│       bg text_primary 6%, border 1px text_primary 14%, FS_SM, Semibold, text_primary; hover opacity 0.85
└─ div features (flex, wrap, gap_x 20, gap_y 8, justify_center, mt 12, max_w 544)
   └─ 3 × feature: fa-иконка 13 accent_primary + label — FS_SM, text_muted
      (comments f086 «Claude chat + tools», folder-tree f802 «Your files & editor», terminal f120 «Integrated terminal»)
```
Действия: folder → нативный `prompt_for_paths` → `kamin:sessions:newSessionInFolder`; empty → `kamin:sessions:newNoFolderSession`.

## Метрики (из кода, точные)
- gap 16 (SPACE_4), p 24 (SPACE_6) ✓ оригинал; лого 112 ✓; версия `v{CARGO_PKG_VERSION}`.
- Заголовок 38px Bold (оригинал 2.4rem = 38.4px, weight 700) — совпадение по факту.
- tagline max_w 480 = 30rem ✓; features max_w 544 = 34rem ✓; gap 20/8 = space-5/space-2 ✓.
- Цвета: text_primary #cfd4e2, text_muted #838aa0, accent_primary #89b4fa, кнопка-текст #313240 (dark).

## Отличия от original.md той же папки
1. Glow: запечённый спрайт 240×240 (bake_glow, alpha 0.5) вместо CSS `::before` 220×220 radial 26% + blur 6 — размер и профиль градиента приблизительные.
2. Hover кнопок: opacity 0.9/0.85 вместо `color-mix 86% black` / `12% заливки` + `translateY(-1px)` — подъёма нет.
3. Нет `drop-shadow(0 6px 18px rgba(0,0,0,.35))` на лого.
4. Нет `letter-spacing:-0.02em` и `line-height:1.05` у заголовка.
5. Primary-кнопка text = accent_action_fg #313240 (оригинал `--accent-on-primary, #fff` — в dark у оригинала белый, у нас тёмный!).
6. tagline line-height 1.4 (у оригинала `--lh-snug`).
7. Поведенчески: welcome заменяет ВСЮ панельную область (root.rs:5366), в оригинале — только main-колонку.
8. `overflow_hidden` вместо `overflow:auto` (низкие окна клипуют, не скроллят).

## Дополнение атрибутов (цикл 10)

- скругления: версия-пилюля radius 999 (`welcome.rs:100`) = `border-radius: var(--radius-pill, 999px)` оригинала, где токен `--radius-pill` в темах НЕ объявлен (grep пуст) и работает именно фолбэк 999px (`WelcomePlaceholder.module.css:61`); обе кнопки radius-sm 8 (`welcome.rs:134,161`, `metrics/lib.rs:37`) = `border-radius: var(--radius-sm)` (`:90`); glow — запечённый спрайт 220×220 вместо `border-radius: 50%` круга (`welcome.rs:73-78` против `:25-34`), т.е. круглая маска не CSS-радиусом; у feature-чипов скруглений нет ни там, ни там.
