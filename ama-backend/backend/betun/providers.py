"""Heavy dependencies run in a separate, user-configured Python environment."""
import hashlib
import json
import os
from pathlib import Path
import sys
from .media import run


def configuration():
    path = os.environ.get('BETUN_MODEL_CONFIG')
    return json.loads(Path(path).read_text()) if path else {}


def invoke(task, payload, directory, cancel=lambda: False, device='auto'):
    config = configuration().get(task)
    if not config:
        raise ValueError(f'No {task} provider configured; set BETUN_MODEL_CONFIG (see docs/models.md)')
    if device not in ('auto', 'cpu', 'cuda'):
        raise ValueError('Choose Auto, CPU or GPU')
    if device != 'auto':
        config = dict(config, device=device)
    if not config.get('revision'):
        raise ValueError('Provider configuration needs a revision for reproducible caching')
    digest = hashlib.sha256(json.dumps(dict(task=task, payload=payload, config=config), sort_keys=True).encode()).hexdigest()
    destination = directory / f'model-{digest}.json'
    if destination.exists():
        return json.loads(destination.read_text())
    request = directory / f'request-{digest}.json'
    partial = directory / f'model-{digest}.partial.json'
    request.write_text(json.dumps(dict(task=task, payload=payload, config=config)))
    # Commands are administrator configuration, never supplied through the HTTP API.
    command = config.get('command')
    if command is None:
        python = config.get('python', sys.executable)
        worker = Path(__file__).with_name('model_worker.py')
        if not worker.is_file():
            raise ValueError('Packaged app requires an external worker command; see docs/models.md')
        command = [python, str(worker)]
    if not isinstance(command, list) or not all(isinstance(x, str) for x in command) or not command:
        raise ValueError('Provider command must be a nonempty argument array')
    try:
        fallback = False
        try:
            run([*command, str(request), str(partial)], cancel, timeout=86400)
        except RuntimeError as exc:
            # The failed GPU process has been reaped by run() before this CPU retry starts.
            if not config.get('allow_cpu_fallback', False) or not any(x in str(exc) for x in ('CUDA out of memory', 'OutOfMemoryError')):
                raise
            fallback = True
            retry = dict(config, device='cpu')
            request.write_text(json.dumps(dict(task=task, payload=payload, config=retry)))
            run([*command, str(request), str(partial)], cancel, timeout=86400)
        result = json.loads(partial.read_text())
        if not isinstance(result, dict):
            raise ValueError('Worker result must be a JSON object')
        result['_execution'] = dict(worker_exited=True, cpu_fallback=fallback)
        partial.write_text(json.dumps(result, ensure_ascii=False), encoding='utf-8')
        partial.replace(destination)
        return result
    finally:
        request.unlink(missing_ok=True)
        partial.unlink(missing_ok=True)
