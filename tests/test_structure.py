import tempfile
import unittest
from pathlib import Path

from tools.check_structure import (
    REQUIRED_DIRECTORIES,
    REQUIRED_FILES,
    check_structure,
)


class StructureCheckTests(unittest.TestCase):
    def make_scaffold(self, root: Path) -> None:
        for relative in REQUIRED_DIRECTORIES:
            (root / relative).mkdir(parents=True, exist_ok=True)
        for relative in REQUIRED_FILES:
            path = root / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text("# fixture\n", encoding="utf-8")

    def test_missing_required_file_is_reported(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.make_scaffold(root)
            (root / "core/README.md").unlink()

            errors = check_structure(root)

            self.assertIn("missing required file: core/README.md", errors)

    def test_broken_local_markdown_link_is_reported(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.make_scaffold(root)
            (root / "README.md").write_text(
                "[missing](docs/not-present.md)\n", encoding="utf-8"
            )

            errors = check_structure(root)

            self.assertIn(
                "README.md:1: missing local link target: docs/not-present.md",
                errors,
            )


if __name__ == "__main__":
    unittest.main()
