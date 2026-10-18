import pytest
from betun.schema import Project, Clip, Activity, Caption, Shot, Reaction
from betun.podcast import clean_activity, speaker_samples, suggestions, manifest, TICKS


def episode():
    p=Project(name='Odia',duration=60,clips=[
        Clip(id='master',path='master.wav',camera='master',duration=60,fingerprint='m'),
        Clip(id='host',path='host.mp4',camera='A',duration=60,fingerprint='h',has_video=True,sync_reviewed=True),
        Clip(id='guest',path='guest.mp4',camera='B',duration=60,fingerprint='g',has_video=True,sync_reviewed=True)])
    p.settings.host_speaker='SPEAKER_00';p.settings.roles_locked=True
    p.activity=[Activity(start=0,end=10,speaker='SPEAKER_00'),Activity(start=8,end=60,speaker='SPEAKER_01')]
    p.shots=[Shot(start=0,end=8,layout='A'),Shot(start=8,end=11,layout='both'),Shot(start=11,end=60,layout='B')]
    p.captions=[Caption(start=0,end=2,speaker='SPEAKER_00',text='ନମସ୍କାର ଓଡ଼ିଶା',reviewed=True)]
    return p


def test_malformed_activity_is_removed_without_removing_short_speech():
    rows=[dict(start=1,end=1,speaker='a'),dict(start=2,end=3,speaker=''),dict(start=70,end=80,speaker='b'),
          dict(start=0,end=.05,speaker='a'),dict(start=10,end=100,speaker='b'),dict(start=float('nan'),end=2,speaker='b')]
    cleaned,removed=clean_activity(rows,60)
    assert removed==4
    assert [(a.start,a.end) for a in cleaned]==[(0,.05),(10,60)]


def test_samples_exclude_other_speaker_and_choose_longest():
    samples=speaker_samples(episode())
    assert samples==[dict(speaker='SPEAKER_00',start=0,end=8),dict(speaker='SPEAKER_01',start=10,end=22)]


def test_reactions_cannot_replace_host_overlap_or_locked_edits():
    p=episode()
    p.shots=[Shot(start=0,end=8,layout='A'),Shot(start=8,end=11,layout='both'),Shot(start=11,end=40,layout='B'),Shot(start=40,end=60,layout='B',locked=True)]
    candidates=[dict(start=a,end=a+4,clip_id='host',score=.9,cues=['nod']) for a in (2,8,10,15,17,45)]
    r=suggestions(p,candidates)
    assert len(r)==1 and r[0].start==15


def test_manifest_uses_original_source_time_and_honest_estimated_words():
    p=episode();p.reactions=[Reaction(start=20,end=24,clip_id='host',score=.8,decision='keep')]
    plan=manifest(p)
    reaction=next(s for s in plan['clips'] if s['start']==20*TICKS)
    assert reaction['video'][0]['asset']=='host'
    assert reaction['video'][0]['source_in']==20*TICKS
    assert plan['transcript']['words'][0]['text']=='ନମସ୍କାର'
    assert 'estimated' in plan['transcript']['source']
    assert plan['transcript']['words'][0]['confidence']==0
    p.reactions[0].decision='pending'
    with pytest.raises(ValueError,match='Keep or reject'):manifest(p)


def test_shard_boundaries_do_not_compress_recording_gaps():
    p=episode();p.clips[1].duration=25
    p.clips.append(Clip(id='host2',path='host2.mp4',camera='A',duration=30,offset=30,fingerprint='h2',has_video=True,sync_reviewed=True))
    p.reactions=[Reaction(start=32,end=36,clip_id='host2',score=.8,decision='keep')]
    plan=manifest(p)
    segment=next(s for s in plan['clips'] if s['start']==32*TICKS)
    assert segment['video'][0]['source_in']==2*TICKS
    assert segment['video'][0]['asset']=='host2'
    assert plan['clips'][-1]['end']==60*TICKS


