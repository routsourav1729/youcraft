import json
import wave
from pathlib import Path
import pytest
from betun.schema import Project, Activity
from betun.pipeline import chunk_turns
from betun import providers


def test_shorter_transcription_segments_cover_the_entire_turn(tmp_path):
    audio = tmp_path / 'audio.wav'
    with wave.open(str(audio), 'wb') as f:
        f.setnchannels(1); f.setsampwidth(2); f.setframerate(16000)
        f.writeframes(b'\0\0' * 16000 * 35)
    p = Project(name='Memory recovery', duration=35, activity=[Activity(start=0,end=35,speaker='host')])
    p.settings.transcription_segment_seconds = 8
    chunks, captions = chunk_turns(p, {None: audio}, tmp_path)
    assert len(chunks) > 4
    assert captions[0].start == 0 and captions[-1].end == 35
    assert all(c.end - c.start <= 8 for c in captions)
    assert all(a.end == b.start for a,b in zip(captions,captions[1:]))
    assert all(Path(c['audio']).is_file() for c in chunks)


@pytest.mark.parametrize('value', [0,7,29,float('nan')])
def test_invalid_segment_size_rejected(value):
    with pytest.raises(ValueError):
        Project(name='Invalid', settings={'transcription_segment_seconds':value})


def test_old_nvidia_auto_config_never_silently_chooses_cpu(tmp_path, monkeypatch):
    monkeypatch.setattr(providers, 'configuration', lambda: {'diarize':dict(
        revision='old',provider='fake',device='auto',allow_cpu_fallback=True,command=['worker'])})
    seen = []
    def run(args, *unused, **kwargs):
        config = json.loads(Path(args[-2]).read_text())['config']
        seen.append(config)
        raise RuntimeError('CUDA out of memory')
    monkeypatch.setattr(providers, 'run', run)
    with pytest.raises(RuntimeError, match='GPU memory is full'):
        providers.invoke('diarize', {}, tmp_path)
    assert len(seen) == 1
    assert seen[0]['device'] == 'cuda'
    assert seen[0]['allow_cpu_fallback'] is False
    assert not list(tmp_path.glob('model-*.json'))
