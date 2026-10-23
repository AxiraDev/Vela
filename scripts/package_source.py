#!/usr/bin/env python3
"""Package original Vela sources and vendored Rust sources; exclude user data and Wine binaries."""
import argparse
from pathlib import Path
import zipfile


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--out", required=True, type=Path)
    args = parser.parse_args()
    project = Path(__file__).resolve().parents[1]
    excluded = {"build", "target", "Data", "Engines", ".git", "__pycache__"}
    args.out.resolve().parent.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(args.out, "w", zipfile.ZIP_DEFLATED, compresslevel=9) as archive:
        for path in sorted(project.rglob("*")):
            relative = path.relative_to(project)
            if any(part in excluded or part.endswith(".app") for part in relative.parts):
                continue
            if path.is_file() and not path.is_symlink() and path.name != ".DS_Store" and path.resolve() != args.out.resolve():
                archive.write(path, Path("vela") / relative)
    print(args.out.resolve())


if __name__ == "__main__":
    main()
