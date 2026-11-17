"""Who keeps a block alive: the retention graph of engine_bench (docs/memprof.md, «Who keeps a block alive»).

The bench (ENGINE_BENCH_RETAINERS_OUT, crates/engine-bench/src/allocations/retain) scans every live
block and its roots once, after the owner report of the first mode, and writes the graph with the
shortest path from a root to each block and the dominator tree (crates/heap/src/retain). This module
names the nodes from the probe file of the same run (owner path when an owner measured the block,
else the [type] and site that allocated it) and prints:

  - retained by root: the snapshot thread's stack, the image statics, its TLS, and «unrooted» (held
    by what the scan does not see: other threads' stacks, OS memory, or nobody — a leak);
  - the dominator tree from the root, largest first, with chains of one dominant child folded;
  - retained by group (owner path or site), not nested, with object counts and self bytes;
  - for the largest holders, the dominator chain and the shortest path from a root, which name the
    holder of a block rather than its allocation site.

`export_heapsnapshot` writes the same graph as a V8 .heapsnapshot for the Memory panel of Chrome.
"""

import array
import collections
import json
import pathlib
import struct

MAGIC = b"kamin retain graph v1\n"
NONE = 0xFFFFFFFF
FLAGS = {1: "unheld", 2: "cycle"}


class Graph:
    """The binary graph file (crates/heap/src/retain/write.rs)."""

    def __init__(self, path):
        data = pathlib.Path(path).read_bytes()
        if not data.startswith(MAGIC):
            raise SystemExit(f"{path}: not a kamin retain graph")
        at = len(MAGIC)
        (length,) = struct.unpack_from("<I", data, at)
        at += 4
        self.header = json.loads(data[at:at + length])
        at += length
        self.blocks, self.nodes, self.edge_count, region_count = struct.unpack_from("<4Q", data, at)
        at += 32
        self.regions = []
        for _ in range(region_count):
            start, size, name_length = struct.unpack_from("<QQI", data, at)
            at += 20
            self.regions.append((data[at:at + name_length].decode("utf-8"), start, size))
            at += name_length

        def take(code, count):
            nonlocal at
            values = array.array(code)
            values.frombytes(data[at:at + count * values.itemsize])
            at += count * values.itemsize
            return values

        n, t, e = self.blocks, self.nodes, self.edge_count
        self.address, self.size = take("Q", n), take("Q", n)
        self.edge_start, self.edge_to, self.edge_offset = take("I", t + 1), take("I", e), take("I", e)
        self.idom, self.retained = take("I", t), take("Q", t)
        self.parent, self.parent_offset = take("I", t), take("I", t)
        self.flags = data[at:at + t]
        self.root, self.unrooted = n, n + 1 + region_count

    def self_bytes(self, node):
        return self.size[node] if node < self.blocks else 0


class Names:
    """Labels of nodes: owner path, [type] site and subsystem from the probe file of the run."""

    def __init__(self, graph, run):
        self.graph = graph
        held = (run.held if run else None) or {}
        measured = (run.measured if run else None) or {}
        self.symbols = run.symbols if run else None
        self.site, self.owner, self.subsystem = [], [], []
        for address in graph.address:
            pointer = format(address, "x")
            hit = held.get(pointer)
            if hit:
                (subsystem, site, owned, _stack), _ = hit
                self.site.append(f"[{owned}] {site}" if owned else site)
                self.subsystem.append(subsystem)
            else:
                self.site.append("(no stack)")
                self.subsystem.append("?")
            path = measured.get(pointer)
            self.owner.append(path[0] if path else None)

    def label(self, node):
        g = self.graph
        if node == g.root:
            return "(root)"
        if node == g.unrooted:
            return "(unrooted: held by other threads, OS memory or nobody)"
        if node > g.root:
            return f"({g.regions[node - g.root - 1][0]})"
        owner = self.owner[node]
        flag = FLAGS.get(g.flags[node])
        text = f"{owner} | {self.site[node]}" if owner else self.site[node]
        return f"{text} {{{flag}}}" if flag else text

    def group(self, node):
        if node >= self.graph.blocks:
            return self.label(node)
        return self.owner[node] or self.site[node]

    def hop(self, node, offset):
        """Where the pointer to the next node is stored: a field offset, or a static by symbol."""
        g = self.graph
        if g.root < node < g.unrooted and self.symbols is not None:
            name, start, _ = g.regions[node - g.root - 1]
            if name.startswith("static"):
                symbol = self.symbols.name(start + offset + 1)
                if symbol != "?":
                    return f"{symbol}"
        return f"+0x{offset:x}"


