"""Run with python -I scripts/test_page_reduce.py (stdlib only)."""

from pathlib import Path
import os
import struct
import sys
import tempfile
import unittest
from unittest.mock import patch
from types import SimpleNamespace
import zlib

sys.path.insert(0, str(Path(__file__).resolve().parent))
from page_reduce_lib.css import Stylesheet, edits, split_ranges
from page_reduce_lib.html import Document, apply, attribute_edits
from page_reduce_lib.png import different, read
from page_reduce_lib.process import run
from page_reduce_lib.reduce import Reducer
from page_reduce_lib.resources import prepare
from page_reduce_lib.predicates import Oracle


class ParserTests(unittest.TestCase):
    def test_html_roundtrip_and_ranges(self):
        text = '''<!DOCTYPE HTML>\n<!-- x --><DIV id='a' CLASS="b c">&amp;&#38;<br/> x<style>a:before{content:"<b>"}</style></DIV>'''
        doc = Document(text)
        self.assertEqual(doc.serialize(), text)
        div, br, style = doc.nodes()
        self.assertEqual(text[div.start:div.end], text[text.index("<DIV"):])
        self.assertEqual(text[br.start:br.end], "<br/>")
        self.assertEqual(text[style.open_end:style.close_start], 'a:before{content:"<b>"}')
        self.assertTrue(any('CLASS="b"' in change[2] for change in attribute_edits(text, div)))

    def test_unclosed_and_unmatched_tags_do_not_disappear(self):
        text = "</oops><div><span>x</div>tail"
        doc = Document(text)
        self.assertEqual(doc.serialize(), text)
        self.assertEqual(text[doc.nodes()[1].start:doc.nodes()[1].end], "<span>x")

    def test_css_nested_strings_comments_functions_and_custom_properties(self):
        source = '''/*{*/@media (min-width:1px){a:is(.a,.b), b{content:"};,";background:url("data:x;,{}");--x:{a:b;c:d};color:red}} unfinished{'''
        sheet = Stylesheet(source)
        self.assertEqual(sheet.serialize(), source)
        self.assertEqual(len(sheet.rules), 1)
        child = sheet.rules[0].children[0]
        body = source[child.opening + 1:child.closing]
        self.assertEqual(len(list(split_ranges(body))), 4)
        selectors = list(edits(source, "selectors"))[0]
        self.assertEqual(len(selectors), 2)
        self.assertIn("a:is(.a,.b)", apply(source, [selectors[1]]))
        self.assertNotIn(", b{", apply(source, [selectors[1]]))

    def test_overlap_is_rejected(self):
        with self.assertRaises(ValueError):
            apply("abcdef", [(0, 3, ""), (2, 4, "")])

    def test_local_css_imports_resources_and_media(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            (root / "css").mkdir()
            (root / "css/base.css").write_text('@import "part.css" screen; a{background:url(../x.png);content:"url(no)"}', encoding="utf-8")
            (root / "css/part.css").write_text('b{color:red}', encoding="utf-8")
            source = '<link rel="stylesheet" href="css/base.css" media="print"><img src="x.png">'
            result = prepare(source, root / "page.html", True)
            self.assertIn("@media print{", result)
            self.assertIn("@media screen{b{color:red}}", result)
            self.assertIn((root / "x.png").resolve().as_uri(), result)
            self.assertIn('content:"url(no)"', result)
            self.assertNotIn("<link", result)

    def test_missing_stylesheet_is_not_silently_discarded(self):
        with tempfile.TemporaryDirectory() as temp:
            with self.assertRaises(FileNotFoundError):
                prepare('<link rel=stylesheet href="missing.css">', Path(temp) / "p.html", True)

    def test_import_comments_spaces_cycles_and_unsupported_selection(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            (root / "a.css").write_text('/* header */ @import "with space.css"; a{color:red}')
            (root / "with space.css").write_text('@import "a.css"; b{color:blue}')
            source = '<style>/* before */ @import url("a.css");</style>'
            result = prepare(source, root / "p.html", True)
            self.assertNotIn("@import", result)
            self.assertEqual(result.count("a{color:red}"), 1)
            self.assertEqual(result.count("b{color:blue}"), 1)
            for link in ('<link rel="alternate stylesheet" href="a.css">',
                         '<link rel=stylesheet disabled href="a.css">'):
                with self.assertRaisesRegex(ValueError, "explicit selection"):
                    prepare(link, root / "p.html", True)


class ReductionTests(unittest.TestCase):
    def test_fixed_point_keeps_dom_css_dependency_and_rechecks(self):
        def predicate(path):
            source = path.read_text(encoding="utf-8")
            doc = Document(source)
            return any(node.tag == "aside" for node in doc.nodes()) and any(
                'float:left' in source[node.open_end:node.close_start]
                and 'aside' in source[node.open_end:node.close_start]
                for node in doc.nodes() if node.tag == "style")
        source = '<!doctype html><div><p>noise</p><aside id="unused">words here</aside></div><style>p{color:red}aside, .unused{float:left;color:blue}</style>'
        with tempfile.TemporaryDirectory() as temp:
            reducer = Reducer(predicate, temp, jobs=3)
            try:
                result = reducer.run(source)
                self.assertIn("<aside", result)
                self.assertIn("float:left", result)
                self.assertNotIn("noise", result)
                self.assertNotIn("color:", result)
                self.assertNotIn("unused", result)
                self.assertLess(len(result), len(source) * .6)
                calls = reducer.runs
                self.assertTrue(reducer.evaluate(result, "cached"))
                self.assertEqual(reducer.runs, calls)
                self.assertGreater(reducer.hits, 0)
            finally:
                reducer.close()

    def test_unresolved_is_never_accepted(self):
        source = '<div>required</div>'
        with tempfile.TemporaryDirectory() as temp:
            reducer = Reducer(lambda path: True if path.read_text() == source else None, temp)
            try:
                self.assertEqual(reducer.run(source), source)
            finally:
                reducer.close()

    def test_initial_false_fails(self):
        with tempfile.TemporaryDirectory() as temp:
            reducer = Reducer(lambda path: False, temp)
            try:
                with self.assertRaisesRegex(ValueError, "initial page"):
                    reducer.run("<p>x</p>")
            finally:
                reducer.close()

    def test_timeout_returns_unresolved_and_records_owned_pid(self):
        with tempfile.TemporaryDirectory() as temp:
            result, _, _ = run([sys.executable, "-I", "-c", "import time; time.sleep(10)"], temp, .1)
            self.assertIsNone(result)
            self.assertIn('"pid":', (Path(temp) / "owned-process.json").read_text())

    def test_completed_capture_reaps_a_process_that_keeps_running(self):
        with tempfile.TemporaryDirectory() as temp:
            capture = Path(temp) / "capture"
            code = "from pathlib import Path; import time; Path('capture').write_bytes(b'done'); time.sleep(30)"
            result, _, _ = run([sys.executable, "-I", "-c", code], temp, 3,
                               ready=lambda: capture.is_file() and capture.read_bytes() == b"done", ready_grace=.05)
            self.assertEqual(result, 0)

    @unittest.skipUnless(os.name == "nt", "Windows job ownership")
    def test_timeout_reaps_descendant_and_releases_inherited_files(self):
        import ctypes
        code = ("import subprocess,sys,time; "
                "p=subprocess.Popen([sys.executable,'-I','-c','import time;time.sleep(30)']); "
                "print(p.pid,flush=True);time.sleep(30)")
        with tempfile.TemporaryDirectory() as temp:
            result, output, _ = run([sys.executable, "-I", "-c", code], temp, .5)
            self.assertIsNone(result)
            pid = int(output.strip())
            kernel = ctypes.WinDLL("kernel32", use_last_error=True)
            kernel.OpenProcess.argtypes = [ctypes.c_uint32, ctypes.c_int, ctypes.c_uint32]
            kernel.OpenProcess.restype = ctypes.c_void_p
            kernel.WaitForSingleObject.argtypes = [ctypes.c_void_p, ctypes.c_uint32]
            kernel.CloseHandle.argtypes = [ctypes.c_void_p]
            handle = kernel.OpenProcess(0x100000, False, pid)
            if handle:
                try:
                    self.assertEqual(kernel.WaitForSingleObject(handle, 0), 0)
                finally:
                    kernel.CloseHandle(handle)


def png(path, color, rgba=False):
    def chunk(kind, body):
        return struct.pack(">I", len(body)) + kind + body + struct.pack(">I", zlib.crc32(kind + body))
    path.write_bytes(b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", 1, 1, 8, 6 if rgba else 2, 0, 0, 0))
                     + chunk(b"IDAT", zlib.compress(b"\0" + bytes(color))) + chunk(b"IEND", b""))


class PngTests(unittest.TestCase):
    def test_rgb_tolerance_alpha_and_bounds(self):
        with tempfile.TemporaryDirectory() as temp:
            a, b = Path(temp) / "a.png", Path(temp) / "b.png"
            png(a, [255, 255, 255])
            png(b, [0, 0, 0, 0], True)
            self.assertFalse(different(a, b, (0, 0, 1, 1)))
            png(b, [254, 255, 255])
            self.assertTrue(different(a, b, (0, 0, 1, 1)))
            self.assertFalse(different(a, b, (0, 0, 1, 1), 1))
            with self.assertRaises(ValueError):
                different(a, b, (1, 0, 1, 1))
            data = b.read_bytes()
            b.write_bytes(data[:-8])
            with self.assertRaises(ValueError):
                read(b)


class OracleTests(unittest.TestCase):
    def test_rgb_uses_hidden_launcher_for_both_renderers(self):
        with tempfile.TemporaryDirectory() as temp:
            page = Path(temp) / "p.html"
            page.write_text("<p>x</p>")
            args = SimpleNamespace(launcher="python hidden.py", runner="page", exe="engine.exe",
                                   predicate="rgb", timeout_s=3, exe_arg=[], require_regex=[],
                                   engine_png=None, chrome="chrome.exe", width=1, height=1,
                                   rect=(0, 0, 1, 1), tol=0)
            oracle = Oracle(args)
            commands = []
            def capture(command, directory, timeout, *extra, **kwargs):
                commands.append(command)
                self.assertEqual(command[:2], ["python", "hidden.py"])
                if command[2] == "engine.exe":
                    shots = page.parent / "target/wpt-shots"
                    shots.mkdir(parents=True)
                    png(shots / "p.png", [255, 0, 0])
                else:
                    self.assertEqual(command[2], "chrome.exe")
                    dest = next(arg.split("=", 1)[1] for arg in command if arg.startswith("--screenshot="))
                    png(Path(dest), [0, 0, 255])
                    self.assertTrue(kwargs["ready"]())
                return 0, b"", b""
            with patch("page_reduce_lib.predicates.run", side_effect=capture):
                self.assertTrue(oracle(page))
            self.assertEqual(len(commands), 2)

    def test_memory_delegates_to_canonical_tool_and_uses_peak(self):
        with tempfile.TemporaryDirectory() as temp:
            page = Path(temp) / "p.html"
            page.write_text("<p>x</p>")
            args = SimpleNamespace(launcher="python hidden.py", runner="page", exe="engine.exe",
                                   predicate="memory", timeout_s=3, exe_arg=["--json"], ws_mb=10,
                                   require_regex=[])
            oracle = Oracle(args)
            def measurement(command, directory, timeout):
                self.assertIn("-I", command)
                self.assertTrue(command[2].endswith("memprof.py"))
                self.assertIn("ws", command)
                self.assertEqual(command[-3:], ["--", str(page), "--json"])
                out = Path(directory) / "ws"
                out.mkdir()
                (out / "ws.json").write_text('{"cand":{"working_set":1,"process_peak_working_set":11534336}}')
                return 0, b"", b""
            with patch("page_reduce_lib.predicates.run", side_effect=measurement):
                self.assertTrue(oracle(page))

    def test_panic_timeout_missing_png_and_feature_guard(self):
        with tempfile.TemporaryDirectory() as temp:
            page = Path(temp) / "p.html"
            page.write_text("<p>x</p>")
            args = SimpleNamespace(launcher="python hidden.py", runner="page", exe="engine.exe",
                                   predicate="panic", match="needle", require_regex=[])
            oracle = Oracle(args)
            with patch.object(oracle, "engine", return_value=(None, b"", b"needle")):
                self.assertIsNone(oracle(page))
            with patch.object(oracle, "engine", return_value=(1, b"", b"needle")):
                self.assertTrue(oracle(page))
            args.predicate, args.engine_png = "rgb", None
            with patch.object(oracle, "engine", return_value=(0, b"", b"")):
                self.assertIsNone(oracle(page))
            args.require_regex = ["<aside"]
            with patch.object(oracle, "engine", side_effect=AssertionError("must not launch")):
                self.assertFalse(oracle(page))


if __name__ == "__main__":
    unittest.main()
