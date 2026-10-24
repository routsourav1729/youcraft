"""Fast check against the actual installed UI; no extra Rust test executable or models."""
import hashlib
import json
from pathlib import Path
import os
import socket
import time


def control_token():
    explicit = os.environ.get("FILMCRAFT_CONTROL_TOKEN")
    if explicit:
        return explicit
    root = Path(os.environ.get("BETUN_DATA_DIR", str(Path(os.environ["LOCALAPPDATA"]) / "Betun")))
    secret = (root / "api-token").read_text(encoding="utf-8").strip()
    return hashlib.sha256(b"youcraft-control-v1\0" + secret.encode()).hexdigest()


def ask(method, **params):
    with socket.create_connection(('127.0.0.1', 9876), timeout=15) as connection:
        connection.sendall((json.dumps(dict(method=method, params=params, token=control_token()))+'\n').encode())
        result = json.loads(connection.makefile('rb').readline())
    assert result.get('ok'), result
    return result['result']


def until(predicate, seconds=30):
    end = time.monotonic()+seconds
    while time.monotonic() < end:
        state = ask('podcast.inspect')
        if predicate(state):
            return state
        time.sleep(.2)
    raise AssertionError(state)


state = until(lambda s: not s['busy'])
assert state['projects_open'] and not state['project_id'], state
ask('ui.click', id='podcast.new_project')
until(lambda s: not s['projects_open'] and s['step'] == 1 and not s['project_id'])
ask('ui.click', id='podcast.audio_output')
until(lambda s: s['audio_output_open'])
ask('ui.click', id='podcast.audio.default')
until(lambda s: s['audio_test_status'].startswith('Default output restored'))
ask('ui.click', id='podcast.audio.test')
# A hosted runner may have no speaker: verify explicit feedback, not pretend it heard sound.
until(lambda s: s['audio_test_status'].startswith(('Playing', 'Test failed:', 'Test finished.', 'No audio callbacks')))
ask('ui.click', id='podcast.audio.copy')
ask('ui.click', id='podcast.audio.done')
until(lambda s: not s['audio_output_open'])
ask('ui.click', id='podcast.episodes')
until(lambda s: s['projects_open'] and not s['busy'])
print('Installed UI: explicit project choice; audio controls available; test feedback and diagnostics available.')
