#!/usr/bin/env python3
"""Verify that native distribution assets exactly match the full canonical media tree."""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import zipfile

ROOT = Path(__file__).resolve().parents[1]


def digest(stream) -> str:
    result = hashlib.sha256()
    for chunk in iter(lambda: stream.read(1024 * 1024), b""):
        result.update(chunk)
    return result.hexdigest()


def sources() -> list[tuple[str, Path]]:
    files = []
    for name in ("atlas", "brand", "sfx"):
        for path in sorted((ROOT / "public" / name).rglob("*")):
            if path.is_file():
                files.append((path.relative_to(ROOT / "public").as_posix(), path))
    for relative, path in (
        ("licenses/content.txt", ROOT / "CONTENT_LICENSE.md"),
        ("licenses/application.txt", ROOT / "LICENSE"),
        ("licenses/ts-fsrs.txt", ROOT / "native/LICENSES/ts-fsrs.txt"),
        ("licenses/nsh.txt", ROOT / "native/LICENSES/nsh.txt"),
        ("licenses/rusqlite.txt", ROOT / "native/LICENSES/rusqlite.txt"),
        ("licenses/sqlite.txt", ROOT / "native/LICENSES/sqlite.txt"),
    ):
        files.append((relative, path))
    return files


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("package", nargs="?", type=Path, help=".app, APK, IPA, or unpacked distribution")
    parser.add_argument("--report", type=Path, help="Write a JSON integrity report")
    args = parser.parse_args()
    package = args.package
    entries = sources()
    errors = []
    total = 0
    counts = {"illustrations": 0, "thumbnails": 0, "audio": 0, "other": 0}
    archive = None
    base = None
    prefix = ""
    if package:
        if package.is_file() and zipfile.is_zipfile(package):
            archive = zipfile.ZipFile(package)
            indexes = [name for name in archive.namelist() if name.endswith("assets/atlas/index.json")]
            if len(indexes) != 1:
                raise SystemExit("Expected exactly one packaged atlas index")
            prefix = indexes[0][: -len("atlas/index.json")]
        elif package.is_dir():
            indexes = list(package.rglob("assets/atlas/index.json"))
            if len(indexes) != 1:
                raise SystemExit("Expected exactly one packaged atlas index")
            base = indexes[0].parent.parent
        else:
            raise SystemExit(f"No native package at {package}")
    for relative, source in entries:
        if not source.is_file() or source.stat().st_size == 0:
            errors.append(f"Missing or empty source: {relative}")
            continue
        total += source.stat().st_size
        if relative.startswith("atlas/img/"):
            counts["illustrations"] += 1
        elif relative.startswith("atlas/thumb/"):
            counts["thumbnails"] += 1
        elif relative.startswith("atlas/audio/"):
            counts["audio"] += 1
        else:
            counts["other"] += 1
        if package:
            with source.open("rb") as original:
                expected = digest(original)
            try:
                if archive:
                    with archive.open(prefix + relative) as packed:
                        actual = digest(packed)
                else:
                    with (base / relative).open("rb") as packed:
                        actual = digest(packed)
            except (FileNotFoundError, KeyError):
                errors.append(f"Not packaged: {relative}")
                continue
            if actual != expected:
                errors.append(f"Content mismatch: {relative}")
    index = json.loads((ROOT / "public/atlas/index.json").read_text())
    for row in index["words"]:
        word_id, media = row[0], row[6]
        required = []
        if media & 1:
            required += [f"atlas/img/{word_id}.webp", f"atlas/thumb/{word_id}.webp"]
        for bit, kind in ((2, "uk"), (4, "us"), (8, "ex"), (16, "ex2")):
            if media & bit:
                required.append(f"atlas/audio/{kind}/{word_id}.mp3")
        for relative in required:
            if not (ROOT / "public" / relative).is_file():
                errors.append(f"Indexed medium missing: {relative}")
    report = {"words": index["count"], "resource_files": len(entries), "source_bytes": total, "media": counts,
              "package": str(package) if package else None, "comparison": "sha256" if package else "source-completeness", "errors": errors}
    if archive:
        archive.close()
    if args.report:
        args.report.parent.mkdir(parents=True, exist_ok=True)
        args.report.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n")
    print(json.dumps(report, ensure_ascii=False, indent=2))
    return bool(errors)


if __name__ == "__main__":
    raise SystemExit(main())
