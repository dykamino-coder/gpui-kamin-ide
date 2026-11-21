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

Every row gets at least 3 values (ours: frames of one series in one process; browsers: page visits), then one
more while it is not stable, up to a cap (ours: 500 ms of frames; browsers: 8 visits). Stable = MAD / median
<= 3 % (median absolute deviation from the median). Scroll keeps the step count of the browser scenario. The
report shows the number of samples in each cell's title.

## Adapters

`ec/adapters/` holds one file per engine kind (`ours`, `browser`, `oneshot`, `kitesurf`); the contract is in
`ec/adapters/__init__.py`. Browsers run through `driver/browser-driver.cjs` (Playwright for Chromium, Selenium for
Firefox and Servo's WebDriver server), one long-lived browser per lane and class. Files the adapters derive next
to a page (`C.html`, `D.html`, `probe-ec.html`) are not part of the page identity.

## Tests

```
python -I -m unittest discover -s tests
```
