//! Одинаковые конечные пробы синхронного и приостанавливаемого поиска.
use super::*;

#[test]
fn feasible_and_infeasible_bounds_meet_at_adjacent_layout_units() {
    let threshold = 137.25f32;
    let mut heights = HashSet::new();
    let (height, chosen) = search(1., 200., false, |height| {
        assert!(heights.insert(height.to_bits()), "repeated trial");
        Trial {
            fits: height >= threshold,
            shortage: None,
            value: height,
        }
    })
    .unwrap();
    assert_eq!(height, threshold);
    assert_eq!(chosen, height);
    assert_eq!(before(height), threshold - 1. / 64.);
}

#[test]
fn positive_shortage_stretches_to_the_first_feasible_height() {
    let mut heights = Vec::new();
    let (height, _) = search(25., 500., false, |height| {
        heights.push(height);
        Trial {
            fits: height >= 100.,
            shortage: Some(100. - height),
            value: (),
        }
    })
    .unwrap();
    assert_eq!(height, 100.);
    assert_eq!(heights, [25., 100.]);
}

#[test]
fn minimal_shortage_rejects_the_heights_it_skips() {
    // Разрывы пробы меняются только после прироста на минимальный
    // недолаз; срез пустой коробки внутри пропуска не проверяется.
    let mut heights = Vec::new();
    let (height, _) = search(65., 150., false, |height| {
        heights.push(height);
        Trial {
            fits: height >= 75.,
            shortage: Some(20.),
            value: (),
        }
    })
    .unwrap();
    assert_eq!((height, heights), (85., vec![65., 85.]));
}

#[test]
fn repeated_shortage_steps_until_the_stretched_height_fits() {
    let mut heights = Vec::new();
    let (height, _) = search(10., 100., false, |height| {
        heights.push(height);
        Trial {
            fits: height >= 30.,
            shortage: Some(7.5),
            value: (),
        }
    })
    .unwrap();
    assert_eq!((height, heights), (32.5, vec![10., 17.5, 25., 32.5]));
}

#[test]
fn rounded_shortage_advances_one_layout_unit() {
    let floor = 1024f32;
    let (height, _) = search(floor, floor + 1. / 64., false, |height| Trial {
        fits: height > floor,
        shortage: Some(f32::MIN_POSITIVE),
        value: (),
    })
    .unwrap();
    assert_eq!(height, floor + 1. / 64.);
}

#[test]
fn css_ceiling_between_layout_units_is_preserved() {
    let cap = 10.001;
    let (height, _) = search(10., cap, true, |_| Trial {
        fits: false,
        shortage: Some(1.),
        value: (),
    })
    .unwrap();
    assert_eq!(height, cap);
}

#[test]
fn untested_adjacent_upper_bound_is_tried_without_shortage() {
    let cap = 10.001;
    let (height, _) = search(10., cap, false, |h| Trial {
        fits: h == cap,
        shortage: None,
        value: (),
    })
    .unwrap();
    assert_eq!(height, cap);
    assert_eq!(
        min_shortage([Some(30.), None, Some(10.), Some(50.)]),
        Some(10.)
    );
}

#[test]
fn css_ceiling_can_overflow_but_unproven_unconstrained_bound_is_an_error() {
    let infeasible = |_| Trial {
        fits: false,
        shortage: None,
        value: (),
    };
    assert_eq!(search(10., 20., true, infeasible).unwrap().0, 20.);
    assert_eq!(search(10., 20., false, infeasible), Err(Error::NoProgress));
    assert_eq!(
        search(20., 10., false, infeasible),
        Err(Error::InvalidGeometry)
    );
}

#[test]
fn best_feasible_trial_survives_later_infeasible_trials() {
    let mut sequence = 0;
    let mut accepted = Vec::new();
    let (height, chosen) = search(1., 200., false, |height| {
        sequence += 1;
        if height >= 101. {
            accepted.push((height, sequence));
        }
        Trial {
            fits: height >= 101.,
            shortage: None,
            value: sequence,
        }
    })
    .unwrap();
    assert_eq!((height, chosen), *accepted.last().unwrap());
    assert_eq!(height, 101.);
}

#[test]
fn lending_search_matches_synchronous_probe_order() {
    let mut sync_trace = Vec::new();
    let mut async_trace = Vec::new();
    let trial = |height| Trial {
        fits: height >= 137.25,
        shortage: None,
        value: height,
    };
    let expected = search(1., 200., false, |height| {
        sync_trace.push(height);
        trial(height)
    })
    .unwrap();
    let actual = reblessive::Stack::new()
        .enter(|stack| {
            search_async(1., 200., false, async |height| {
                async_trace.push(height);
                stack.run(|_| async { trial(height) }).await
            })
        })
        .finish()
        .unwrap();
    assert_eq!(actual, expected);
    assert_eq!(async_trace, sync_trace);
}
