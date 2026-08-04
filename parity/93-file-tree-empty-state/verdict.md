

## Цикл 4: DIVERGES

Пустое состояние: нет второй подсказки «Pick a session in Projects, or start
one with a folder.», `.emptyIcon` должен быть `--text-disabled` (у нас
наследуется text-muted), нет `padding: var(--space-5)`, и в empty-состоянии
оригинал ВСЁ РАВНО рисует `FileTreeHeader` — у нас ранний return без него.

## Цикл 8: DIVERGES

Пустое состояние: нет заголовка дерева, второй подсказки, `padding: space-5`, иконка наследует muted вместо text-disabled.

## Цикл 10: DIVERGES

Все четыре пункта живы: в пустой ветке не рисуется хедер; нет второй подсказки «Pick a session in Projects…»; нет padding 20; цвет глифа text_muted #838aa0 вместо text-disabled #60667b.

## Цикл 11: MATCH

Все четыре претензии закрыты: `FileTreeHeader` рисуется и в пустом состоянии
(титул «PROJECT», кнопки disabled) — как `FileTreeView.tsx:40-53`; добавлена вторая
подсказка «Pick a session in Projects, or start one with a folder.»; блок получил
`padding: space-5` 20 и `text-align: center`; глиф 32 покрашен в `--text-disabled`
#60667b вместо text-muted. Остальные метрики (`flex 1`, центровка, gap 8, fs-sm у
подсказок) сверены и совпадают.

## Цикл 12: MATCH

Ревью подтвердило 4/4: хедер в пустой ветке, титул «PROJECT», три кнопки disabled,
вторая подсказка, `padding: space-5` + `text-align: center`, глиф 32 в
`--text-disabled` #60667b. Открытых претензий нет.

## Цикл 13 (ревью зоны): DIVERGES

Закрыто: глиф пустого состояния 32 → **16**. Это единственное место зоны, где
кегль был ЗАВЫШЕН: `.emptyIcon { font-size: 32px }` стоит на самом
`<i class="codicon codicon-folder emptyIcon">`, то есть (0,1,0) против базы
(0,2,0). Каскад кодиконов: класс, задающий `font-size` НА ТОМ ЖЕ элементе, что и `.codicon`, имеет специфичность (0,1,0) и проигрывает вендорному `.codicon[class*=codicon-]` (0,2,0) — значит в оригинале глиф 16.

## Цикл 14: MATCH

Ревью подтвердило элемент целиком, включая спорный глиф: `.emptyIcon
{font-size:32px}` стоит на самом `<i class="codicon codicon-folder">` и
проигрывает вендорной базе — фактические 16, как мы и поставили в цикле 13.

## Цикл 15: MATCH

Пустое дерево: центр, gap 8, p 20, глиф 16 (32px на самом `.codicon` проигрывает базе), две строки fs-sm, хедер рисуется.

## Цикл 18: MATCH

Пустое дерево: `.empty` flex 1 + центр + gap 8 + p 20, глиф эффективно 16 в `--text-disabled`, обе подсказки fs-sm, хедер рисуется.

## Цикл 21: MATCH

Пустое дерево: хедер «PROJECT» + 3 disabled-кнопки, глиф 16 в text-disabled, обе подсказки fs-sm — снято живьём.
