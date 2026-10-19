//! Opt-in deterministic frame lists and once-per-pair animation source detection.

use std::time::Duration;

pub(super) fn configured() -> Option<Vec<Duration>> {
    let value = match std::env::var("WPT_ANIMATION_FRAMES") {
        Ok(value) => value,
        Err(std::env::VarError::NotPresent) => return None,
        Err(error) => {
            eprintln!("Invalid WPT_ANIMATION_FRAMES: {error}");
            std::process::exit(2);
        }
    };
    Some(parse(&value).unwrap_or_else(|error| {
        eprintln!("Invalid WPT_ANIMATION_FRAMES: {error}");
        std::process::exit(2);
    }))
}

fn parse(value: &str) -> Result<Vec<Duration>, &'static str> {
    value
        .split(',')
        .map(|part| {
            let part = part.trim();
            if part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit()) {
                return Err("expected comma-separated nonnegative integer milliseconds");
            }
            part.parse::<u64>()
                .map(Duration::from_millis)
                .map_err(|_| "milliseconds overflow u64")
        })
        .collect()
}

fn animated(test: &str, reference: &str) -> bool {
    [test, reference].iter().any(|source| {
        let lower = source.to_ascii_lowercase();
        [
            "@keyframes",
            "animation:",
            "transition:",
            "<animate",
            "<set",
        ]
        .iter()
        .any(|marker| lower.contains(marker))
            || ["animation", "transition"].iter().any(|property| {
                lower
                    .match_indices(property)
                    .any(|(at, _)| lower[at + property.len()..].trim_start().starts_with(':'))
            })
    })
}

/// Resolve stylesheets just as capture does, so external CSS also selects frames.
pub(super) fn for_pair(
    frames: Option<&[Duration]>,
    test: &str,
    reference: &str,
) -> Vec<Option<Duration>> {
    if let Some(frames) = frames {
        let sources: Vec<_> = [test, reference]
            .iter()
            .map(|path| {
                super::links::resolve_links(
                    &std::fs::read_to_string(path).unwrap_or_default(),
                    path,
                )
            })
            .collect();
        if animated(&sources[0], &sources[1]) {
            return frames.iter().copied().map(Some).collect();
        }
    }
    vec![None]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_order_duplicates_zero_and_whitespace_are_preserved() {
        let frames = parse(" 500,0,125,500,1000 ").unwrap();
        assert_eq!(
            frames.iter().map(Duration::as_millis).collect::<Vec<_>>(),
            [500, 0, 125, 500, 1000]
        );
        assert_eq!(
            parse("18446744073709551615"),
            Ok(vec![Duration::from_millis(u64::MAX)])
        );
    }

    #[test]
    fn invalid_or_empty_frame_lists_are_rejected() {
        for input in [
            "",
            " ",
            ",",
            "0,",
            ",0",
            "0,,500",
            "-1",
            "+1",
            "1.5",
            "1ms",
            "18446744073709551616",
        ] {
            assert!(parse(input).is_err(), "{input}");
        }
    }

    #[test]
    fn either_side_selects_frames_for_every_requested_marker() {
        for source in [
            "@keyframes move {}",
            "animation: move 1s",
            "transition: all 1s",
            "<animateTransform/>",
            "<set attributeName='x'/>",
            "ANIMATION : move 1s",
            "TRANSITION\n: all 1s",
        ] {
            assert!(animated(source, "<div/>"), "{source}");
            assert!(animated("<div/>", source), "{source}");
        }
    }

    #[test]
    fn static_sources_keep_a_single_capture() {
        assert!(!animated(
            "<div>animation transition</div>",
            "<style>div{color:green}</style>"
        ));
        assert_eq!(for_pair(None, "unused", "unused"), vec![None]);
        assert_eq!(
            for_pair(Some(&[Duration::ZERO]), "missing-test", "missing-ref"),
            vec![None]
        );
    }
}
