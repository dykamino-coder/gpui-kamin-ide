use super::*;

fn base(dir: &Path) -> Args {
    let page = dir.join("p.html");
    std::fs::write(&page, "<p>x</p>").expect("write page");
    Args::parse([page.to_string_lossy().into_owned()])
        .expect("parse")
        .expect("args")
}

#[test]
fn plan_phases_take_their_own_input_and_drop_the_steady_window() {
    let dir = std::env::temp_dir().join(format!("engine-bench-plan-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("dir");
    let b = base(&dir);
    let plan = Plan::parse(&b, r#"{"series_ms": 400, "min_runs": 4, "memory_settle_ms": 2000, "stable_mad": 0.03,
            "phases": [{"name": "R", "mode": "relayout", "input": "p.html", "runs": 12},
                       {"name": "C", "mode": "edit", "edit": "class", "input": "p.html"},
                       {"name": "S", "mode": "scroll", "input": "p.html", "runs": 5, "min_runs": 1, "series_ms": 60000, "stable_mad": 0}]}"#, &dir)
        .expect("plan");
    assert_eq!(plan.phases.len(), 3);
    for phase in &plan.phases {
        assert_eq!(
            phase.args.animation_time(0),
            Some(std::time::Duration::ZERO)
        );
        assert_eq!(
            phase.args.animation_time(3),
            Some(std::time::Duration::from_micros(50_001))
        );
    }
    let s = &plan.phases[2].args;
    assert_eq!(
        (s.runs, s.min_runs, s.series_ms, s.stable_mad),
        (5, 1, 60000, 0.0)
    );
    assert_eq!(plan.phases[0].args.stable_mad, 0.03);
    assert!(
        Plan::parse(
            &b,
            r#"{"stable_mad": 1.5, "phases": [{"name": "R", "mode": "warm", "input": "p.html"}]}"#,
            &dir
        )
        .is_err()
    );
    assert_eq!(
        (plan.phases[1].args.min_runs, plan.phases[1].args.series_ms),
        (4, 400)
    );
    let r = &plan.phases[0].args;
    assert_eq!(
        (r.mode, r.runs, r.series_ms, r.min_runs, r.steady_each_phase),
        (Mode::Relayout, 12, 400, 4, false)
    );
    assert_eq!(plan.phases[1].args.edit, Edit::Class);
    assert_eq!(plan.memory_settle_ms, 2000);
    assert!(Plan::parse(&b, r#"{"phases": []}"#, &dir).is_err());
    assert!(
        Plan::parse(
            &b,
            r#"{"phases": [{"name": "X", "mode": "both", "input": "p.html"}]}"#,
            &dir
        )
        .is_err()
    );
    assert!(Plan::parse(&b, r#"{"memory_settle_ms": 10, "phases": [{"name": "R", "mode": "warm", "input": "p.html"}]}"#, &dir).is_err());
    assert!(
        Plan::parse(
            &b,
            r#"{"phases": [{"name": "R", "mode": "warm", "input": "missing.html"}]}"#,
            &dir
        )
        .is_err()
    );
    std::fs::remove_dir_all(&dir).expect("cleanup");
}
