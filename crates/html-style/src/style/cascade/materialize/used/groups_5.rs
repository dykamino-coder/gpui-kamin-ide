//! Наложение изменённых used-операндов на разделяемые группы.
use super::*;
overlay!(
    inherited_text_ink,
    InheritedTextInk,
    text_shadow,
    text_shadow_rest,
    text_emphasis,
    emphasis_auto_shape,
    emphasis_position_auto,
    emphasis_under,
    emphasis_left,
    emphasis_color,
    underline_offset,
    underline_pos,
    skip_ink,
    skip_spaces
);
overlay!(
    inherited_writing,
    InheritedWriting,
    rtl,
    combine_upright,
    writing_mode,
    flat_mode,
    text_orientation
);
