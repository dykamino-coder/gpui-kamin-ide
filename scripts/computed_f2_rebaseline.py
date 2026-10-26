"""Bounded lead-reviewed F2 adjustments at their exact integration merge bases.

The lead approved only these existing fragment-copy counters. This adjustment
expires automatically when the merge base advances beyond the census commit;
later PRs still compare actual base sources and cannot spend removed debt.
"""

from collections import Counter

CENSUS_BASE = 'f05aec7b98ecd67c407fc47dac5d5a65fb744f66'
# field / owner / access, old ceiling, approved ceiling. No transferable budget.
APPROVED = (
    ('@borrow_mut', 'layout', 'escape', 134, 141),
    ('@mut-Computed', 'layout', 'escape', 19, 21),
    ('@used_mut', 'layout', 'escape', 356, 362),
    ('Computed.bdb_clone', 'layout', 'write', 1, 2),
    ('Computed.border_box', 'layout', 'write', 3, 5),
    ('Computed.border_width', 'layout', 'mutable', 2, 4),
    ('Computed.column_count', 'layout', 'write', 1, 2),
    ('Computed.column_width', 'layout', 'write', 1, 2),
    ('Computed.height', 'layout', 'mutable', 8, 10),
    ('Computed.height', 'layout', 'write', 61, 63),
    ('Computed.max_height', 'layout', 'mutable', 8, 10),
    ('Computed.max_height', 'layout', 'write', 9, 11),
    ('Computed.min_height', 'layout', 'mutable', 8, 10),
    ('Computed.min_height', 'layout', 'write', 12, 14),
    ('Computed.padding', 'layout', 'mutable', 15, 17),
    ('Computed.page', 'layout', 'write', 1, 2),
    ('Computed.position', 'layout', 'write', 19, 20),
    ('Computed.svg_no_css_box', 'dom', 'write', 0, 1),
    ('InheritedStyle.writing_mode', 'layout', 'write', 4, 5),
)


OVERFLOW_FLOAT_BASE = '948ed6dd2733e643908ccd6a444411811de5fcef'
# Reviewed: not a style escape. Float growth floors contain only u64/f32
# (layout/fragment/push/float_flow.rs); context interning contains FlowMeasure
# (layout/fragment/flow/contexts.rs). Neither map exposes a style operand.
OVERFLOW_FLOAT_NON_STYLE = (
    ('@make_mut', 'layout', 'escape', 1, 2),
    ('@borrow_mut', 'layout', 'escape', 141, 142),
)


def approved_base(counts, revision):
    """Replace only the enumerated old counts at the exact census merge base."""
    result = Counter(counts)
    approved = APPROVED if revision == CENSUS_BASE else OVERFLOW_FLOAT_NON_STYLE if revision == OVERFLOW_FLOAT_BASE else ()
    if approved:
        for field, owner, access, old, new in approved:
            key = field, owner, access
            if result[key] != old:
                raise ValueError(f'F2 census base changed: {key}')
            result[key] = new
    return result
