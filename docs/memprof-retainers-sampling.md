# Exact retention graphs with sampled allocation labels

An exact allocation log can exceed the disk limit before a large page reaches
its retention checkpoint. To bound this log, select a sampling interval explicitly:

```sh
python -I scripts/memprof.py run --exe CAND.exe --retainers --sample 16384 \
  --out DIR -- PAGE.html --mode cold --runs 3
```

The native retainer table still records every live Rust allocation. Block sizes,
edges, dominators and retained bytes are exact within the conservative graph's
documented scope. The option samples only allocation-stack labels. It does not
sample blocks or edges and does not weight graph sizes by sampling probability.

Labels use measured owner paths wherever available. Other blocks without a
sampled allocation stack keep the label `(no stack)`. The text report gives the
percentage of graph bytes with allocation-stack labels; the JSON records
`named_bytes`, `unnamed_bytes` and `graph_sampled: false`. Measured owner labels
can also name blocks lacking an allocation stack. Never infer a type or an
allocation site for an unnamed block.

Without an explicit interval, `--retainers` retains its exact allocation log.
The ordinary allocation-site report accompanying a sampled capture remains an
estimate, even though its retention graph is exact. Graph collection still runs
once, after the first mode's owner report. This option does not select a later
mode or provide a graph of the allocation peak. All disk, timeout, process-memory
and host-hold guards remain in force.
The collector excludes the stack sampler's raw-pointer table from static roots.
Those addresses label allocations; they do not own them. Other static roots,
including words adjacent to that table, are still scanned.
