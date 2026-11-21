# engine-compare

Compares our HTML engine (`engine_bench`) with other engines (Chrome, Edge, Firefox, Servo, Blitz, Kitesurf) on
prebuilt executables. Nothing is built here: the external config names every executable, the page set and the
result store. Pages, configs and results are private and stay outside the repository.

## Commands

```
python -I engine_compare.py --config cfg.json doctor [--aa]          # executables, pages, CPU topology, lane load
python -I engine_compare.py --config cfg.json quick <page> [--cls gpu4] [--engines a,b]
python -I engine_compare.py --config cfg.json run [--pages a,b | --kinds real,syn] [--engines ...] [--classes ...]
                                                  [--force] [--dry] [--ignore-load] [--redo-between FROM,TO]
python -I engine_compare.py --config cfg.json report out.html [--lang ru] [--title ...]
python -I engine_compare.py diff rows-A.json rows-B.json             # before/after of one engine, in the noise or not
```

`run` resumes: finished units are cached by a key of engine build, page content, class and measurement
definition, so only changed units are measured again. `--redo-between` measures again the units started inside a
time window (for example while a foreign process loaded the lanes). `quick` measures one page on every engine with
one repeat.

## Hardware classes and lanes

A lane is two physical P-cores with HT (4 logical CPUs). Classes are masks inside a lane: `gpu4` (4 threads,
GPU raster), `gpu2` (2 cores, one thread each, GPU raster), `soft2` (same mask, software raster), `soft1`
(one thread). Lanes run units in parallel; at most `gpu_parallel` GPU units run at once. With
`"split_soft1": true` the soft1 units run after the other classes on one lane per physical core (twice as many at
once, the HT sibling stays empty). Turn it on only after an A/A check shows that neighbouring one-core lanes do not
move each other's numbers by more than 3 %.

Before every block the tool checks the lanes for 10 s and refuses if a foreign process uses more than 5 % of a core
there (`allow_load` lists programs that are reported, not refused). `--ignore-load` runs anyway and writes the
load line into the store's `run.log`, which the report shows under the measurement bounds.

## Repeats

Ours: frames of one series in one process, at least 3, then until MAD / median <= 3 % or 500 ms of frames. Browsers:
visit 1 loads the page (one cold sample), keeps it for the steady memory window, probes every warm row (every frame of
the probe is a sample of its row) and runs the CPU loops; visit 2 is a fresh load with a frames-only probe of every
warm row; visits 3..8 load again and probe only the rows that have not settled. Cold takes one value per load
(`"cold_clock": "min"`, the default: every load is traced and the better of navigation -> two rAFs after load and the
trace's main + raster sum is taken; `"load"`: the first clock only, which read far higher on the stand). A row's
accuracy is the larger of the 90 % confidence interval half-width of its pooled samples' median (order statistics)
and half the spread of its visit medians; it settles when that is within 3 % of the median (or within the 0.1 ms
clock step), or when that range lies wholly beyond our value of the same row by 3 %: its place is decided, so ours is
measured first in every class. The report shows the accuracy next to a number (`±N%`) and marks a row stopped at the
cap (`!`). Scroll keeps the step count of the browser scenario. The cell title gives the number
of samples.
## Adapters

`ec/adapters/` holds one file per engine kind (`ours`, `browser`, `native`, `oneshot`, `kitesurf`); the contract is in
`ec/adapters/__init__.py`. Browsers run through `driver/browser-driver.cjs` (Playwright for Chromium, Selenium for
Firefox and Servo's WebDriver server), one long-lived browser per lane and class. Files the adapters derive next
to a page (`C.html`, `D.html`, `probe-ec.html`) are not part of the page identity.

`native` drives an engine's own plan harness (protocol `native-plan-v1`, for example Blitz and Kitesurf built with a
plan runner): one process per page and class runs cold, R, the four edits and S on the engine's API with the same
inputs, selectors and scroll step count as the other engines; CPU per frame is the process user + kernel time over each
series divided by its frames. A phase the harness refuses keeps its named reason in the cell.

## Warm-up

The first run of a fresh executable pays the cold disk cache and the GPU driver's shader cache (memory up by a third,
slower frames). Before its first measured unit every engine gets one quick idle run of that unit per tool run; its
numbers are dropped and other lanes wait for it. `--no-warmup` (or `"warmup": false` in the config) skips it.
## Tests

```
python -I -m unittest discover -s tests
```
