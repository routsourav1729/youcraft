import pytest
from betun.framing import stable_center
from betun.schema import Clip, Settings
from betun.podcast import review_manifest, manifest
from test_podcast_workflow import episode


def test_framing_rejects_ambiguous_or_sparse_faces():
    assert stable_center([[.2, .4], [.8, .4], [.2, .4], [.8, .4]]) is None
    assert stable_center([[.4, .4], [.4, .4]]) is None
    assert stable_center([[float('nan'), .4], [2, .4], ['x', .4]]) is None
    assert stable_center([[.4, .3]] * 8 + [[.9, .9]]) == (.4, .3)


def test_crop_settings_are_bounded_and_legacy_projects_load():
    assert Settings().split_framing == 'fill'
    clip = Clip(path='a.mp4', camera='A', duration=10, fingerprint='a')
    assert (clip.frame_x, clip.frame_y, clip.framing_reviewed) == (.5, .5, False)
    with pytest.raises(ValueError):
        clip.model_validate(dict(clip.model_dump(), frame_x=float('inf')))


def test_framing_job_preserves_confirmed_camera_and_edit_decisions(tmp_path, monkeypatch):
    from betun import pipeline
    from betun.store import Store
    p = episode()
    p.clips[1].frame_x = .7
    p.clips[1].framing_reviewed = True
    before = p.model_dump()
    store = Store(tmp_path)
    store.create(p)
    monkeypatch.setattr(pipeline.media, 'fingerprint', lambda path: 'g')
    def invoke(task, payload, *args, **kwargs):
        assert [c['id'] for c in payload['clips']] == ['guest']
        assert kwargs['device'] == 'cpu'
        return dict(framing={'guest': [[.6, .35]] * 9})
    monkeypatch.setattr(pipeline.providers, 'invoke', invoke)
    result = pipeline.process(store, p, 'framing', {}, tmp_path, lambda: False, lambda _: None)
    saved = store.get(p.id).model_dump()
    assert result['framed'] == 1
    assert saved['clips'][1] == before['clips'][1]
    assert saved['clips'][2]['frame_x'] == .6
    for key in ('activity', 'shots', 'captions', 'reactions'):
        assert saved[key] == before[key]


def test_same_framing_in_preview_and_final_only_on_split_screen():
    p = episode()
    p.clips[1].frame_x = .65
    # Explicit framing preview works before an edit is generated.
    preview = review_manifest(p, edit=True, framing=True)
    split = next(v for s in preview['clips'] for v in s['video'] if v['side'] == 'review-top-A')
    assert split['frame_center'] == [.65, .5]
    from betun.schema import Shot
    p.shots = [Shot(start=0, end=p.duration, layout='both')]
    final = manifest(p)
    assert all(v['frame_center'] == [.65, .5] for s in final['clips'] for v in s['video'] if v['side'] == 'A')
    sources = review_manifest(p)
    assert all('frame_center' not in v for s in sources['clips'] for v in s['video'])
    p.settings.split_framing = 'fit'
    assert all('frame_center' not in v for s in manifest(p)['clips'] for v in s['video'])
