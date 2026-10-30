//! Итеративные компоненты сильной связности и вычисление custom-media DAG.

use super::*;
use crate::budget::admission::{Admission, Buffer, Error};

struct Visit {
    index: usize,
    low: usize,
    pending: bool,
}
struct Frame {
    name: String,
    dependencies: std::vec::IntoIter<String>,
    self_loop: bool,
}

fn enter(
    name: String,
    raw: &HashMap<String, String>,
    visits: &mut HashMap<String, Visit>,
    pending: &mut Buffer<String>,
    frames: &mut Buffer<Frame>,
    admission: &Admission,
) -> Result<(), Error> {
    let query = &raw[&name];
    admission.work(query.len())?;
    let dependencies = dependencies(query);
    let self_loop = dependencies.iter().any(|dependency| dependency == &name);
    let index = visits.len();
    visits.insert(
        name.clone(),
        Visit {
            index,
            low: index,
            pending: true,
        },
    );
    pending.push(name.clone(), admission)?;
    frames.push(
        Frame {
            name,
            dependencies: dependencies.into_iter(),
            self_loop,
        },
        admission,
    )
}

/// SCC завершается до вычисления её зависимых: каждый участник цикла
/// неопределён, даже если обратное ребро ведёт в уже завершённую ветвь DFS.
pub fn resolve_custom_media(
    raw: HashMap<String, String>,
    media: Media,
    vcx: ValueCx<'_>,
) -> Result<HashMap<String, SupTri>, Error> {
    let admission = Admission::new(vcx.doc.revision_budget());
    let mut frames: Buffer<Frame> = Buffer::default();
    let mut pending = Buffer::default();
    let mut visits: HashMap<String, Visit> = HashMap::new();
    let mut values = HashMap::new();
    let mut names: Vec<_> = raw.keys().cloned().collect();
    names.sort_unstable();
    for root in names {
        if visits.contains_key(&root) {
            continue;
        }
        enter(
            root,
            &raw,
            &mut visits,
            &mut pending,
            &mut frames,
            &admission,
        )?;
        while let Some(frame) = frames.last_mut() {
            admission.work(1)?;
            if let Some(name) = frame.dependencies.next() {
                if let Some(visit) = visits.get(&name) {
                    if visit.pending {
                        let index = visit.index;
                        let parent = visits.get_mut(&frame.name).expect("active media visit");
                        parent.low = parent.low.min(index);
                    }
                } else if raw.contains_key(&name) {
                    enter(
                        name,
                        &raw,
                        &mut visits,
                        &mut pending,
                        &mut frames,
                        &admission,
                    )?;
                }
                continue;
            }
            let frame = frames.pop().expect("completed media frame");
            let visit = &visits[&frame.name];
            let low = visit.low;
            if low == visit.index {
                let cyclic = frame.self_loop || pending.last() != Some(&frame.name);
                loop {
                    let name = pending.pop().expect("pending media component");
                    visits.get_mut(&name).expect("pending media visit").pending = false;
                    let value = if cyclic {
                        SupTri::Unknown
                    } else {
                        match raw[&name].as_str() {
                            "true" => SupTri::True,
                            "false" => SupTri::False,
                            query => {
                                if media.matches_in(
                                    query,
                                    CustomMedia {
                                        aliases: Some(&values),
                                    },
                                    vcx,
                                ) {
                                    SupTri::True
                                } else {
                                    SupTri::False
                                }
                            }
                        }
                    };
                    let done = name == frame.name;
                    values.insert(name, value);
                    if done {
                        break;
                    }
                }
            }
            if let Some(parent) = frames.last() {
                let parent = visits.get_mut(&parent.name).expect("parent media visit");
                parent.low = parent.low.min(low);
            }
        }
    }
    Ok(values)
}

fn dependencies(query: &str) -> Vec<String> {
    query
        .char_indices()
        .filter_map(|(at, c)| {
            if c != '(' {
                return None;
            }
            let (inner, _) = take_parens(&query[at..])?;
            let name = inner.trim();
            (name.starts_with("--")
                && name
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_')))
            .then(|| name.into())
        })
        .collect()
}
