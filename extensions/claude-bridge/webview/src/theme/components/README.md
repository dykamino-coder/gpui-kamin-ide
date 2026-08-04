# theme/components/ — per-family CSS modules (E3)

Migration target for `theme/legacy-global.css` — split the 2530-LOC monolith
into focused per-family files for sidebar / chat / jsonl / widgets so each
component's styles live near (or inside) the component itself.

**Status: scaffolding only.** The legacy file still owns all the rules at
runtime — these files are placeholders for the incremental migration.
Each migration step:

1. Move 5–20 related rules from `legacy-global.css` to the matching
   `components/<family>.css` file.
2. Delete those rules from `legacy-global.css`.
3. Verify visually with the dev server.
4. Commit per family. Don't migrate the whole file at once — visual
   regressions would be impossible to bisect.

When each `components/<family>.css` is loaded the same way `legacy-global.css`
is (via `main.tsx`), order matters — keep the import after `variables.css`
and `global.css` so cascade resolution stays correct.

## Family inventory (rule counts as of E3 scaffolding)

| Family | Rules in legacy-global.css | Target file |
|--------|----------------------------|-------------|
| sidebar | 61 | `sidebar.css` |
| chat / input / message | 24 | `chat.css` |
| jsonl viewer (entries + tools + blocks) | 100 | `jsonl.css` |
| widgets / modals / toasts / dropdowns | 26 | `widgets.css` |
| (everything else: titlebar, agents, plugins, customize tabs) | ~270 | not yet split |

## Theme tokens

All component CSS files MUST consume colours / spacing / radii / fonts via
the `var(--*)` tokens from `variables.css`. New colour values introduced in
component CSS are bugs — they break light-theme parity (see `light-theme.css`).
