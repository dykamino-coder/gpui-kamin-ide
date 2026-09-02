# Второй заход: WARP + что анимируется

## Тест 1. WARP — подтверждён

Модули главного процесса (pid 38392), фильтр `warp|d3d|dxgi|nvwgf|igd|amdx|vulkan|opengl|dcomp`:

| ModuleName | FileName | MB |
|---|---|---|
| dxgi.dll | C:\Windows\SYSTEM32\dxgi.dll | 1 |
| d3d11.dll | C:\Windows\SYSTEM32\d3d11.dll | 2,4 |
| dcomp.dll | C:\Windows\SYSTEM32\dcomp.dll | 1,9 |
| **d3d10warp.dll** | C:\Windows\SYSTEM32\d3d10warp.dll | **7** |

Никаких `nvwgf*`, `igd*`, `amdx*` — аппаратного драйвера нет вообще.
Гипотеза подтверждена: gpui рисует через WARP, программный растеризатор D3D.

## Тест 2. Что анимируется

`wvjs` вернул `{"ok":true}` на все 6 id, но в `chrome_debug.log` отписались только два вью —
`claudeBridgeChat` и `claudeBridgeConsoleView`. Остальные четыре, судя по всему, не созданы
(что сходится с двумя живыми renderer-процессами).

```
[anim] claudeBridgeChat        vis=visible hidden=false n=1 :: _spinner_1yjoq_35/_spin_1yjoq_35
[anim] claudeBridgeConsoleView vis=visible hidden=false n=0 ::
```

Детали единственной анимации:
```
[anim2] claudeBridgeChat url=http://kamin.localhost/claudeBridgeChat vis=visible n=1 ::
        DIV._spinner_1yjoq_35 | st=running | rect=19x19 | disp=block | vis=visible | op=1 | off=false
```

Один CSS-спиннер 19×19 px, keyframes `_spin_1yjoq_35`, playState=running, реально в лейауте.

## Прямая причинно-следственная связь: спиннер ↔ 6,8 ядра

Замеры в двух состояниях, одна и та же машина, ничего не меняли:

| состояние | анимаций в чате | кадров/с (diag) | кадров окна/с | CPU main |
|---|---|---|---|---|
| спиннер крутится | 1 | 120–128 | 22–27 | **6,8 ядра** |
| спиннер снят | 0 | **0** | 0 | **0,24–0,28 ядра** |

Чистый 30-секундный замер простоя после того, как спиннер исчез: `8,53 CPU-сек / 30 с = 0,28 ядра`,
и все 30 строк `[cef]` в этом окне — `кадров 0, кадров окна 0`.

То есть один спиннер 19×19 пикселей стоит примерно **6,5 ядра**.

## rAF идёт 60/с всегда, но сам по себе бесплатен

Замер частоты `requestAnimationFrame` в обоих вью:

```
во время жора: [raf]  claudeBridgeChat        rafPerSec=60 mutPerSec=4 vis=visible anims=1
               [raf]  claudeBridgeConsoleView rafPerSec=60 mutPerSec=0 vis=visible anims=0
на простое:    [raf2] claudeBridgeChat        rafPerSec=60 vis=visible anims=0
               [raf2] claudeBridgeConsoleView rafPerSec=60 vis=visible anims=0
```

Важное уточнение к гипотезе «хост безусловно качает BeginFrame»: качает-то он безусловно —
rAF тикает 60/с в обоих вью даже когда `кадров 0` и CPU 0,24 ядра. Но пустой BeginFrame
OnPaint не вызывает, и стоит он около нуля. Платить начинаем только когда в кадре
что-то реально меняется: тогда каждый OnPaint → заказ перерисовки всего окна → WARP.

`claudeBridgeConsoleView` при нуле анимаций и нуле DOM-мутаций даёт 0 кадров — то есть
второго источника 60 fps нет, все 120+ кадров/с давал один вью в моменты активности.

## visibilityState

У обоих вью `visibilityState=visible`, `document.hidden=false` — в том числе у консольного,
которого на экране не видно. Незакрытый `was_hidden` подтверждается со стороны страницы.

## Файлы

- `modules.txt` — список D3D/DXGI-модулей главного процесса
- `anim.txt` — ответы probe + все строки `[anim]`, `[anim2]`, `[raf]`, `[raf2]`
- `cache-diag.log` — обновлён, 10856 строк (включает окно жора и последующий простой)
- `diag-tail.txt` — последние 800 строк
- `cache-cef-chrome_debug.log` — обновлён, содержит вывод console.log из вью