def mb(value):
    return f"{value / 1e6:9.3f}"


def children(graph):
    """Dominator tree as CSR: (start, kids), kids of each node sorted by retained, largest first."""
    t = graph.nodes
    count = array.array("I", bytes(4 * (t + 1)))
    for node in range(t):
        parent = graph.idom[node]
        if parent != NONE and node != graph.root:
            count[parent + 1] += 1
    for index in range(t):
        count[index + 1] += count[index]
    fill = array.array("I", count)
    kids = array.array("I", bytes(4 * max(count[t], 1)))
    for node in range(t):
        parent = graph.idom[node]
        if parent != NONE and node != graph.root:
            kids[fill[parent]] = node
            fill[parent] += 1
    return count, kids


def sorted_kids(graph, tree, node):
    start, kids = tree
    return sorted(kids[start[node]:start[node + 1]], key=lambda kid: -graph.retained[kid])


def dominator_tree(graph, names, tree, out, threshold, depth_limit=14):
    """Largest first; a chain where one child keeps almost all of its parent is folded on one line."""
    total = graph.retained[graph.root]
    rows = []
    stack = [(graph.root, 0)]
    while stack:
        node, depth = stack.pop()
        chain = [node]
        kids = sorted_kids(graph, tree, node)
        while len(kids) >= 1 and graph.retained[kids[0]] >= 0.97 * graph.retained[chain[-1]] \
                and (len(kids) == 1 or graph.retained[kids[1]] < threshold) and len(chain) < 6:
            chain.append(kids[0])
            kids = sorted_kids(graph, tree, kids[0])
        last = chain[-1]
        text = "  ".join(names.label(item) for item in chain[-3:])
        if len(chain) > 3:
            text = f"… {text}"
        rows.append({"retained_bytes": graph.retained[last], "self_bytes": graph.self_bytes(last),
                     "depth": depth, "chain": [names.label(item) for item in chain]})
        print(f"{mb(graph.retained[last])} {100 * graph.retained[last] / max(total, 1):5.1f}%  "
              f"{'  ' * depth}{text}", file=out)
        if depth >= depth_limit:
            continue
        big = [kid for kid in kids if graph.retained[kid] >= threshold]
        rest = sum(graph.retained[kid] for kid in kids) - sum(graph.retained[kid] for kid in big)
        if rest >= threshold:
            print(f"{mb(rest)} {100 * rest / max(total, 1):5.1f}%  {'  ' * (depth + 1)}"
                  f"(smaller: {len(kids) - len(big)} subtrees)", file=out)
        stack.extend((kid, depth + 1) for kid in reversed(big))
    return rows


def by_group(graph, names, tree):
    """{group: [objects, self bytes, retained bytes not counted under a dominator of the same group]}."""
    groups = collections.defaultdict(lambda: [0, 0, 0])
    active = collections.Counter()
    start, kids = tree
    stack = [(graph.root, False)]
    while stack:
        node, leaving = stack.pop()
        group = names.group(node) if node < graph.blocks else None
        if leaving:
            if group is not None:
                active[group] -= 1
            continue
        if group is not None:
            entry = groups[group]
            entry[0] += 1
            entry[1] += graph.size[node]
            if not active[group]:
                entry[2] += graph.retained[node]
            active[group] += 1
        stack.append((node, True))
        stack.extend((kid, False) for kid in kids[start[node]:start[node + 1]])
    return groups


def walk(graph, node, step):
    seen = set()
    while node not in seen and node != NONE:
        seen.add(node)
        yield node
        if node == graph.root:
            return
        node = step[node]


