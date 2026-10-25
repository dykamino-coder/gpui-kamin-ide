"""ADR3 on-demand, private field/access snapshot (not a CI baseline).

This is a conservative *source* census, not type inference or runtime frequency.
Every token matching a field is retained (including macros/initializers and
same-named fields of other types). Indirect writes through whole-value mutable
borrows must also be reviewed using the accompanying API census. Never interpret
the absence of a direct assignment as proof that a property is computed-only.
"""

import argparse
import hashlib
import json
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[1]
SRC = ROOT / "crates/html/src"
OUT = ROOT / "scripts/computed_manifest"
FIELDS = SRC / "style/computed/fields.rs"
FIELD = re.compile(r"^    pub(?:\([^)]*\))? (\w+):\s*(.*)")
STRUCT = re.compile(r"^pub(?:\([^)]*\))? struct (Computed|InheritedStyle)\b")
# Owners describe the eventual contract, not a claim that today's fields are pure.
OWNERS = {
    "cascade": "explicit_inherit decl_order side_order inherit_longhands inherit_bits declaration_parse content_inherit logical width_from_inline css_wide_value border_inherit shadow_inherit text_overflow_inherit content_inherit bidi_inherit clear_inherit font_weight_step font_size_step",
    "html-input": "attr_width attr_height attr_sized",
    "box-context": "parent_grid parent_grid_align_items parent_flex_grid parent_lanes parent_subgrid cross_stretched decor_box icb cq_box cb_wm ortho_limit orthogonal_scrollport orthogonal_inline ortho_col wm_contained root_box cb_height_def_hint quirk_pct_base_hint icb_scroll_insets viewport_unit_insets transform_ancestor transformed_ancestor perspective_frame frame_3d flex_main_def flex_item flex_main_vertical plain_block_box inline_display abs_inline_level fragment_clip_ancestor in_multicol backdrop_root_above will_change_root backdrop_filter_set block_tag is_caption row_group_kind col_role inline_control mathml math_layout math_font_size ruby_unit ruby_suppress_breaks hoisted_block implicit_anchor abs_static hug_inline hug_claim baseline_orthogonal align_content_block lanes_taffy offset_icb floated_box intrinsic_wrapper_required",
    "inline-context": "flat_mode para_vertical turned_line lines_reversed upright_stack",
    "used-geometry": "rel_shift box_shift float_tail band_kid flex_line_wraps flow_shapes own_margin float_margin_offset spacer_edge vertical_shift_px vertical_align_base frame_sizing_height transform_origin_px perspective_origin_px bg_border_insets clip_round mask_box_override mask_user_scale shape_image fit_arg stretch_size calc_size_basis calc_size_max",
    "paint-context": "canvas_bg text_shadow_ink scrollbar_filter_ops decors flat_text_background flat_border_background inline_bg inline_border inline_pad inline_radius text_clip_fill text_clip_raw embedding_dark_scheme first_letter_excluded first_letter_own first_line_own",
    "computed-and-used-font": "font_adjust_base font_size line_height",
}
API = re.compile(
    r"\b(?:Computed|InheritedStyle|SharedStyle|BoxStyle|ElementStyle|ComputedSlot)\b"
    r"|\b(?:make_mut|unwrap_or_clone|used_mut|used_rc|layout_made|into_computed|lay_used)\b"
    r"|\b(?:clone|cloned|clone_from|clone_from_slice|to_owned|to_vec|get_mut|borrow_mut|deref_mut|take|replace|swap)\b"
    r"|&\s*mut\b|\bTRANSITIONAL\("
)


def relative(path):
    return path.relative_to(ROOT).as_posix()


def fields():
    owner = None
    result = []
    for line, text in enumerate(FIELDS.read_text(encoding="utf-8").splitlines(), 1):
        start = STRUCT.match(text)
        if start:
            owner = start[1]
        if owner and (match := FIELD.match(text)):
            result.append((owner, match[1], line, match[2]))
        if text == "}":
            owner = None
    assert {f[0] for f in result} == {"Computed", "InheritedStyle"}
    return result


def destination(name, sites):
    for owner, names in OWNERS.items():
        if name in names.split():
            return owner
    # Any out-of-style site is retained as a possible dual-meaning property.
    # CSS operands remain in Computed even when layout resolves the same field.
    if any("/style/" not in site[0] for site in sites):
        return "computed-operand; audit box/used projections"
    return "style/cascade; audit indirect mutation"


def census():
    definitions = fields()
    names = {f[1] for f in definitions}
    tokens = re.compile(r"\b(?:" + "|".join(sorted(names, key=len, reverse=True)) + r")\b")
    sites = {name: [] for name in names}
    apis = []
    for path in sorted(SRC.rglob("*.rs")):
        for line, text in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
            stripped = text.strip()
            if not stripped or stripped.startswith("//"):
                continue
            for name in sorted(set(tokens.findall(text))):
                tail = text[text.index(name) + len(name):]
                kind = "read-or-indirect-write"
                if re.match(r"\s*(?:[+*/|&^-]?=(?!=))", tail):
                    kind = "direct-write"
                elif re.match(r"\s*:", tail):
                    kind = "initializer-or-pattern-or-definition"
                elif re.search(r"&mut\s+[^;]*\b" + name + r"\b", text):
                    kind = "mutable-projection"
                sites[name].append([relative(path), line, kind, stripped])
            if API.search(text):
                apis.append([relative(path), line, stripped])
    rows = []
    for struct, name, line, declaration in definitions:
        rows.append({"field": f"{struct}.{name}", "definition": line,
                     "declaration": declaration, "destination": destination(name, sites[name]),
                     "sites": sites[name]})
    # One field/API site per physical line; no generated file exceeds 250 lines.
    outputs = {}
    for prefix, records, size in [("fields", rows, 100), ("apis", apis, 200)]:
        for offset in range(0, len(records), size):
            outputs[f"{prefix}-{offset // size:03}.jsonl"] = "".join(
                json.dumps(row, ensure_ascii=False, separators=(",", ":")) + "\n"
                for row in records[offset:offset + size])
    metadata = {"schema": 1, "fields": len(rows), "api_sites": len(apis),
                "field_source_sha256": hashlib.sha256(FIELDS.read_text(encoding="utf-8").encode()).hexdigest(),
                "limitations": "Conservative token census; same-name false positives; whole-value writes in APIs; not runtime counts",
                "files": {name: hashlib.sha256(data.encode()).hexdigest() for name, data in outputs.items()}}
    outputs["identity.json"] = (OUT / "identity.json").read_text(encoding="utf-8")
    outputs["index.json"] = json.dumps(metadata, ensure_ascii=False, indent=2) + "\n"
    return outputs


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--snapshot", type=Path, required=True,
                        help="New private directory outside the repository")
    args = parser.parse_args()
    output = args.snapshot.resolve()
    if output == ROOT or ROOT in output.parents:
        parser.error("full snapshots belong outside the repository")
    output.mkdir(parents=True, exist_ok=False)
    outputs = census()
    for name, data in outputs.items():
        (output / name).write_text(data, encoding="utf-8", newline="\n")
    print(outputs["index.json"], end="")


if __name__ == "__main__":
    main()
