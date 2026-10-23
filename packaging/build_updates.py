"""Create immutable compressed file blobs for the Windows incremental updater."""
import gzip
import hashlib
import json
from pathlib import Path
import sys


def build(source, output, version):
    source, output = Path(source), Path(output)
    (output / 'files').mkdir(parents=True, exist_ok=True)
    entries = []
    for path in sorted(source.rglob('*')):
        if not path.is_file():
            continue
        data = path.read_bytes()
        digest = hashlib.sha256(data).hexdigest()
        compressed = gzip.compress(data, compresslevel=6, mtime=0)
        (output / 'files' / (digest + '.gz')).write_bytes(compressed)
        entries.append(dict(path=path.relative_to(source).as_posix(), size=len(data), sha256=digest,
                            download_size=len(compressed), download_sha256=hashlib.sha256(compressed).hexdigest()))
    (output / f'files-{version}.json').write_text(json.dumps(dict(version=version, files=entries), indent=2), encoding='utf-8')


if __name__ == '__main__':
    build(*sys.argv[1:])