def holder_paths(graph, names, tree, out, top, threshold):
    """The largest maximal holders: blocks whose dominator is not a block, or whose share of their
    dominator is small; for each, the dominator chain up to the first named owner and the path."""
    candidates = []
    for node in range(graph.blocks):
        parent = graph.idom[node]
        if parent == NONE or graph.retained[node] < threshold:
            continue
        if parent >= graph.blocks or graph.retained[node] < 0.5 * graph.retained[parent]:
            candidates.append(node)
    candidates.sort(key=lambda node: -graph.retained[node])
    rows = []
    for node in candidates[:top]:
        chain = list(walk(graph, node, graph.idom))
        path = list(walk(graph, node, graph.parent))
        named = next((hop for hop in chain if hop < graph.blocks and names.owner[hop]), None)
        print(f"\n{mb(graph.retained[node])} MB  {names.label(node)}", file=out)
        print("  retained by (dominators): " + " <- ".join(names.label(hop) for hop in chain[1:12]), file=out)
        # From the root down; `@where` is the word of the previous node that points to the next.
        hops = [f"@{names.hop(graph.parent[hop], graph.parent_offset[hop])} {names.label(hop)}"
                for hop in reversed(path[:-1])]
        if len(hops) > 16:
            hops = hops[:8] + [f"... {len(hops) - 16} more"] + hops[-8:]
        print("  shortest path from a root: " + " > ".join([names.label(path[-1])] + hops), file=out)
        rows.append({"retained_bytes": graph.retained[node], "block": names.label(node),
                     "named_owner": names.label(named) if named is not None else None,
                     "dominators": [names.label(hop) for hop in chain],
                     "path": [names.label(hop) for hop in path]})
    return rows


def report(graph_path, run, out, top=25, threshold_mb=1.0):
    """Prints the retention report to `out` and returns it as a dict."""
    graph = Graph(graph_path)
    names = Names(graph, run)
    tree = children(graph)
    header = graph.header
    threshold = int(threshold_mb * 1e6)
    total = graph.retained[graph.root]
    print(f"memprof retainers: {graph_path} mode={header.get('mode')}", file=out)
    print(f"  {graph.blocks} blocks, {total / 1e6:.3f} MB, {graph.edge_count} edges; collect "
          f"{header.get('collect_ms')} ms, scan {header.get('scan_ms')} ms, analyze {header.get('analyze_ms')} ms; "
          f"missed {header.get('missed_blocks')}", file=out)
    named = sum(graph.size[i] for i in range(graph.blocks) if names.site[i] != "(no stack)")
    print(f"  named by the probe: {100 * named / max(total, 1):.2f}% of bytes", file=out)
    print("retained by root (MB):", file=out)
    roots = []
    for index, (name, _start, size) in enumerate(graph.regions):
        node = graph.root + 1 + index
        roots.append({"root": name, "retained_bytes": graph.retained[node], "bytes": size})
        print(f"  {mb(graph.retained[node])}  {name} ({size / 1e6:.2f} MB scanned)", file=out)
    flags = collections.Counter(graph.flags[:graph.blocks])
    roots.append({"root": "unrooted", "retained_bytes": graph.retained[graph.unrooted]})
    print(f"  {mb(graph.retained[graph.unrooted])}  unrooted ({flags[1]} unheld entries, {flags[2]} unheld cycles)",
          file=out)
    print(f"dominator tree (MB, share; subtrees >= {threshold_mb} MB):", file=out)
    tree_rows = dominator_tree(graph, names, tree, out, threshold)
    groups = by_group(graph, names, tree)
    rows = sorted(groups.items(), key=lambda item: -item[1][2])[:top]
    print("retained by group, not nested (MB retained, objects, MB self, group):", file=out)
    for group, (objects, own, kept) in rows:
        print(f"  {mb(kept)} {objects:9d} {mb(own)}  {group}", file=out)
    print("largest holders (dominator chain and shortest path from a root):", file=out)
    holders = holder_paths(graph, names, tree, out, min(top, 12), threshold)
    return {"graph": str(graph_path), "header": header, "bytes": total, "blocks": graph.blocks,
            "roots": roots, "dominator_tree": tree_rows, "holders": holders,
            "groups": [{"group": g, "objects": o, "self_bytes": s, "retained_bytes": r}
                       for g, (o, s, r) in sorted(groups.items(), key=lambda item: -item[1][2])]}


