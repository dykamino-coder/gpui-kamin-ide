# Browser-depth native frames (INC-2026-0084)

The owner requires the same 1026-level reflow contract at opt-level 0 and 2,
on the existing 8 MiB stack. Tests, depth budgets, profiles and stack settings
must stay unchanged. This investigation starts at exact main `d9dcc677`.

## Measurement plan

The existing isolated probes generate block, inline, float, grid, math, table
and multicol documents. Small-depth runs avoid crashing while establishing
the recurring per-level cost. Build runs inside heap-scheduled futures;
native stack growth must be distinguished from suspended future count.
Compiler x64 unwind information identifies the functions owning allocations.

The preferred measurement now uses local DbgHelp/PDB symbols and Windows debug
events on our own test process. Entry breakpoints sample the original x64 RSP
before the function prologue; no measurement function executes in the child.
Each original instruction is restored, single-stepped, then the breakpoint
is reinserted. N=16/32 stack-span differences remove fixed entry/setup cost.
This covers Taffy's recursive layout dispatch as well as the shared Hooked and
direct Paragraph element methods. The debugger affects elapsed time, so these
runs cannot supply performance results. No symbol server is used.
The breakpoint mechanism passed a smoke check on a private Python child.
The PE UNWIND_INFO decoder was compared against llvm-readobj on an available
Rust DLL: all 2954 symbol-matched frame sizes agreed, including return addresses
and nonvolatile-register pushes. This validates the decoder, not engine frames.

Temporary Rust address probes were prepared as a fallback, saved outside the
checkout and restored to exact source bytes before baseline compilation.
They have not supplied measurements and are not present in the checkout.

## Browser mechanisms inspected

Local reference revisions:

- Blink `a1e311f433fe2936c2fd10eac90145bf2f3559c4`.
- Gecko `0f76fed7aea1f77db85afbf5a895754d8ad971d7`.
- Servo `8b3eb30f1b76eabef6a0803eb50bc1f763a6e49c`.

Blink's HTML construction site uses `kMaximumHTMLParserDOMTreeDepth=512`
(`html_construction_site.h:109`) and attaches excess-depth parsed nodes beside the
parent when the parser stack exceeds `kMaximumHTMLParserDOMTreeDepth`.
This bounds parser-produced nesting, not every tree created through DOM APIs.
`PrePaintTreeWalk::Walk` copies a traversal context and calls `WalkChildren`;
the inspected paint walk remains recursive. `LayoutObject`'s 10-bit cached
depth saturates at 1023 and falls back to an ancestor walk; that saturation is
not a layout-depth rejection policy.

Gecko uses `MAX_REFLOW_DEPTH=1026` on 64-bit desktop, 585 on the selected
smaller-stack platforms. Container reflow checks the budget plus four root
frames and returns zero geometry beyond it. Frame construction admits twice
the reflow depth. This is a mechanism comparison only; no MPL implementation
is copied. Its depth budget assumes native frames fit the supported stack;
the number alone does not establish that property for Rust at opt-level 0.

Servo's inspected layout and display-list paint traversal also contain native
recursive calls. `PaintTraversal` owns outline/float vectors once for the walk
and passes borrowed traversal state into fragment recursion. No numeric
layout/paint recursion budget was found in those inspected modules; this is
not a claim about every Servo subsystem. A broader search across the available
`components` tree found the explicit 8 MiB script-thread stack at
`components/script/event_loop/script_thread.rs:490`, rather than a layout/paint
recursion counter. Rust crate `recursion_limit` attributes concern compilation,
not the runtime DOM depth.

Relevant source paths: Blink `core/html/parser/html_construction_site.cc`,
`core/paint/pre_paint_tree_walk.cc`, `core/layout/layout_object.h`; Gecko
`layout/generic/nsIFrame.h`, `nsContainerFrame.cpp`,
`layout/base/nsCSSFrameConstructor.cpp`; Servo
`components/layout/display_list/paint_traversal.rs`, `dom_traversal.rs`,
`traversal.rs`.

A broader text search over Blink `third_party/blink/renderer/core/{layout,paint}`
and Servo `components/layout` for recursion depth/guards and stack limits found
no additional numeric guard. The Blink cached depth hits above are not a guard.
The local Blink checkout is sparse (`core` exposes css/layout/paint/style/svg);
a follow-up search in css/style found only the CSS math-expression limit, not
a document layout/paint limit. The comparison therefore distinguishes the observed mechanisms from any claim
that these complete engines have no other stack protection.

## Product shell

`crates/shell/Cargo.toml` embeds CEF and does not depend on `kamin-html`.
The shell's normal dev profile is opt-level 2. The existing agent profile
provides opt-level 0 for workspace code without a new profile override.
`cargo tree -p kaminide-gpui --edges normal,build --prefix none` confirms that
neither kamin-html nor its font-facts crate is in the shell dependency graph.
`web/browsers.rs` sends URLs to CEF; Rust does not build/layout/paint the document.
Consequently the same kamin-html opt-level-dependent overflow is not reachable
from the product shell. A normal debug executable also uses opt-level 2, not 0;
opt-level 0 for shell workspace code would require the existing agent profile.
No shell runtime result is claimed. Its startup creates a centered visible
window and may forward to the owner's running instance; the no-screen/no-focus
constraint excludes using that startup as an unattended document experiment.
CEF's independent depth behavior is not established by the engine tests.