def test_remote_export_relinks_snapshot_without_changing_edit(tmp_path,monkeypatch):
    import json
    from betun.remote_media import export_originals
    p=episode()
    cli=tmp_path/'filmcraft-cli';cli.touch()
    monkeypatch.setenv('BETUN_FILMCRAFT_CLI',str(cli))
    monkeypatch.setattr('betun.remote_media.media.fingerprint',lambda path:next(c.fingerprint for c in p.clips if c.path==path))
    def run(args,cancel,timeout,on_poll=None):
        snapshot=json.loads((tmp_path/'snapshot.fcproj').read_text())['project']
        assert snapshot['items']['3']['kind']['Media']['media']['File']['path']=='host.mp4'
        assert snapshot['custom_edit']=='retained'
        commands=[json.loads(line) for line in (tmp_path/'render.jsonl').read_text().splitlines()]
        assert commands[-1]['id']=='file.exportMedia' and commands[-1]['params']['wait']
        (tmp_path/'episode.mp4').touch()
    monkeypatch.setattr('betun.remote_media.media.run',run)
    snapshot={'items':{'3':{'kind':{'Media':{'media':{'File':{'path':r'C:\cache\h-preview-v1.mp4'}}}}},'4':{'kind':{'Sequence':{'video_tracks':[{'items':[{'item':3}]}]}}}},'custom_edit':'retained'}
    result=export_originals(p,dict(snapshot=snapshot,sequence=4),tmp_path,lambda:False,lambda _:None)
    assert 'episode.or.srt' in result['files']


def test_cancelled_worker_is_reaped(tmp_path):
    import sys,time
    from betun.media import run,Cancelled
    start=time.monotonic()
    with pytest.raises(Cancelled):
        run([sys.executable,'-c','import time; time.sleep(30)'],cancel=lambda:time.monotonic()-start>.1)
    assert time.monotonic()-start<10


def test_provider_cpu_retry_starts_after_gpu_worker_has_exited(tmp_path,monkeypatch):
    import json
    from pathlib import Path
    from betun.providers import invoke
    monkeypatch.setattr('betun.providers.configuration',lambda:{'diarize':dict(provider='fake',revision='fixture',device='cuda',allow_cpu_fallback=True,command=['worker'])})
    devices=[]
    def run(args,cancel,timeout,on_poll=None):
        config=json.loads(Path(args[-2]).read_text())['config']
        devices.append(config['device'])
        if config['device']=='cuda':raise RuntimeError('CUDA out of memory')
        Path(args[-1]).write_text(json.dumps(dict(activity=[])))
    monkeypatch.setattr('betun.providers.run',run)
    result=invoke('diarize',dict(audio='fake'),tmp_path)
    assert devices==['cuda','cpu']
    assert result['_execution']==dict(worker_exited=True,cpu_fallback=True)


def test_cpu_choice_overrides_gpu_config_and_uses_a_separate_cache(tmp_path, monkeypatch):
    import json
    from pathlib import Path
    from betun import providers
    config = dict(provider='fake', revision='fixture', device='cuda', command=['worker'])
    monkeypatch.setattr(providers, 'configuration', lambda: {'diarize': config})
    devices = []
    def run(args, cancel, timeout, on_poll=None):
        device = json.loads(Path(args[-2]).read_text())['config']['device']
        devices.append(device)
        Path(args[-1]).write_text(json.dumps(dict(activity=[])))
    monkeypatch.setattr(providers, 'run', run)
    providers.invoke('diarize', dict(audio='fixture'), tmp_path, device='cpu')
    providers.invoke('diarize', dict(audio='fixture'), tmp_path, device='cpu')
    providers.invoke('diarize', dict(audio='fixture'), tmp_path, device='auto')
    assert devices == ['cpu', 'cuda']
    assert config['device'] == 'cuda'


def test_reaction_edits_reject_out_of_range_and_overlapping_kept_shots():
    p=episode()
    p.reactions=[Reaction(start=20,end=24,clip_id='host',score=.8,decision='keep'),
                 Reaction(start=23,end=27,clip_id='host',score=.8,decision='keep')]
    with pytest.raises(ValueError,match='must not overlap'):
        Project.model_validate(p.model_dump())
    p.reactions=[Reaction(start=58,end=62,clip_id='host',score=.8)]
    with pytest.raises(ValueError,match='exceeds master'):
        Project.model_validate(p.model_dump())


