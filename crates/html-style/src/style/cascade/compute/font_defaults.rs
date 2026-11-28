//! Generic monospace has its own default size; explicit lengths retain theirs.

use super::*;

const NORMAL: [f32; 8] = [9., 10., 13., 16., 18., 24., 32., 48.];
const MONO: [f32; 8] = [9., 10., 12., 13., 16., 20., 26., 39.];

fn fixed(style: &Computed) -> bool {
    style.inh.font.monospace == Some(true)
        && style.inh.font.font_family.is_none()
        && style
            .inh
            .font
            .font_families
            .as_ref()
            .is_some_and(|v| v.len() == 1)
}

pub(in crate::style::cascade) fn inherit_size(
    parent: &Computed,
    own: &Computed,
    c: &mut Computed,
    vcx: ValueCx<'_>,
) {
    let inherited = own.inh.font.font_size.is_none() && own.inh.font.font_size_origin == 0;
    let origin = if inherited {
        parent.inh.font.font_size_origin
    } else if own.inh.font.font_size_origin == 9 && parent.inh.font.font_size_origin == 10 {
        10
    } else if own.inh.font.font_size_origin == 0 {
        10
    } else {
        own.inh.font.font_size_origin
    };
    if c.inh.font.font_size_origin != origin {
        c.inh.font.font_size_origin = origin;
    }
    let mono = fixed(c);
    let changed = mono != fixed(parent);
    if origin == 10 || (!changed && (inherited || !mono || origin >= 9)) {
        return;
    }
    let px = if origin <= 8 {
        let i = if origin == 0 {
            3
        } else {
            usize::from(origin - 1)
        };
        (if mono { MONO[i] } else { NORMAL[i] }) * c.inh.font.zoom_eff.unwrap_or(1.)
    } else {
        // Initial and relative sizes use the UA default-size ratio.
        let parent_px = super::parent_font_px(parent);
        let px = c
            .computed_font_px(parent.font_cx(vcx), vcx)
            .unwrap_or(parent_px);
        px * if mono { 13. / 16. } else { 16. / 13. }
    };
    c.inh.font.font_size = Some(Len::Px(px));
    c.reset_cascade.font_size_step = 0;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn board_appearance_monospace_defaults_preserve_explicit_and_relative_sizes() {
        let parsed = crate::dom::parse(
            "<!doctype html><style>.mono{font-family:monospace}</style>\
             <code id=a>x</code><div class=mono id=b>x<span style='font-family:serif' id=c>x</span></div>\
             <div style='font-size:20px'><code id=d>x</code></div>\
             <div style='font-size:100%'><code id=e>x</code></div>\
             <div style='font-size:1em'><code id=f>x</code></div>\
             <code style='font-size:medium' id=g>x</code>\
             <code style='font-size:small' id=h>x</code>\
             <code style='font-family:monospace,monospace' id=i>x</code>\
             <code style='font-size:1rem' id=j>x</code>\
             <code style='font-size:larger' id=k>x</code>\
             <div class=mono><span style='font-size:120%' id=l>x</span></div>\
             <div style='font-family:Example'><code id=m>x</code></div>\
             <code style='font-size:calc(1em)' id=n>x</code>\
             <code style='font-size:calc(100%)' id=o>x</code>\
             <code style='font-size:calc(100% + 0px)' id=p>x</code>\
             <div class=mono><span style='font-size:larger' id=q>x</span></div>\
             <div style='font-size:medium'><span style='font-size:larger' id=s>x</span></div>",
            "",
        );
        for (id, px) in [
            ("a", 13.),
            ("b", 13.),
            ("c", 16.),
            ("d", 20.),
            ("e", 13.),
            ("f", 13.),
            ("g", 13.),
            ("h", 12.),
            ("i", 16.),
            ("j", 16.),
            ("k", 15.6),
            ("l", 15.6),
            ("m", 13.),
            ("n", 16.),
            ("o", 13.),
            ("p", 16.),
            ("q", 15.6),
            ("s", 19.2),
        ] {
            let node = crate::dom::test_support::find(&parsed.nodes, &|e| e.attr("id") == Some(id))
                .unwrap();
            assert!(
                matches!(node.style.used().inh.font.font_size, Some(Len::Px(v)) if (v-px).abs()<0.001),
                "{id}: {:?}",
                node.style.used().inh.font
            );
        }
        let root = crate::dom::parse(
            "<!doctype html><style>html{font-family:monospace}p{font-size:1rem}</style><p id=r>x",
            "",
        );
        let node =
            crate::dom::test_support::find(&root.nodes, &|e| e.attr("id") == Some("r")).unwrap();
        assert_eq!(node.style.used().inh.font.font_size, Some(Len::Px(13.)));
    }
}
