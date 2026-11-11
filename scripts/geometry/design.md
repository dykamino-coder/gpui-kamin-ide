# Element geometry queries

The public contract is a synchronous query on `Document`. `element_geometry`
returns an owned snapshot of the original elements; `get_client_rects` and
`get_bounding_client_rect` select an element by its original DOM index path.
The caller supplies the same viewport, font options and window used to render
the document. `element_geometry_in_scope` disambiguates scroll state when a
window contains multiple document instances. Paged hosts supply their page
geometry through `element_geometry_with_layout`.

## Specification and reference mechanisms

The normative algorithm is [CSSOM View §6](https://drafts.csswg.org/cssom-view/#extension-to-the-element-interface),
including its empty and degenerate rectangle cases. Transforms apply to the
four corners before taking an axis-aligned bounding rectangle. Overflow clips
and painted effects do not trim a client rectangle. Rectangles use CSS pixels
in the viewport coordinate system. Each inline line box or block continuation
contributes its own rectangle in content order. A table contributes its grid
box and caption boxes, excluding the anonymous wrapper. Boxless elements still
appear in the dump with an empty client list and a zero bounding rectangle.

Reference checkouts were inspected under `Projects/refs`; the paths below are
relative to each reference repository, with line numbers at the stated SHA.
Only the mechanisms were used; reference implementation code was not copied.

| Reference | Source and mechanism |
| --- | --- |
| Blink `a1e311f433fe2936c2fd10eac90145bf2f3559c4` | `third_party/blink/renderer/core/dom/element.cc:3412`: update style/layout on query, obtain client quads, then bound each quad. `element.cc:3361`: adjust the results for scroll and absolute zoom. `element.cc:3458`: the bounding query also synchronously updates layout. |
| Blink, same SHA | `third_party/blink/renderer/core/layout/layout_box.cc:1107`: visit physical fragments, position each continuation relative to the first fragment, and map its local rectangle to an ancestor quad. `layout_object.cc:2040`: `AbsoluteQuads` is the common source for an absolute bounding union. |
| Blink, same SHA | `third_party/blink/renderer/core/dom/geometry_utils.cc:408`: request the selected border, padding or content box from actual fragments, then map the quad to the requested coordinate system. This avoids reconstructing transformed padding/content boxes by subtracting edges from an already transformed bounding rectangle. |
| Servo `8b3eb30f1b76eabef6a0803eb50bc1f763a6e49c` | `components/script/dom/element/element.rs:3280`: client rectangles come from the node's border-box fragments; `element.rs:3300`: the bounding query consumes the layout border-box result. `components/script/dom/window/window.rs:3034`: a synchronous query forces reflow before asking layout. |
| Servo, same SHA | `components/layout/layout_impl.rs:427`: dispatch the element query to the fragment/layout layer. `components/layout/query.rs:96` and `query.rs:131`: gather the selected area of the element's fragments and map through the cumulative root transform. `query.rs:65`: derive that transform from the scroll tree. |

The current Servo snapshot calls the formerly content-box-specific mechanism
`process_box_area_request` / `process_box_areas_request`; it accepts a
`BoxAreaType`. There is no `process_content_box_request` symbol in this shallow
snapshot. Its current implementation, rather than an assumed historical
function body, is the reference described here.

## Query lifecycle and normal rendering cost

The document already retains its admitted source DOM for normal document
operations. A query borrows that source through `DocumentValues::query_input`,
creates a separate budget revision and `DocState`, then builds a temporary
layout tree. The query build preserves original `html`/`body`, hidden-element
computed styles and inline ownership that normal rendering can elide.
Computed `font_size` and `line_height` come from computed style, independently
of `font-size-adjust` or the size of painted glyphs. `normal` remains `normal`.

The ordinary parser/build entry points select the `QUERY = false` const
specialization. Query adapters use `QUERY = true` and replace the render
builder only inside the explicit query. The collector and projective transform
map live in scoped render contexts created by that call. Normal elements and
normal frames receive no geometry map, retained fragment list or geometry
cache. The API deliberately trades work during a query for no retained
geometry and no geometry collection in unused frames.

`Window::with_layout_query` isolates the temporary layout/prepaint pass from
the live frame: layout inputs, frame state, callbacks and paint state are
restored on return and unwind. The query reads current scroll offsets and
copies them into temporary scroll handles; it does not update live extents or
offsets. Prepaint supplies layout positions, but the query does not paint or
publish a frame. Deferred positioned boxes are completed before collection
finishes. Source DOM and the live document revision are not mutated.

Inline observers report the line fragments produced by actual text layout.
Column/page observers use the fragment placement records to slice along the
block axis and map a continuation into its displayed position. This is
fragmentation, not overflow clipping. Local border, padding and content boxes
are projected separately. The query transform path preserves projective
corners, singular transforms and flattening boundaries without requiring the
invertible mapping used by hit testing. SVG queries use source-node identity
and geometry bounds, excluding stroke, markers and clipping effects.

This construction explains where collection is disabled; it is not a measured
performance result. Acceptance still requires the shared P70.1 `SCREEN=1`
`build_pair2.sh` run against `engine_bench-base-d77295e3-screen-a.exe`, with
per-phase counters and exact RGB on the final candidate commit. The screen
manifest and raw evidence belong in private shared storage, not this document.

## Harness and comparison

`wptrun --geometry` is used because that harness already owns browser CSS,
fonts, viewport and window initialization. It can exercise the public document
query with device scale factor 1 without adding diagnostic work to the timing
harness. JSON is emitted in original DOM preorder. Paths start at `Document`
and contain `childNodes` indices, so comments, text and the doctype count.
Generated or anonymous layout boxes never receive synthetic DOM paths.

`chromium.cjs` loads the same input in headless Linux Chromium through
Playwright, using an explicit private `PLAYWRIGHT_BROWSERS_PATH` and device
scale factor 1. It waits for fonts and images. Border geometry comes directly
from CSSOM View; padding/content fragment quads come from GeometryUtils.
Chromium without that API fails instead of estimating transformed edges.
The Windows corpus orchestrator launches both engine and container processes
through the supplied shared hidden-desktop launcher. Matching host fonts must
be mounted into the container to separate layout differences from font
substitution.

`compare.py` matches only identical DOM paths. It rejects duplicate paths and
different viewports. Numerical tolerance is
`absolute + relative * max(abs(engine), abs(chromium))`; fragment counts and
identity metadata remain exact. Reports contain signed coordinate deltas,
counts of affected elements per property, missing elements, page totals and
the largest absolute deltas. `corpus.py` defaults to widths 360, 768 and 1280
and produces a sanitized private `first-report.json` without IDs or raw
metadata differences. Geometry tolerance never changes the exact-RGB gate.

## Verification boundaries

Real layout tests cover multiline/empty inline boxes, multicol continuations,
paged continuations, table/caption boxes, SVG, projective transforms and
current scroll offsets. Isolation tests check repeat queries, restoration
after unwind and live scroll-handle preservation. Comparator tests exercise
exact path matching, fragment cardinality, tolerance accounting and invalid
numeric inputs. These are distinct from the Chromium corpus comparison and
the ordinary-frame performance/RGB gates; passing one does not imply the
others passed.