def test_gaze_tolerance_uses_angle_between_directions():
    import math
    from betun.vision_worker import angle
    assert angle((0,0),(0,math.radians(10))) == pytest.approx(10)
    assert angle((.2,.3),(.2,.3)) == pytest.approx(0,abs=1e-5)
    # Yaw wrapping must not turn nearby directions into a 358-degree mismatch.
    assert angle((0,math.radians(179)),(0,math.radians(-179))) == pytest.approx(2)


def test_gaze_skips_missing_faces_without_hiding_model_failures(monkeypatch):
    import contextlib, sys
    from types import SimpleNamespace
    from betun.vision_worker import estimate_gaze
    monkeypatch.setitem(sys.modules, 'torch', SimpleNamespace(inference_mode=contextlib.nullcontext))
    class Detector:
        message = 'need at least one array to stack'
        def step(self, frame):
            raise ValueError(self.message)
    detector = Detector()
    assert estimate_gaze(detector, None) is None
    detector.message = 'invalid model dimensions'
    with pytest.raises(ValueError, match='invalid model dimensions'):
        estimate_gaze(detector, None)


@pytest.mark.parametrize('device', ['auto', 'cuda'])
def test_gpu_budget_resolves_current_cuda_index(tmp_path, monkeypatch, device):
    import json, sys
    from types import SimpleNamespace
    from betun.model_worker import main
    limited = []
    def properties(selected):
        assert selected == 'cuda:2'
        return SimpleNamespace(total_memory=6 * 1024**3)
    cuda = SimpleNamespace(is_available=lambda: True, current_device=lambda: 2,
        get_device_properties=properties,
        set_per_process_memory_fraction=lambda fraction, selected: limited.append((fraction, selected)))
    monkeypatch.setitem(sys.modules, 'torch', SimpleNamespace(cuda=cuda,
        set_num_threads=lambda _: None, device=lambda _: SimpleNamespace(index=None)))
    def analyze(payload, config, progress=None):
        assert config['device'] == 'cuda:2'
        return {'candidates': []}
    monkeypatch.setitem(sys.modules, 'vision_worker', SimpleNamespace(analyze=analyze))
    request = tmp_path/'request.json'; result = tmp_path/'result.json'
    request.write_text(json.dumps(dict(task='reactions', payload={}, config=dict(
        provider='mediapipe-l2cs', model_path=str(tmp_path), device=device, max_vram_gb=4.5))))
    main(str(request), str(result))
    assert limited == [(0.75, 'cuda:2')]
    assert json.loads(result.read_text()) == {'candidates': []}


def test_reference_preview_validates_shard_bounds_before_decoding(tmp_path,monkeypatch):
    from betun.pipeline import process
    from betun.store import Store
    store=Store(tmp_path/'data');p=store.create(episode())
    out=tmp_path/'preview';out.mkdir()
    with pytest.raises(ValueError,match='within a camera shard'):
        process(store,p,'reference_frame',dict(clip_id='host',time=61),out,lambda:False,lambda _:None)
    monkeypatch.setattr('betun.pipeline.media.fingerprint',lambda _: 'changed')
    with pytest.raises(ValueError,match='Source changed'):
        process(store,p,'reference_frame',dict(clip_id='host',time=10),out,lambda:False,lambda _:None)


def test_odia_worker_results_and_cache_ignore_windows_default_encoding(tmp_path, monkeypatch):
    import json
    from pathlib import Path
    from betun import providers
    original_read = Path.read_text
    def windows_read(path, encoding=None, errors=None):
        return original_read(path, encoding=encoding or 'cp1252', errors=errors)
    monkeypatch.setattr(Path, 'read_text', windows_read)
    config = tmp_path / 'models.json'
    config.write_text(json.dumps({'transcribe': dict(provider='fake', revision='fixture',
        model_path='ଓଡ଼ିଆ', command=['worker'])}, ensure_ascii=False), encoding='utf-8')
    monkeypatch.setenv('BETUN_MODEL_CONFIG', str(config))
    assert providers.configuration()['transcribe']['model_path'] == 'ଓଡ଼ିଆ'
    calls = []
    text = 'ନମସ୍କାର ଓଡ଼ିଶା'
    def run(args, cancel, timeout, on_poll=None):
        calls.append(True)
        Path(args[-1]).write_text(json.dumps(dict(captions=[dict(id='1', text=text)]),
            ensure_ascii=False), encoding='utf-8')
    monkeypatch.setattr(providers, 'run', run)
    for _ in range(2):
        result = providers.invoke('transcribe', dict(audio='fixture'), tmp_path)
        assert result['captions'][0]['text'] == text
    assert len(calls) == 1


