//! Сборка этого exe для JSON стенда: features, профиль и его переопределения
//! (`build.rs`). По этой записи скрипты сравнения отвергают пару exe, собранных
//! по-разному.

use serde_json::{Value, json};

/// Включённые features пакета, по алфавиту.
fn features() -> Vec<&'static str> {
    [
        ("allocation-audit", cfg!(feature = "allocation-audit")),
        ("engine-bench", cfg!(feature = "engine-bench")),
        ("heap-profile", cfg!(feature = "heap-profile")),
        ("heap-report", cfg!(feature = "heap-report")),
        ("timing-only", cfg!(feature = "timing-only")),
        ("work-counters", cfg!(feature = "work-counters")),
    ]
    .into_iter()
    .filter_map(|(name, on)| on.then_some(name))
    .collect()
}

pub(super) fn json() -> Value {
    let overrides: Vec<&str> = env!("ENGINE_BENCH_BUILD_OVERRIDES")
        .split_whitespace()
        .collect();
    json!({
        "features": features(),
        "profile": env!("ENGINE_BENCH_BUILD_PROFILE"),
        "opt_level": env!("ENGINE_BENCH_BUILD_OPT_LEVEL"),
        "debug_assertions": cfg!(debug_assertions),
        "profile_overrides": overrides,
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn build_record_names_features_and_profile() {
        let build = super::json();
        let features: Vec<&str> = build["features"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| f.as_str().unwrap())
            .collect();
        assert!(features.contains(&"timing-only"));
        assert!(features.is_sorted());
        assert!(!build["profile"].as_str().unwrap().is_empty());
        assert!(!build["opt_level"].as_str().unwrap().is_empty());
    }
}
