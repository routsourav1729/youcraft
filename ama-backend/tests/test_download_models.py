"""Small download fixtures: no tokens, GPU, or multi-GB model downloads."""
import hashlib
import importlib.util
import json
from pathlib import Path
import sys
from types import SimpleNamespace
import pytest

spec = importlib.util.spec_from_file_location('download_models', Path(__file__).resolve().parents[1] / 'scripts/download_models.py')
download = importlib.util.module_from_spec(spec)
spec.loader.exec_module(download)


@pytest.fixture
def hub(tmp_path, monkeypatch):
    # The downloader uses injected API/fetch implementations; dependency imports stay optional in CI.
    monkeypatch.setitem(sys.modules, 'huggingface_hub', SimpleNamespace(HfApi=object, hf_hub_download=None))
    data = {'config.json': b'{}', 'model.safetensors': b'0123456789'}
    items = [SimpleNamespace(rfilename=k, size=len(v), lfs={'sha256': hashlib.sha256(v).hexdigest()}) for k, v in data.items()]
    api = SimpleNamespace(model_info=lambda *a, **kw: SimpleNamespace(siblings=items))
    repo, revision = download.MODELS['indic-core']
    snapshot = tmp_path / 'hf-cache' / ('models--' + repo.replace('/', '--')) / 'snapshots' / revision
    snapshot.mkdir(parents=True)
    calls = []

    def fetch(filename, tqdm_class, force_download=False, **kw):
        calls.append((filename, force_download))
        target = snapshot / filename
        if target.exists() and not force_download:
            return str(target)
        with tqdm_class(total=len(data[filename]), initial=0) as bar:
            target.write_bytes(data[filename])
            bar.update(len(data[filename]))
        return str(target)
    return api, fetch, snapshot, calls, data


def test_progress_verified_install_then_offline_retry(tmp_path, hub):
    api, fetch, snapshot, calls, _ = hub
    status = tmp_path / 'status.json'
    with download.Progress(status, 'Odia model') as progress:
        download.download_pack(tmp_path, 'indic-core', progress, api, fetch)
        report = json.loads(status.read_text())
        assert report['downloaded_bytes'] == report['total_bytes'] == 12
        assert report['state'] == 'succeeded'
        calls.clear()
        api.model_info = lambda *a, **kw: pytest.fail('Completed pack must work offline')
        download.download_pack(tmp_path, 'indic-core', progress, api, fetch)
        assert calls == []


def test_interrupted_pack_not_marked_ready_and_retry_keeps_completed_file(tmp_path, hub):
    api, fetch, snapshot, calls, _ = hub
    status = tmp_path / 'status.json'
    def interrupted(**kw):
        if kw['filename'] == 'model.safetensors':
            raise TimeoutError('network stopped')
        return fetch(**kw)
    with download.Progress(status, 'Odia model') as progress:
        with pytest.raises(TimeoutError):
            download.download_pack(tmp_path, 'indic-core', progress, api, interrupted)
        assert not (tmp_path / 'manifest.json').exists()
        before = (snapshot / 'config.json').stat().st_mtime_ns
        download.download_pack(tmp_path, 'indic-core', progress, api, fetch)
        assert (snapshot / 'config.json').stat().st_mtime_ns == before
        assert json.loads((tmp_path / 'manifest.json').read_text())['indic-core']['files']


def test_corrupt_cached_weight_is_replaced_not_accepted(tmp_path, hub):
    api, fetch, snapshot, calls, data = hub
    (snapshot / 'model.safetensors').write_bytes(b'xxxxxxxxxx')
    with download.Progress(tmp_path / 'status.json', 'Odia model') as progress:
        download.download_pack(tmp_path, 'indic-core', progress, api, fetch)
    assert ('model.safetensors', True) in calls
    assert (snapshot / 'model.safetensors').read_bytes() == data['model.safetensors']


def test_errors_do_not_expose_credentials():
    error = RuntimeError('secret signed download URL')
    assert 'secret' not in download.error_message(error)
    error.response = SimpleNamespace(status_code=403)
    assert 'access denied' in download.error_message(error)
    assert 'disk space' in download.error_message(OSError(28, 'full'))


def test_real_hub_http_resume_uses_partial_bytes(tmp_path):
    """Exercise the actual Hub Range downloader against a tiny local HTTP server."""
    from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
    import threading
    from huggingface_hub.file_download import http_get
    payload = b'youcraft-download-fixture' * 100
    offsets = []

    class Handler(BaseHTTPRequestHandler):
        def do_GET(self):
            offset = int(self.headers.get('Range', 'bytes=0-').split('=')[1].split('-')[0])
            offsets.append(offset)
            self.send_response(206 if offset else 200)
            self.send_header('Content-Length', str(len(payload) - offset))
            if offset:
                self.send_header('Content-Range', f'bytes {offset}-{len(payload)-1}/{len(payload)}')
            self.end_headers()
            self.wfile.write(payload[offset:])

        def log_message(self, *_):
            pass

    server = ThreadingHTTPServer(('127.0.0.1', 0), Handler)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    partial = tmp_path / 'weight.incomplete'
    partial.write_bytes(payload[:81])
    try:
        with partial.open('ab') as stream:
            http_get(f'http://127.0.0.1:{server.server_port}/weight', stream,
                     resume_size=81, expected_size=len(payload))
        assert offsets == [81]
        assert partial.read_bytes() == payload
    finally:
        server.shutdown()
        server.server_close()
        thread.join()
