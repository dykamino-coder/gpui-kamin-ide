"""Unit tests of the English-only rule in check_conventions.py."""

from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parent))
import check_conventions as conventions


WORD = "".join(map(chr, (0x0442, 0x0435, 0x043A, 0x0441, 0x0442)))


class EnglishOnlyTests(unittest.TestCase):
    def test_cyrillic_in_engine_paths_is_reported(self):
        text = "// " + WORD
        for path in ("crates/html/src/layout/mod.rs", "crates/html-style/src/lib.rs",
                     "vendor/gpui/src/scene.rs", "vendor/taffy/src/lib.rs",
                     "scripts/memprof.py", "scripts/wpt_context.py", "docs/memprof.md"):
            self.assertTrue(conventions.cyrillic_added(path, text), path)

    def test_english_text_and_other_paths_pass(self):
        self.assertFalse(conventions.cyrillic_added("crates/html/src/lib.rs", "// layout text"))
        for path in ("crates/shell/src/main.rs", "extensions/claude-bridge/server/src/a.ts",
                     "vendor/gpui-component/src/lib.rs", "vendor/gpui-kaminide.patch",
                     "scripts/check_event_routing.py", "docs/guide.md"):
            self.assertFalse(conventions.cyrillic_added(path, WORD), path)


if __name__ == "__main__":
    unittest.main()
