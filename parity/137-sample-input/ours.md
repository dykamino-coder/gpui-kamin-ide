# 137 sample-input — наша реализация
Файлы: %PROJECTS%\gpui-kamin-ide\crates\shell\src\ui\design_panel.rs:256-271 (`sample_input`), 810 (`block("Input", …)`)

## Структура/содержание
```
block «INPUT»
└─ div.w_full.max_w(360).px(12).py(8).rounded(8)
     .bg(bg_base).border_1(bg_surface).text_size(13).text_color(text_muted)
   └─ «Sample input»
```
Это СТАТИЧНЫЙ div-макет инпута, а не редактируемое поле: ни `InputState`, ни обработчиков ввода нет. Живые текстовые инпуты в приложении есть отдельно (gpui-component `InputState` — quick_open, find_in_files, command_palette, prompt-модалка).

## Метрики (из кода, точные)
- отступы: px 12 (SPACE_3) / py 8 (SPACE_2)
- гэпы: N/A: гэпы — у бокса ровно один текстовый ребёнок, `gap` не задан
- цвета: bg p.bg_base #313240; border 1px p.bg_surface #3d3f51; текст-заглушка «Sample input» p.text_muted #838aa0
- скругления: rounded 8 (RADIUS_SM)
- шрифты: font-size 13 (FS_MD), weight 400, семейство UI «Bricolage Grotesque» (наследуется, своего `font_family` нет)
- ховер: N/A: ховер — статичный div, ни `.hover(...)`, ни фокуса, ни курсора текста

## Отличия от original.md той же папки
1. Геометрия и цвета совпадают 1:1: width 100% + max-width 360, padding 8×12, border 1px `bg-surface`, radius-sm 8, background `bg-base`, font-size fs-md 13.
2. Это не `<input type=text>`, а статичный div: контролируемого значения, каретки, ввода и `useState`/`onInput` нет.
3. Как следствие — состояние `:focus` (border-color `--accent-primary`) и `transition: border-color 150ms` отсутствуют.
4. Строка «Sample input» — в оригинале это `placeholder` (цвет — UA-дефолт), у нас обычный текст цветом `--text-muted`; `color: var(--text-primary)` для введённого текста у нас неприменимо.
5. `outline: none` и `font: inherit` неактуальны (в gpui нет outline; семейство наследуется).
