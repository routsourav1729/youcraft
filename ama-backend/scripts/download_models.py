"""Pinned model packs, resumable Hub cache and atomic desktop progress reports."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import threading
import time

MODELS = {
    'indic-core': ('bodhan-ai/indic-transcribe-core', 'e083ab7dfeeddcb5c0b4a2d7df53f53e7b0f1537'),
    'nemotron': ('nvidia/Nemotron-3-Diarization', 'f667ed73aee57d40cc39428eb768b4fd87a0a29e'),
}
LABELS = {'indic-core': 'Odia model', 'nemotron': 'Speaker model'}


def atomic_json(path, value):
    temporary = path.with_suffix('.tmp')
    temporary.write_text(json.dumps(value, indent=2), encoding='utf-8')
    # Antivirus or a Windows reader can briefly hold the destination open.
    for attempt in range(20):
        try:
            temporary.replace(path)
            return
        except PermissionError:
            if attempt == 19:
                raise
            time.sleep(.05)


def read_json(path):
    try:
        return json.loads(path.read_text(encoding='utf-8'))
    except (OSError, ValueError):
        return {}


def installed(entry, repo, revision):
    if entry.get('repo') != repo or entry.get('revision') != revision or not entry.get('files'):
        return False
    root = Path(entry['path'])
    try:
        return all((root / name).stat().st_size == record['size']
                   and (root / name).stat().st_mtime_ns == record['mtime_ns']
                   for name, record in entry['files'].items())
    except (OSError, KeyError):
        return False


class Progress:
    def __init__(self, path, label):
        self.path, self.label = path, label
        self.lock = threading.RLock()
        self.stop = threading.Event()
        self.value = dict(stage=f'Checking {label.lower()}', state='running', error='')
        self.changed = self.sample_time = time.monotonic()
        self.sample_bytes = 0
        self.thread = threading.Thread(target=self._heartbeat, daemon=True)

    def __enter__(self):
        self.publish()
        self.thread.start()
        return self

    def __exit__(self, *_):
        self.stop.set()
        self.thread.join()

    def update(self, **values):
        with self.lock:
            if values.get('downloaded_bytes', 0) != self.value.get('downloaded_bytes', 0):
                self.changed = time.monotonic()
            self.value.update(values)

    def publish(self):
        with self.lock:
            value = dict(self.value)
            now = time.monotonic()
            count = value.get('downloaded_bytes', 0)
            value['bytes_per_second'] = max(0, count - self.sample_bytes) / max(.001, now - self.sample_time)
            self.sample_bytes, self.sample_time = count, now
            value['idle_seconds'] = int(now - self.changed)
            value['updated_at'] = time.time()
            if self.path:
                atomic_json(self.path, value)

    def _heartbeat(self):
        while not self.stop.wait(1):
            self.publish()


def download_pack(directory, name, progress, api=None, fetch=None):
    from huggingface_hub import HfApi, hf_hub_download
    from tqdm.auto import tqdm
    api = api or HfApi()
    fetch = fetch or hf_hub_download
    repo, revision = MODELS[name]
    manifest_path = directory / 'manifest.json'
    manifest = read_json(manifest_path)
    if installed(manifest.get(name, {}), repo, revision):
        progress.update(stage=f'{LABELS[name]} ready', state='succeeded')
        progress.publish()
        return
    # Metadata establishes the total before downloading, and validates gated access.
    info = api.model_info(repo, revision=revision, files_metadata=True, timeout=30)
    files = [f for f in info.siblings if not f.rfilename.startswith('nemo/') and not f.rfilename.endswith(('.nemo', '.gguf', '.png', '.gif', '.mp4'))]
    if not files or any(f.size is None for f in files):
        raise ValueError('Model file list is incomplete. Retry setup.')
    cache = directory / 'hf-cache'
    snapshot = cache / ('models--' + repo.replace('/', '--')) / 'snapshots' / revision
    total = sum(f.size for f in files)
    needed = sum(f.size for f in files if not (snapshot / f.rfilename).is_file())
    if shutil.disk_usage(directory).free < needed + 256 * 1024**2:
        raise OSError(28, f'Not enough disk space. Free at least {needed / 1024**3 + .25:.1f} GB.')
    completed, records = 0, {}
    for item in files:
        name_in_repo = item.rfilename
        relative = Path(name_in_repo)
        if relative.is_absolute() or '..' in relative.parts:
            raise ValueError('Unsafe model file path')
        progress.update(stage=f'Downloading {LABELS[name].lower()}', file=name_in_repo,
                        downloaded_bytes=completed, total_bytes=total)
        progress.publish()

        class DownloadBar(tqdm):
            def __init__(self, *args, **kwargs):
                kwargs['disable'] = False
                kwargs.pop('name', None)
                super().__init__(*args, **kwargs)
                progress.update(downloaded_bytes=completed + self.n)

            def display(self, *args, **kwargs):
                pass  # GUI owns progress; do not fill setup.log with terminal bars.

            def update(self, n=1):
                result = super().update(n)
                progress.update(downloaded_bytes=completed + self.n)
                return result

        options = dict(repo_id=repo, filename=name_in_repo, revision=revision,
                       cache_dir=str(cache), tqdm_class=DownloadBar)
        path = Path(fetch(**options))
        for attempt in range(2):
            progress.update(stage=f'Verifying {LABELS[name].lower()}', downloaded_bytes=completed + item.size)
            valid = path.stat().st_size == item.size
            sha = (item.lfs or {}).get('sha256') if isinstance(item.lfs, dict) else getattr(item.lfs, 'sha256', None)
            if valid and sha:
                with path.open('rb') as stream:
                    valid = hashlib.file_digest(stream, 'sha256').hexdigest() == sha
            if valid:
                break
            if attempt:
                raise ValueError('Model file verification failed. Retry setup to replace the damaged file.')
            progress.update(stage=f'Repairing {LABELS[name].lower()}')
            path = Path(fetch(**options, force_download=True))
        records[name_in_repo] = dict(size=item.size, mtime_ns=path.stat().st_mtime_ns)
        completed += item.size
    manifest[name] = dict(repo=repo, revision=revision, path=str(snapshot), files=records)
    atomic_json(manifest_path, manifest)
    progress.update(stage=f'{LABELS[name]} ready', state='succeeded', downloaded_bytes=total)
    progress.publish()


def error_message(error):
    status = getattr(getattr(error, 'response', None), 'status_code', None)
    if status in (401, 403):
        return 'Model access denied. Open Odia model access, accept its terms, and use a read token from that same account.'
    if isinstance(error, OSError) and error.errno == 28:
        return 'Not enough disk space. Free space on the app-data drive and retry.'
    if isinstance(error, ValueError):
        return str(error)
    return f'Download failed ({type(error).__name__}). Check your connection and retry. Completed files are kept; partial downloads resume.'


def main():
    # Use the Hub HTTP downloader: resumable partial files, bounded socket timeouts,
    # and byte callbacks instead of an opaque Xet transfer/reconstruction stage.
    os.environ['HF_HUB_DISABLE_XET'] = '1'
    os.environ['HF_HUB_DOWNLOAD_TIMEOUT'] = '30'
    os.environ['HF_HUB_ETAG_TIMEOUT'] = '15'
    parser = argparse.ArgumentParser()
    parser.add_argument('--directory', type=Path, default=Path(__file__).resolve().parents[1] / 'models')
    parser.add_argument('--pack', choices=[*MODELS, 'all'], default='all')
    parser.add_argument('--status-file', type=Path)
    args = parser.parse_args()
    args.directory.mkdir(parents=True, exist_ok=True)
    for name in MODELS:
        if args.pack not in ('all', name):
            continue
        with Progress(args.status_file, LABELS[name]) as progress:
            try:
                download_pack(args.directory, name, progress)
            except Exception as error:
                message = error_message(error)
                progress.update(stage='Setup failed', state='failed', error=message)
                progress.publish()
                print(message, flush=True)  # No signed URLs or credentials in logs.
                return 1
            print(f'{LABELS[name]} ready', flush=True)
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