## Results and acceptance

Checkpoint: engine fix and acceptance are incomplete. At tool-call 360/400 the
driver required finishing the current investigation cluster without new pilots.
No engine source, profile, test, depth limit or shared script was changed.

## Original-main agent measurements

Unchanged `d9dcc67728d92ef1c75e521438f17c0181f2db0e`, Rust 1.96.0
x86_64-pc-windows-msvc, existing agent profile: HTML opt-level 0, dependencies
opt-level 2, PE stack reserve 8,388,608 bytes. No source instrumentation was
compiled. Retained binary: `baseline-agent/kamin_html-a618fc1719772087.exe`.

The exact regression fails with stack overflow. Inline/grid/math/table finish
paint; multicol finishes layout and fails before prepaint completion. Existing
ignored probes at depth 1026 produced 33 successful phase endpoints out of 35:
all build/request/layout endpoints pass; all prepaint/paint endpoints pass
except multicol (`0xc00000fd`). The paint endpoint includes prepaint, so that
failure does not independently establish an overflow inside paint. This is not
a complete HTML suite result. The 2052-level block build was not rerun.

Entry-RSP span growth, bytes per additional source-document nesting level:

| Kind | Request | Layout | Prepaint | Paint |
| --- | ---: | ---: | ---: | ---: |
| block | 3008 | 4000 | 2640 | 2512 |
| inline | 0 | 0 | 0 | 0 |
| float | 7360 | 7216 | 6816 | 7680 |
| grid | 3008 | 6224 | 2640 | 2512 |
| math | 0 | 0 | 0 | 0 |
| table | 3008 | 4236 | 2640 | 2512 |
| multicol | 8256 | 8256 | 8256 | 7728 |

Calculation: `(span(N=32) - span(N=16)) / 16`; span is max minus min entry RSP
among matching functions during a paint invocation. This is an invocation
envelope, not independent phase cost. Multicol lazily builds/layouts descendants
inside recursive prepaint; request/layout inherit the enclosing chain. Inline
and math flatten source nesting: zero slope does not mean zero-byte frames.
Table adds a table every fourth source level. Initial heap-scheduled build has
zero measured growth for block/float/grid/table; inline/math have no recursive
build sample. Multicol's 5984-byte build envelope includes lazy construction
during prepaint and cannot be attributed to the initial build alone.

The actual repeating multicol prepaint chain has 30 frames totalling 8256
bytes. At depth 1026 its recursive part projects to 8,470,656 bytes, exceeding
the reserve before fixed setup. The 28-frame repeating paint chain totals
7728 bytes; full-depth paint safety remains unproven because prepaint fails.

| Owner on the recurring chain | Agent frame bytes |
| --- | ---: |
| `ColumnStack::prepaint_frags` | 1520 |
| `ColumnStack::prepaint_columns` | 368 |
| GPUI `Interactivity::prepaint_with_state` | 832 |
| GPUI `Div::prepaint` | 688 |
| `ColumnStack::paint_frags` | 2080 |
| `Hooks for ColumnStack::paint` | 928 |

Sizes include return addresses, allocation and nonvolatile-register pushes.
Root PDB public symbols omit private functions; the raw chain labels those by
RVA. The 209,808 original COFF unwind records identify the missing owners and
agree with linked PE records for the 1520/2080-byte frames. Larger nonrecurring
frames, such as vertical-axis prepaint, do not establish depth growth and are
not on this horizontal fixture's chain.

Native frame accumulation on the recursive element/fragment path is confirmed.
The particular locals still need allocation attribution before selecting a
general fix. Fragment dispatch keeps geometry, borrowed children, optional
replay routes and closures alive across descendant prepaint/paint. Splitting
preparation from dispatch or reducing replay values are candidate directions
only: neither is implemented or benchmarked here.

## Evidence and continuation

External root:
`C:/Users/MSI/kaminide-private-diagnostics/split-2026-10-06/agents/deepframes`.
Generated synthetic documents and local debugger/compiler data only:

- `before-agent-native.json`: seven kinds, N=16/32 entry-RSP samples.
- `before-agent-chain.json`: multicol chains, including unnamed RVAs.
- `before-agent-matrix.json` and matching logs: 35 phase endpoints.
- `before-agent-object-frames.json`: 209,808 original COFF records;
  `selected-agent-object-frames.json`: narrowed multicol owner list.
- `before-agent-all-frames.json`: linked PE frames including private RVAs.
- `native-probe.py`, `chain-probe.py`, `symbols.py`, `all-frames.py`,
  `unwind.py`, `phase-matrix.py`: retained measurement scripts.
- `base/main.exe`: exact-main wptrun from unchanged `build_shared.sh`;
  eight private synthetic self-pairs passed, 16 PNGs retained. This is not
  the required before/after exact-RGB acceptance gate.

Existing dev-profile and release engine-bench baseline builds started before
the driver constraint. Event-driven continuations retain the binaries and run
dev measurements after success. Inspect their final results before reporting
opt-level 2 numbers. No after-fix build exists.

Remaining: actual fix, both-profile frame comparisons, release before/after
paint timings, complete agent/dev suites, fmt, clippy and zero-loss exact-RGB
gate. The shell finding above is source analysis, not an opt-level 0 runtime
test. No READY or Fix PR claim is justified. Documentation checks are recorded
in progress.md.