def diff(cand, base, out, top=30):
    """Retained by group, candidate against base (two saved reports)."""
    index = {}
    for side, data in (("cand", cand), ("base", base)):
        for row in data["groups"]:
            index.setdefault(row["group"], {"cand": 0, "base": 0})[side] = row["retained_bytes"]
    print(f"retained by group: cand {cand['bytes'] / 1e6:.3f} MB, base {base['bytes'] / 1e6:.3f} MB, "
          f"delta {(cand['bytes'] - base['bytes']) / 1e6:+.3f} MB", file=out)
    rows = sorted(index.items(), key=lambda item: -abs(item[1]["cand"] - item[1]["base"]))[:top]
    for group, sides in rows:
        print(f"  {(sides['cand'] - sides['base']) / 1e6:+10.3f} {mb(sides['cand'])} {mb(sides['base'])}  {group}",
              file=out)
    return rows


def export_heapsnapshot(graph_path, run, path):
    """V8 .heapsnapshot: node 0 is the root, then the roots, «unrooted» and the blocks."""
    graph = Graph(graph_path)
    names = Names(graph, run)
    order = [graph.root] + list(range(graph.root + 1, graph.nodes)) + list(range(graph.blocks))
    position = array.array("I", bytes(4 * graph.nodes))
    for index, node in enumerate(order):
        position[node] = index
    strings, interned = [], {}

    def string(text):
        if text not in interned:
            interned[text] = len(strings)
            strings.append(text)
        return interned[text]

    node_fields = 7
    meta = {"node_fields": ["type", "name", "id", "self_size", "edge_count", "trace_node_id", "detachedness"],
            "node_types": [["hidden", "array", "string", "object", "code", "closure", "regexp", "number",
                            "native", "synthetic", "concatenated string", "sliced string", "symbol",
                            "bigint", "object shape"], "string", "number", "number", "number", "number", "number"],
            "edge_fields": ["type", "name_or_index", "to_node"],
            "edge_types": [["context", "element", "property", "internal", "hidden", "shortcut", "weak"],
                           "string_or_number", "node"],
            "trace_function_info_fields": ["function_id", "name", "script_name", "script_id", "line", "column"],
            "trace_node_fields": ["id", "function_info_index", "count", "size", "children"],
            "sample_fields": ["timestamp_us", "last_assigned_id"],
            "location_fields": ["object_index", "script_id", "line", "column"]}
    with open(path, "w", encoding="utf-8") as handle:
        head = {"meta": meta, "node_count": graph.nodes, "edge_count": graph.edge_count, "trace_function_count": 0}
        handle.write('{"snapshot":' + json.dumps(head) + ',\n"nodes":[')
        for index, node in enumerate(order):
            kind = 9 if node >= graph.blocks else 3  # synthetic | object
            edges = graph.edge_start[node + 1] - graph.edge_start[node]
            name = string(names.label(node) if node >= graph.blocks else names.group(node))
            handle.write(("," if index else "") +
                         f"{kind},{name},{2 * index + 1},{graph.self_bytes(node)},{edges},0,0")
        handle.write("],\n\"edges\":[")
        first = True
        for node in order:
            for edge in range(graph.edge_start[node], graph.edge_start[node + 1]):
                target = graph.edge_to[edge]
                label = string(f"+0x{graph.edge_offset[edge]:x}")
                handle.write(("" if first else ",") + f"2,{label},{position[target] * node_fields}")
                first = False
        handle.write("],\n\"trace_function_infos\":[],\"trace_tree\":[],\"samples\":[],\"locations\":[],\n\"strings\":")
        handle.write(json.dumps(strings))
        handle.write("}\n")
    return path