def test_native_review_has_both_cameras_without_approving_or_mutating_project():
    from betun.podcast import review_manifest
    p = episode()
    p.clips[1].sync_reviewed = False
    p.captions[0].reviewed = False
    before = p.model_dump()
    plan = review_manifest(p)
    assert plan['review'] is True
    assert plan['master'] == 'master'
    assert {v['asset'] for v in plan['clips'][0]['video']} == {'host', 'guest'}
    assert p.model_dump() == before


def test_edit_review_shows_kept_reaction_and_preserves_rejected_guest_shot():
    from betun.podcast import review_manifest
    p = episode()
    p.reactions = [Reaction(start=20, end=24, clip_id='host', score=.9, decision='keep'),
                   Reaction(start=30, end=34, clip_id='host', score=.8, decision='reject')]
    plan = review_manifest(p, edit=True)
    keep = next(c for c in plan['clips'] if c['start'] == 20*TICKS)
    reject = next(c for c in plan['clips'] if c['start'] == 30*TICKS)
    assert next(v['asset'] for v in keep['video'] if v['side']=='review-top-full') == 'host'
    assert next(v['asset'] for v in reject['video'] if v['side']=='review-top-full') == 'guest'
    assert len(keep['video']) == 3


def test_review_preserves_camera_gaps_and_shard_clock():
    from betun.podcast import review_manifest
    p = episode()
    p.clips[1].offset = 10
    p.clips[2].offset = 10
    p.clips[1].rate = 1.01
    plan = review_manifest(p)
    assert plan['clips'][0]['start'] == 0
    assert plan['clips'][0]['end'] == 10*TICKS
    assert plan['clips'][0]['video'] == []
    assert plan['clips'][1]['video'][0]['source_in'] == 0
    assert plan['clips'][1]['video'][0]['speed'] == 1/1.01


def test_gap_removal_only_after_approval_preserves_source_audio_time():
    from betun.schema import Cut
    p = episode()
    for camera in p.clips[1:]:
        camera.offset = 5
        camera.duration = 55
    with pytest.raises(ValueError, match='No camera covers'):
        manifest(p)
    p.cuts = [Cut(start=0, end=5, approved=False)]
    with pytest.raises(ValueError, match='No camera covers'):
        manifest(p)
    p.cuts[0].approved = True
    plan = manifest(p)
    assert plan['source_duration'] == 60*TICKS
    assert plan['clips'][0]['start'] == 0
    assert plan['clips'][0]['source_start'] == 5*TICKS
    assert plan['clips'][0]['video'][0]['source_in'] == 0
    assert plan['clips'][-1]['end'] == 55*TICKS
    assert plan['master'] == 'master'


def test_overlapping_speech_keeps_both_speakers_text_for_native_editor():
    p = episode()
    p.captions.append(Caption(start=1,end=3,speaker='SPEAKER_01',text='ହଁ',reviewed=True))
    words = manifest(p)['transcript']['words']
    assert [w['text'] for w in words] == ['ନମସ୍କାର', 'ଓଡ଼ିଶା', 'ହଁ']
    assert all(a['end'] <= b['start'] for a,b in zip(words, words[1:]))
    assert p.captions[1].start == 1


def test_subframe_camera_start_is_reviewable_without_false_gap_failure():
    p = episode()
    p.clips[1].offset = .013
    p.clips[2].offset = .001
    plan = manifest(p)
    assert plan['clips'][0]['video'][0]['source_in'] == 0
    assert plan['clips'][-1]['end'] == 60*TICKS
