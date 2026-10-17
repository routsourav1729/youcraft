"""Verify the frozen helper can launch the installed real CPU speech runtime."""
import json
import os
from pathlib import Path
import sys
import time
import urllib.request

endpoint = os.environ['YOUCRAFT_SMOKE_ENDPOINT']
token = os.environ['YOUCRAFT_SMOKE_TOKEN']
config, installed, media = map(Path, sys.argv[1:])
original = config.read_bytes()
def api(path, data=None):
    request = urllib.request.Request(endpoint+path, data=None if data is None else json.dumps(data).encode(),
        headers={'Authorization':'Bearer '+token,'Content-Type':'application/json'})
    with urllib.request.urlopen(request, timeout=10) as response:
        return json.load(response)
def job(pid, kind, options):
    result = api(f'/v1/projects/{pid}/jobs', dict(kind=kind, options=options))
    deadline = time.monotonic()+180
    while result['state'] in ('queued','running'):
        assert time.monotonic()<deadline, 'Packaged model job timed out'
        time.sleep(.2)
        result = api('/v1/jobs/'+result['id'])
    return result
try:
    config.write_bytes(installed.read_bytes())
    pid = api('/v1/projects', {'name':'Packaged CPU model check'})['id']
    result = job(pid, 'import', {'master':str(media/'master.wav')})
    assert result['state']=='succeeded', result
    result = job(pid, 'diarize', {})
    # Synthetic tone can legitimately contain no speech; distinguish that from a runtime failure.
    assert result['state']=='succeeded' or (result['state']=='failed' and 'No usable speaker activity' in result['error']), result
    print('Packaged helper launched the installed CPU speaker model successfully.')
finally:
    config.write_bytes(original)
