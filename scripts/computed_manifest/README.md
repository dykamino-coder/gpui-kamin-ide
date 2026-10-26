# ADR3 computed access inventory

`python scripts/check_computed.py --base origin/main` checks non-growing ceilings
in `access-*.json`. CI runs it beside `check_transitional`, on every PR including
documentation-only changes. Rows aggregate by Computed/InheritedStyle field,
top-level HTML module (owner), and read/write/mutable/initialize/escape kind. They
contain no source paths or line numbers. `identity.json` records the independent
style, box and fragment identities.

Only non-style field writes/mutable projections and mutable escape calls are
ratcheted. Style construction writes and reads are informational baseline counts,
not exact snapshots. Reductions and file/line moves within a phase pass without
regeneration. `--write` explicitly lowers ceilings; checked-in ceilings cannot
increase relative to the merge base. First installation checks base sources, so
it cannot bless new writes. A new field/owner/access starts with zero debt.
Actual source counters are also compared with the merge base: unused ceiling
headroom cannot reintroduce writes removed by an earlier PR.

One lead-authorized F2 P01–P04 rebaseline is enumerated in
`scripts/computed_f2_rebaseline.py`: exactly 19 pre-census fragment-copy counters,
with explicit old/new ceilings. It applies only against census commit
`f05aec7b98ecd67c407fc47dac5d5a65fb744f66`, and expires when the merge base
advances. All other counters and any growth beyond those exact ceilings remain
blocked. F2 P21/P22 removes the copies; ADR3 A08/A14 owns remaining mutable
projection/escape cleanup. This is not a general ceiling-increase mechanism.

This is a conservative lexical check, not Rust type inference or proof of deep
immutability. Same-named fields and non-style generic mutable APIs are deliberately
over-counted. Comments and literals are ignored. Explicit `#[cfg(test)]`/`#[test]`
items and default-path module descendants have informational `:test` owners;
test fixtures cannot spend production debt. Custom `#[path]` modules and all
other cfg branches count conservatively as production.
Direct/indexed/nested assignments, compound assignments, mutable projections,
explicit Computed/InheritedStyle initializers/patterns (including shorthand),
legacy used adapters, generic mutable calls outside style, and `&mut Computed`
signatures outside style are checked. Aliases, arbitrary mutating methods, macros
and interior mutability still require review. Construction inside style is not
used resolution; the publication API enforces the new materialized handle's
ownership boundary. No new escape API may be introduced to bypass the inventory.

Generate the full field/write/read/import/clone census only when auditing:

```powershell
python scripts/computed_manifest.py --snapshot C:\private\adr3a\snapshot-new
```

The output directory must be new and outside the repository. It contains the
complete conservative, line-numbered JSONL census and hashed index. Keep it in
the owner's private diagnostics directory, never as a checked-in CI fixture.
