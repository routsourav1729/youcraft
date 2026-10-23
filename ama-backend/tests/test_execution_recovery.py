"""Recovery regressions requiring neither model weights nor a GPU."""
import json
import sqlite3
import sys
import time
from pathlib import Path
from types import SimpleNamespace

import pytest

from betun import providers
from betun.jobs import Queue
from betun.media import Cancelled
from betun.schema import Project
from betun.store import Store


def configure(monkeypatch, device='cuda'):
    monkeypatch.setattr(providers, 'configuration', lambda: {'diarize': dict(
        provider='fake', revision='test', device=device,
        allow_cpu_fallback=True, command=['worker'])})


@pytest.mark.parametrize('device,error', [
    ('cpu', 'OutOfMemoryError: CPU allocation failed'),
    ('auto', 'OutOfMemoryError: CPU allocation failed'),
    ('cuda', 'OutOfMemoryError: unrelated allocator failed'),
])
def test_non_cuda_oom_is_not_retried(tmp_path, monkeypatch, device, error):
    configure(monkeypatch, device)
    calls = []
    def run(*args, **kwargs):
        calls.append(args)
        raise RuntimeError(error)
    monkeypatch.setattr(providers, 'run', run)
    with pytest.raises(RuntimeError, match='OutOfMemoryError'):
        providers.invoke('diarize', {}, tmp_path)
    assert len(calls) == 1


def test_gpu_failure_does_not_cache_partial_output_or_fallback(tmp_path, monkeypatch):
    configure(monkeypatch)
    devices = []
    def run(args, *unused, **kwargs):
        request = json.loads(Path(args[-2]).read_text())
        devices.append(request['config']['device'])
        if devices == ['cuda']:
            Path(args[-1]).write_text('{"activity": [{"invalid": true}]}')
            raise RuntimeError('CUDA out of memory')
        assert not Path(args[-1]).exists()
        # Broken provider exits successfully without producing a result.
    monkeypatch.setattr(providers, 'run', run)
    with pytest.raises(RuntimeError, match='GPU memory is full'):
        providers.invoke('diarize', {}, tmp_path)
    assert devices == ['cuda']
    assert not list(tmp_path.glob('model-*.json'))


def test_cancel_after_gpu_failure_prevents_cpu_launch(tmp_path, monkeypatch):
    configure(monkeypatch)
    cancelled = False
    calls = []
    def run(*args, **kwargs):
        nonlocal cancelled
        calls.append(args)
        cancelled = True
        raise RuntimeError('CUDA out of memory')
    monkeypatch.setattr(providers, 'run', run)
    with pytest.raises(Cancelled):
        providers.invoke('diarize', {}, tmp_path, cancel=lambda: cancelled)
    assert len(calls) == 1


def test_cancelled_cache_hit_does_not_return_results(tmp_path, monkeypatch):
    configure(monkeypatch)
    def run(args, *unused, **kwargs):
        Path(args[-1]).write_text('{"activity": []}')
    monkeypatch.setattr(providers, 'run', run)
    providers.invoke('diarize', {}, tmp_path)
    with pytest.raises(Cancelled):
        providers.invoke('diarize', {}, tmp_path, cancel=lambda: True)


def test_gpu_selection_on_cpu_runtime_is_actionable(tmp_path, monkeypatch):
    from betun.model_worker import main
    monkeypatch.setitem(sys.modules, 'torch', SimpleNamespace(
        set_num_threads=lambda _: None, cuda=SimpleNamespace(is_available=lambda: False)))
    request = tmp_path / 'request.json'
    request.write_text(json.dumps(dict(task='diarize', payload={}, config=dict(
        provider='fake', model_path=str(tmp_path), device='cuda'))))
    with pytest.raises(ValueError, match='Choose CPU or install the NVIDIA runtime'):
        main(request, tmp_path / 'result.json')


@pytest.mark.parametrize('fail', [False, True])
def test_database_connection_is_closed_on_all_exits(tmp_path, fail):
    store = Store(tmp_path)
    try:
        with store.connect() as connection:
            connection.execute('SELECT 1')
            if fail:
                raise ValueError('rollback')
    except ValueError:
        pass
    with pytest.raises(sqlite3.ProgrammingError, match='closed'):
        connection.execute('SELECT 1')


def wait_job(queue, job):
    deadline = time.monotonic() + 10
    while time.monotonic() < deadline:
        current = queue.get(job['id'])
        if current['state'] not in ('queued', 'running', 'cancelling'):
            return current
        time.sleep(.02)
    pytest.fail('Worker did not finish')


def test_stale_job_fails_before_inference_and_retry_uses_latest_project(tmp_path, monkeypatch):
    store = Store(tmp_path)
    project = store.create(Project(name='Before review'))
    queue = Queue(store)
    job = queue.submit(project.id, 'diarize', {})
    project.name = 'Reviewed'
    store.save(project, project.revision)
    calls = []
    def process(store, project, *args):
        calls.append((project.name, project.revision))
        return {'ok': True}
    monkeypatch.setattr('betun.jobs.process', process)
    queue.start()
    try:
        result = wait_job(queue, job)
        assert result['state'] == 'failed'
        assert 'Project changed while queued' in result['error']
        assert calls == []
        assert wait_job(queue, queue.retry(job['id']))['state'] == 'succeeded'
        assert calls == [('Reviewed', 1)]
        assert store.get(project.id).name == 'Reviewed'
    finally:
        queue.stop()
