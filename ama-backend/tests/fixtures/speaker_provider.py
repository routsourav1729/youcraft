"""Synthetic two-speaker worker for packaged/native workflow tests; no ML weights."""
import json
from pathlib import Path
import sys

request = json.loads(Path(sys.argv[1]).read_text(encoding='utf-8'))
assert request['task'] == 'diarize'
assert request['config']['device'] == 'cpu'
Path(sys.argv[2]).write_text(json.dumps({'activity': [
    {'start': 0, 'end': 2, 'speaker': 'first'},
    {'start': 2, 'end': 4, 'speaker': 'second'},
]}), encoding='utf-8')
