"""Publish only a self-consistent immutable release into the existing download site."""
import gzip
import hashlib
import json
import re
import shutil
import sys
from pathlib import Path


def publish(incremental, installer, site, version):
    if not re.fullmatch(r"\d+\.\d+\.\d+", version):
        raise ValueError("Invalid release version")
    incremental, installer, site = map(Path, (incremental, installer, site))
    name = f"files-{version}.json"
    manifest = json.loads((incremental / name).read_text(encoding="utf-8"))
    if manifest["version"] != version or not manifest["files"]:
        raise ValueError("Manifest version mismatch or empty release")
    if (site / name).exists():
        raise ValueError("This version is already published; bump youcraft-version.txt")
    for entry in manifest["files"]:
        digest = entry["sha256"]
        if not re.fullmatch(r"[a-f0-9]{64}", digest):
            raise ValueError("Invalid blob identifier")
        packed = incremental / "files" / f"{digest}.gz"
        data = packed.read_bytes()
        if len(data) != entry["download_size"] or hashlib.sha256(data).hexdigest() != entry["download_sha256"]:
            raise ValueError(f"Invalid compressed blob: {entry['path']}")
        expanded = gzip.decompress(data)
        if len(expanded) != entry["size"] or hashlib.sha256(expanded).hexdigest() != digest:
            raise ValueError(f"Invalid expanded blob: {entry['path']}")
    data = installer.read_bytes()
    if not data.startswith(b"MZ"):
        raise ValueError("Installer is not a Windows executable")
    digest = hashlib.sha256(data).hexdigest()
    shutil.copytree(incremental, site, dirs_exist_ok=True)
    shutil.copy2(installer, site / "YouCraft-Setup-x64.exe")
    (site / "SHA256SUMS.txt").write_text(f"{digest}  YouCraft-Setup-x64.exe\n", encoding="utf-8")
    (site / "update.json").write_text(json.dumps(dict(version=version, sha256=digest, size=len(data)), indent=2), encoding="utf-8")


if __name__ == "__main__":
    publish(*sys.argv[1:])
