#!/usr/bin/env python3
"""Install the pinned official NSH CLI into the project's disposable output/tools directory."""

from __future__ import annotations

import argparse
import hashlib
from pathlib import Path
import platform
import re
import tarfile
import urllib.request
import zipfile

VERSION = "v0.3.0"
ROOT = Path(__file__).resolve().parents[1]


def download(url: str, target: Path) -> None:
    request = urllib.request.Request(url, headers={"User-Agent": "word-atlas-native-build"})
    with urllib.request.urlopen(request, timeout=60) as source, target.open("wb") as output:
        while chunk := source.read(1024 * 1024):
            output.write(chunk)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--directory", type=Path, default=ROOT / "output/tools")
    args = parser.parse_args()
    system, arch = platform.system(), platform.machine().lower()
    targets = {("Darwin", "arm64"): "macos-arm64.tar.gz", ("Linux", "x86_64"): "linux-x86_64.tar.gz",
               ("Windows", "amd64"): "windows-x86_64.zip", ("Windows", "x86_64"): "windows-x86_64.zip"}
    try:
        name = "nsh-" + targets[system, arch]
    except KeyError as error:
        raise SystemExit(f"No official prebuilt CLI for {system}/{arch}; build NSH {VERSION} from source.") from error
    directory = args.directory.resolve()
    directory.mkdir(parents=True, exist_ok=True)
    archive = directory / name
    checksum = directory / (name + ".sha256")
    base = f"https://github.com/longbridge/nsh/releases/download/{VERSION}/"
    download(base + name + ".sha256", checksum)
    expected = re.search(r"\b[a-fA-F0-9]{64}\b", checksum.read_text())
    if not expected:
        raise SystemExit("The official checksum file has an unsupported format.")
    if not archive.is_file() or hashlib.file_digest(archive.open("rb"), "sha256").hexdigest() != expected[0].lower():
        download(base + name, archive)
    with archive.open("rb") as source:
        if hashlib.file_digest(source, "sha256").hexdigest() != expected[0].lower():
            raise SystemExit("NSH checksum mismatch; the archive was not executed or extracted.")
    if name.endswith(".zip"):
        with zipfile.ZipFile(archive) as source:
            for item in source.infolist():
                if not (directory / item.filename).resolve().is_relative_to(directory):
                    raise SystemExit("Unsafe archive member")
            source.extractall(directory)
    else:
        with tarfile.open(archive) as source:
            source.extractall(directory, filter="data")
    print(directory / "nsh/bin" / ("nsh.exe" if system == "Windows" else "nsh"))


if __name__ == "__main__":
    main()
