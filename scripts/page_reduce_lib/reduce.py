"""Deterministic parallel hierarchical delta debugging with three-valued oracles."""

from concurrent.futures import ThreadPoolExecutor
import hashlib
import json
from pathlib import Path
import threading
import time

from . import css
from .html import Document, apply, attribute_edits


def groups(source, stage):
    doc = Document(source)
    if stage in ("dom", "unwrap"):
        for level in doc.levels():
            edits = []
            for node in level:
                if stage == "dom":
                    edits.append((node.start, node.end, ""))
                elif node.open_end < node.close_start:
                    edits.append((node.start, node.end, source[node.open_end:node.close_start]))
            if edits:
                yield edits
    elif stage.startswith("css-"):
        for node in doc.nodes():
            if node.tag == "style":
                body = source[node.open_end:node.close_start]
                local = ([list(css.trivia_edits(body))] if stage == "css-trivia"
                         else css.edits(body, stage[4:]))
                for edits in local:
                    yield [(a + node.open_end, b + node.open_end, c) for a, b, c in edits]
    elif stage == "attributes":
        # Whole attributes are disjoint; alternate class-token edits are separate.
        whole, tokens = [], []
        for node in doc.nodes():
            for edit in attribute_edits(source, node):
                (whole if not edit[2] else tokens).append(edit)
        yield whole
        for edit in tokens:
            yield [edit]
    elif stage == "text":
        edits = list(doc.comments)
        for start, end, tag in doc.text:
            if tag not in ("style", "script"):
                words = source[start:end].split()
                replacement = words[0] if words else ""
                if replacement != source[start:end]:
                    edits.append((start, end, replacement))
        yield edits


STAGES = ("dom", "unwrap", "css-rules", "css-unwrap", "css-declarations",
          "css-selectors", "attributes", "text", "css-trivia")


class Reducer:
    def __init__(self, predicate, out, jobs=3, max_seconds=None):
        self.predicate, self.out, self.jobs = predicate, Path(out), jobs
        self.out.mkdir(parents=True, exist_ok=True)
        self.cache, self.lock = {}, threading.Lock()
        self.runs = self.hits = 0
        self.started = time.monotonic()
        self.deadline = self.started + max_seconds if max_seconds else float("inf")
        self.log = (self.out / "log.jsonl").open("a", encoding="utf-8")
        self.pool = ThreadPoolExecutor(max_workers=jobs)

    def close(self):
        self.pool.shutdown(wait=True)
        self.log.close()

    def evaluate(self, source, step, fresh=False):
        digest = hashlib.sha256(source.encode("utf-8")).hexdigest()
        # Register an event before leaving the lock: identical concurrent variants
        # share one run, including unresolved results.
        with self.lock:
            owner = False
            existing = None if fresh else self.cache.get(digest)
            if existing:
                self.hits += 1
            else:
                existing = [threading.Event(), None]
                self.cache[digest] = existing
                self.runs += 1
                number = self.runs
                owner = True
            if existing[0].is_set():
                return existing[1]
        if not owner:
            existing[0].wait()
            return existing[1]
        directory = self.out / "runs" / f"{number:06d}"
        directory.mkdir(parents=True)
        page = directory / "page.html"
        page.write_text(source, encoding="utf-8", newline="")
        start = time.monotonic()
        error = None
        try:
            verdict = self.predicate(page) if start < self.deadline or fresh else None
            if verdict is not None and type(verdict) is not bool:
                raise TypeError("predicate must return bool or None")
        except Exception as exc:
            verdict, error = None, f"{type(exc).__name__}: {exc}"
        record = dict(step=step, size=len(source.encode("utf-8")), verdict=verdict,
                      seconds=round(time.monotonic() - start, 4), sha256=digest,
                      run=number, error=error)
        with self.lock:
            self.log.write(json.dumps(record, ensure_ascii=False) + "\n")
            self.log.flush()
            existing[1] = verdict
            existing[0].set()
        return verdict

    def candidates(self, source, edits):
        # ddmin complements, from large chunks down to single source edits.
        count = len(edits)
        parts = min(2, count)
        seen = set()
        while parts:
            chunk = (count + parts - 1) // parts
            for start in range(0, count, chunk):
                try:
                    candidate = apply(source, edits[start:start + chunk])
                except ValueError:
                    continue
                digest = hashlib.sha256(candidate.encode("utf-8")).digest()
                if len(candidate.encode("utf-8")) < len(source.encode("utf-8")) and digest not in seen:
                    seen.add(digest)
                    yield candidate
            if parts == count:
                break
            parts = min(count, parts * 2)

    def attempt(self, source, edits, stage):
        iterator = iter(self.candidates(source, edits))
        while time.monotonic() < self.deadline:
            batch = []
            for _ in range(self.jobs):
                candidate = next(iterator, None)
                if candidate is None:
                    break
                batch.append(candidate)
            if not batch:
                break
            futures = [self.pool.submit(self.evaluate, text, stage) for text in batch]
            verdicts = [future.result() for future in futures]
            for text, verdict in zip(batch, verdicts):
                if verdict is True:
                    return text
        return source

    def run(self, source):
        if self.evaluate(source, "initial") is not True:
            raise ValueError("initial page did not reproduce; inspect log.jsonl and runs/000001")
        self.save(source)
        while time.monotonic() < self.deadline:
            before = source
            for stage in STAGES:
                while time.monotonic() < self.deadline:
                    reduced = source
                    for edits in groups(source, stage):
                        reduced = self.attempt(source, edits, stage)
                        if reduced != source:
                            break
                    if reduced == source:
                        break
                    source = reduced
                    self.save(source)
            if source == before:
                break
        if self.evaluate(source, "final-recheck", fresh=True) is not True:
            raise ValueError("final page failed fresh reproduction; result is not verified")
        return source

    def save(self, source):
        temporary = self.out / "min.next.html"
        temporary.write_text(source, encoding="utf-8", newline="")
        temporary.replace(self.out / "min.html")
