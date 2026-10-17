from pathlib import Path
import pytest
from betun.pipeline import process
from betun.schema import Project, Clip, Activity, Shot
from betun.store import Store


def setup(tmp_path):
    store = Store(tmp_path / 'data')
    project = store.create(Project(name='Audio first', duration=60,
        clips=[Clip(path=str(tmp_path / 'master.wav'), camera='master', duration=60, fingerprint='audio')],
        activity=[Activity(start=2, end=9, speaker='person-1')],
        shots=[Shot(start=0, end=10, layout='A'), Shot(start=10, end=20, layout='both', locked=True)]))
    work = tmp_path / 'job'
    work.mkdir()
    return store, project, work


def test_attach_after_diarization_preserves_voices_and_manual_edits(tmp_path, monkeypatch):
    store, project, work = setup(tmp_path)
    def probe(path, camera, cancel):
        return Clip(path=path, camera=camera, duration=30, has_video=True, fingerprint=path)
    monkeypatch.setattr('betun.pipeline.media.probe', probe)
    process(store, project, 'attach_cameras', {'A':['cam10.mp4','cam2.mp4'], 'B':['guest.mp4']}, work, lambda:False, lambda p:None)
    loaded = store.get(project.id)
    assert loaded.duration == 60
    assert [c.path for c in loaded.clips[1:]] == ['cam10.mp4','cam2.mp4','guest.mp4']  # Preserve the user's explicit order.
    assert loaded.activity[0].speaker == 'person-1'
    assert len(loaded.shots) == 1 and loaded.shots[0].locked
    assert all(not c.sync_reviewed for c in loaded.clips[1:])


def test_duplicate_attachment_is_atomic(tmp_path, monkeypatch):
    store, project, work = setup(tmp_path)
    monkeypatch.setattr('betun.pipeline.media.probe', lambda path, camera, cancel:
        Clip(path=path, camera=camera, duration=30, has_video=True, fingerprint='video'))
    with pytest.raises(ValueError, match='already imported'):
        process(store, project, 'attach_cameras', {'A':['same.mp4'], 'B':['same.mp4']}, work, lambda:False, lambda p:None)
    assert len(store.get(project.id).clips) == 1


def test_audio_cannot_be_attached_as_camera(tmp_path, monkeypatch):
    store, project, work = setup(tmp_path)
    monkeypatch.setattr('betun.pipeline.media.probe', lambda path, camera, cancel:
        Clip(path=path, camera=camera, duration=30, has_video=False, fingerprint='audio'))
    with pytest.raises(ValueError, match='contain video'):
        process(store, project, 'attach_cameras', {'A':['voice.wav'], 'B':[]}, work, lambda:False, lambda p:None)
    assert len(store.get(project.id).clips) == 1
