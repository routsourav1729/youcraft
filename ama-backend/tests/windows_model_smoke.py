"""Exercise downloaded speech runtime/weights on Windows without private recordings."""
import json
from pathlib import Path
import subprocess
import sys
import wave

root = Path(sys.argv[1])
source = Path(__file__).resolve().parents[1]
models = root/'models'
subprocess.run([sys.executable, str(source/'scripts/download_models.py'), '--directory', str(models), '--pack', 'nemotron'], check=True, timeout=900)
subprocess.run([sys.executable, str(source/'scripts/configure_local_models.py'), '--directory', str(models), '--output', str(root/'models.json'), '--partial'], check=True)
config = json.loads((root/'models.json').read_text(encoding='utf-8'))['diarize']
audio = root/'silence.wav'
with wave.open(str(audio), 'wb') as stream:
    stream.setnchannels(1)
    stream.setsampwidth(2)
    stream.setframerate(16000)
    stream.writeframes(b'\0\0'*16000*4)
request = root/'request.json'
result = root/'result.json'
request.write_text(json.dumps(dict(task='diarize', config=config, payload=dict(audio=str(audio)))), encoding='utf-8')
subprocess.run([sys.executable, str(source/'backend/betun/model_worker.py'), str(request), str(result)], check=True, timeout=180)
assert isinstance(json.loads(result.read_text(encoding='utf-8'))['activity'], list)
print('Downloaded Windows CPU speaker model completed inference on synthetic audio.')
