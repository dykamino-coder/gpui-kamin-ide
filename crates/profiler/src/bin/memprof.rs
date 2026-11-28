//! Offline adapter for the same reports and graph queries used by DevTools.
use kamin_profiler::{graph, report, symbols::Symbols, wire::Archive};
use serde_json::{Value, json};
use std::{
    error::Error,
    fs::File,
    io::{self, BufReader, BufWriter, Write},
    path::Path,
};

fn archive(path: &str) -> Result<Archive, Box<dyn Error>> {
    Ok(Archive::read(&mut BufReader::new(File::open(path)?))?)
}

fn fold(path: &str, raw: bool) -> Result<Value, Box<dyn Error>> {
    let archive = archive(path)?;
    let mut symbols = Symbols::open(&archive.modules)?;
    Ok(report::snapshot(
        &archive.snapshot,
        &archive.build,
        &mut symbols,
        raw,
    ))
}

fn run(args: &[String]) -> Result<(), Box<dyn Error>> {
    let usage = "usage: kamin_memprof report SNAPSHOT [--raw] | diff BEFORE AFTER | graph-report GRAPH | graph-query GRAPH cut|holders PATTERN... | graph-export GRAPH OUTPUT";
    let Some(command) = args.first().map(String::as_str) else {
        return Err(usage.into());
    };
    let result = match (command, &args[1..]) {
        ("report", [path]) => fold(path, false)?,
        ("report", [path, flag]) if flag == "--raw" => fold(path, true)?,
        ("diff", [before, after]) => {
            let before = archive(before)?;
            let after = archive(after)?;
            if before.build.features != after.build.features {
                return Err("cannot compare snapshots from different feature sets".into());
            }
            let mut a = Symbols::open(&before.modules)?;
            let mut b = Symbols::open(&after.modules)?;
            report::diff(&before.snapshot, &after.snapshot, &mut a, &mut b)
        }
        ("graph-report", [path]) => {
            let (graph, meta) = graph::wire::read(&mut BufReader::new(File::open(path)?))?;
            let view = graph::View::new(&graph, &meta.labels)?;
            json!({"schema": 1, "metadata": meta, "observed_requested_bytes": graph.retained[graph.root() as usize], "groups": view.groups()})
        }
        ("graph-query", [path, query, patterns @ ..]) if !patterns.is_empty() => {
            let (graph, meta) = graph::wire::read(&mut BufReader::new(File::open(path)?))?;
            let view = graph::View::new(&graph, &meta.labels)?;
            match query.as_str() {
                "cut" => serde_json::to_value(view.cut(patterns))?,
                "holders" => serde_json::to_value(view.holders(patterns))?,
                _ => return Err(usage.into()),
            }
        }
        ("graph-export", [path, output]) => {
            if Path::new(path).canonicalize()?
                == Path::new(output).canonicalize().unwrap_or_default()
            {
                return Err("graph export would overwrite its input".into());
            }
            let (graph, meta) = graph::wire::read(&mut BufReader::new(File::open(path)?))?;
            let view = graph::View::new(&graph, &meta.labels)?;
            let mut writer = BufWriter::new(File::create(output)?);
            graph::export::heapsnapshot(&view, &mut writer)?;
            writer.flush()?;
            return Ok(());
        }
        _ => return Err(usage.into()),
    };
    let mut writer = BufWriter::new(io::stdout().lock());
    serde_json::to_writer(&mut writer, &result)?;
    writer.flush()?;
    Ok(())
}

fn main() {
    if let Err(error) = run(&std::env::args().skip(1).collect::<Vec<_>>()) {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
