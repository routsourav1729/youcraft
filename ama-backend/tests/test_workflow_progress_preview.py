import json
from pathlib import Path
from types import SimpleNamespace
import sys
import pytest
from betun.schema import Clip, Project
from betun.store import Store
from betun.pipeline import process


@pytest.mark.parametrize('position,expected', [(0, 3.), (.5, 7.), (1, 11.)])
def test_sync_preview_maps_camera_and_master_times(tmp_path, monkeypatch, position, expected):
    store = Store(tmp_path / 'data')
    clip = Clip(path='camera.mp4', camera='A', fingerprint='camera', duration=20, offset=3., rate=1., has_video=True)
    project = store.create(Project(name='Sync', duration=40, clips=[
        Clip(path='master.wav', camera='master', fingerprint='master', duration=40), clip]))
    commands = []
    monkeypatch.setattr('betun.pipeline.media.binary', lambda _: 'ffmpeg')
    monkeypatch.setattr('betun.pipeline.media.fingerprint', lambda _: 'camera')
    monkeypatch.setattr('betun.pipeline.media.run', lambda command, cancel: commands.append(command))
    result = process(store, project, 'sync_preview', dict(clip_id=clip.id, position=position), tmp_path, lambda:False, lambda _:None)
    command = commands[0]
    seeks = [float(command[i + 1]) for i, arg in enumerate(command) if arg == '-ss']
    assert seeks == [expected - 3., expected]
    assert command[command.index('-t') + 1] == '12.0'
    assert '1:a:0' in command  # Master audio, never the feeble camera track.
    assert result['files'] == ['reaction.mp4']
    assert store.get(project.id).clips[1].sync_reviewed is False


def test_transcription_reports_loading_and_completed_chunks(tmp_path, monkeypatch):
    from betun.model_worker import main
    status = tmp_path / 'status.json'
    events = []
    monkeypatch.setitem(sys.modules, 'torch', SimpleNamespace(set_num_threads=lambda _:None))
    class ASR:
        @staticmethod
        def from_pretrained(*args, **kwargs):
            events.append(json.loads(status.read_text(encoding="utf-8")))
            return object()
    def transcribe(*args, **kwargs):
        events.append(json.loads(status.read_text(encoding="utf-8")))
        return 'ନମସ୍କାର'
    monkeypatch.setitem(sys.modules, 'indic_transcribe', SimpleNamespace(IndicTranscribe=ASR))
    monkeypatch.setitem(sys.modules, 'long_form', SimpleNamespace(transcribe_long=transcribe))
    request = tmp_path / 'request.json'
    request.write_text(json.dumps(dict(task='transcribe', config=dict(provider='indic-core', model_path=str(tmp_path), device='cpu'),
        progress_path=str(status), payload=dict(chunks=[dict(id='one', audio='one.wav'), dict(id='two', audio='two.wav')]))))
    main(str(request), str(tmp_path / 'result.json'))
    assert events[0]['stage'] == 'Loading model · CPU'
    assert [e['completed'] for e in events[1:]] == [0, 1]
    assert json.loads(status.read_text(encoding="utf-8"))['fraction'] == 1.
    assert len(json.loads((tmp_path / 'result.json').read_text(encoding="utf-8"))['captions']) == 2
    main(str(request), str(tmp_path / 'result.json'))
    # Restart reloads the runtime but does not transcribe either completed chunk.
    assert len(events) == 4


def test_lightweight_reactions_do_not_import_torch(tmp_path, monkeypatch):
    from betun.model_worker import main
    monkeypatch.setitem(sys.modules, 'torch', None)
    monkeypatch.setitem(sys.modules, 'vision_worker', SimpleNamespace(analyze=lambda *args:dict(candidates=[])))
    request = tmp_path / 'request.json'
    request.write_text(json.dumps(dict(task='reactions', payload={}, config=dict(provider='mediapipe-reference', model_path=str(tmp_path), device='cpu'))))
    main(str(request), str(tmp_path / 'result.json'))
    result = json.loads((tmp_path / 'result.json').read_text(encoding="utf-8"))
    assert result['candidates'] == []
    assert isinstance(result.get('_resources'), dict)
    assert sys.modules['torch'] is None
