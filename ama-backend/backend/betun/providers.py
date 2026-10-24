"""Heavy dependencies run in a separate, user-configured Python environment."""
import hashlib
import json
import os
from pathlib import Path
import sys
import time
from .media import Cancelled, run


def configuration():
    path = os.environ.get('BETUN_MODEL_CONFIG')
    if not path and os.environ.get('LOCALAPPDATA'):
        base = Path(os.environ['LOCALAPPDATA'])
        path = next((p for p in (base / 'YouCraft/models.json', base / 'AmaBartalap/models.json') if p.is_file()), None)
    return json.loads(Path(path).read_text(encoding='utf-8')) if path else {}


def invoke(task, payload, directory, cancel=lambda: False, device='auto', progress=lambda _: None):
    if cancel():
        raise Cancelled('Job cancelled')
    config = configuration().get(task)
    if not config and task == 'transliterate':
        from indic_transliteration import sanscript
        text = payload.get('text', '')
        if not isinstance(text, str) or not text.strip() or len(text) > 10000:
            raise ValueError('Enter a word or phrase to convert')
        converted = sanscript.transliterate(text, sanscript.ITRANS, sanscript.ORIYA)
        return dict(text=converted, candidates=[converted], method='phonetic-itrans')
    if not config:
        raise ValueError('Open Models → Install reactions first' if task == 'reactions' else 'Open Models and finish speech setup first' if task in ('diarize', 'transcribe') else f'{task.title()} is not installed in this version')
    if task == 'reactions':
        config = dict(config, reaction_policy='host-reference-v4-multiple-look-nod')
        if config.get('provider') == 'mediapipe-reference':
            device = 'cpu'  # This lightweight face model has a CPU runtime, not a GPU fallback.
    if device not in ('auto', 'cpu', 'cuda'):
        raise ValueError('Choose Auto, CPU or GPU')
    if device != 'auto':
        config = dict(config, device=device)
    # Old NVIDIA installers wrote auto; CPU installs explicitly write cpu.
    if task in ('diarize', 'transcribe') and config.get('device') == 'auto':
        config = dict(config, device='cuda')
    # Never reuse a cache produced under the old silent CPU-fallback policy.
    config = dict(config, allow_cpu_fallback=False)
    if not config.get('revision'):
        raise ValueError('Provider configuration needs a revision for reproducible caching')
    digest = hashlib.sha256(json.dumps(dict(task=task, payload=payload, config=config), sort_keys=True).encode()).hexdigest()
    destination = directory / f'model-{digest}.json'
    if destination.exists():
        return json.loads(destination.read_text(encoding='utf-8'))
    request = directory / f'request-{digest}.json'
    partial = directory / f'model-{digest}.partial.json'
    status_path = directory / f'progress-{digest}.json'
    partial.unlink(missing_ok=True)
    status_path.unlink(missing_ok=True)
    request.write_text(json.dumps(dict(task=task, payload=payload, config=config, progress_path=str(status_path))))
    last_status = None
    last_poll = 0.
    def report():
        nonlocal last_status, last_poll
        now = time.monotonic()
        if now - last_poll < .5:
            return
        last_poll = now
        try:
            text = status_path.read_text(encoding='utf-8')
            if text != last_status:
                value = json.loads(text)
                progress(value)
                last_status = text
        except (OSError, ValueError):
            pass
    execution_label = 'NVIDIA GPU' if str(config.get('device', 'cpu')).startswith('cuda') else 'CPU'
    progress(dict(stage=f'Starting {execution_label} worker'))
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
    started = time.monotonic()
    diagnostic = dict(task=task, provider=config.get('provider'), device=config.get('device', 'cpu'), status='failed')
    try:
        try:
            run([*command, str(request), str(partial)], cancel, timeout=86400, on_poll=report)
        except RuntimeError as exc:
            if cancel():
                raise Cancelled('Job cancelled')
            diagnostic['error_category'] = 'gpu_out_of_memory' if 'cuda out of memory' in str(exc).lower() else 'worker_error'
            if 'cuda out of memory' in str(exc).lower():
                raise RuntimeError('GPU memory is full. Your saved results are unchanged. '
                    'Retry on GPU after closing other GPU apps; for transcription, choose shorter segments. '
                    'CPU processing starts only when you select CPU explicitly.') from exc
            if task == 'reactions':
                messages = [line.strip() for line in str(exc).splitlines() if line.startswith(('ValueError:', 'FileNotFoundError:'))]
                if messages:
                    raise ValueError(messages[-1].split(':', 1)[1].strip()) from exc
            raise
        if cancel():
            raise Cancelled('Job cancelled')
        report()
        progress(dict(stage='Saving results'))
        result = json.loads(partial.read_text(encoding='utf-8'))
        if not isinstance(result, dict):
            raise ValueError('Worker result must be a JSON object')
        diagnostic.update(status='succeeded', device=result.get('_device', config.get('device','cpu')), resources=result.pop('_resources', {}))
        result['_execution'] = dict(worker_exited=True, cpu_fallback=False, device=result.pop('_device', config.get('device', 'cpu')))
        partial.write_text(json.dumps(result, ensure_ascii=False), encoding='utf-8')
        partial.replace(destination)
        return result
    finally:
        diagnostic['elapsed_seconds'] = round(time.monotonic() - started, 2)
        diagnostic['utc'] = time.strftime('%Y-%m-%dT%H:%M:%SZ', time.gmtime())
        try:
            log = directory / 'model-diagnostics.json'
            history = json.loads(log.read_text(encoding='utf-8')) if log.is_file() else []
            if not isinstance(history, list): history = []
            log.write_text(json.dumps((history + [diagnostic])[-100:]), encoding='utf-8')
        except (OSError, ValueError):
            pass  # Diagnostic storage must never lose a successful edit.
        status_path.unlink(missing_ok=True)
        request.unlink(missing_ok=True)
        partial.unlink(missing_ok=True)
