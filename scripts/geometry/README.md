# Element geometry diagnostics

These tools compare layout measurements alongside the exact-RGB gate. Geometry
tolerances never affect screenshot verdicts.

Build the existing `wptrun` example with the `wpt-harness` feature, using the
shared build script on the Windows benchmark host. Run its diagnostic mode on
the shared hidden desktop:

```text
python hidden-desktop-launch.py wptrun.exe --geometry page.html engine.json 768 600
```

The command loads the page with `BROWSER_CSS`, the harness fonts, and device
scale factor 1. It performs layout without capturing or comparing a screenshot.
`wptrun` already owns the window and font setup needed by the document query;
the timing harness remains dedicated to measuring rendering phases.

Run the Chromium side inside Linux/Podman, headless. Install Playwright and its
matching browsers in private storage, then mount that storage into the container:

```text
node chromium.cjs --input page.html --output chromium.json --width 768 --height 600 \
  --browsers-path /private/browsers --playwright-module /private/playwright/node_modules/playwright-core
python compare.py engine.json chromium.json --output comparison.json --absolute 0.5 --relative 0
```

Chromium must expose `GeometryUtils.getBoxQuads`, enabled by the script. Padding
and content boxes come from that API's actual fragments. A browser lacking it
fails explicitly. Every browser context uses device scale factor 1.

Schema version 1 stores viewport dimensions and a preorder list of elements.
Each `path` contains **childNodes** indices starting at `Document`; text nodes,
comments and the doctype count. Matching uses that exact path, including elements
without boxes. `border_box` is `getBoundingClientRect`; `client_rects` contains
border boxes in fragment order. `padding_boxes` and `content_boxes` retain the
same fragmentation. Rectangles use CSS pixels relative to the viewport and do
not apply overflow clipping. Computed `line_height` is either a number of CSS
pixels or the string `"normal"`.

The comparator reports signed engine-minus-Chromium deltas outside the tolerance
`absolute + relative * max(abs(engine), abs(chromium))`. Fragment counts and DOM
identity are exact comparisons. Property counts count each element once per
property; the report also includes missing elements, page totals and the worst
elements ranked by absolute delta.

For a corpus run, supply `corpus.py` with a private JSON manifest containing
`[{"name":"page-name","input":"relative/path.html"}]`. Its `--input-root` is
the common source directory. Pass `--engine`, `--private`, `--podman` and
`--hidden-launcher` explicitly. The private directory must contain `browsers/`
and `playwright/node_modules/playwright-core`. The launcher runs each engine and
Podman process on the shared hidden desktop. Default widths are 360, 768 and
1280 CSS pixels; height defaults to 600. Individual snapshots, logs, comparisons
and `first-report.json` stay in that directory. The summary contains manifest
page names rather than host filesystem paths. Use public or sanitized inputs
when sharing a report; element IDs and classes are page content.

The library exposes synchronous `Document::element_geometry`,
`Document::get_client_rects` and `Document::get_bounding_client_rect` queries.
They use a temporary layout pass with the supplied `RenderOpts`, `Window` and
`App`; the caller can consume the owned snapshot after the query returns.
