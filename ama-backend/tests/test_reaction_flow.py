import pytest
from test_podcast_workflow import episode
from betun import pipeline
from betun.podcast import suggestions, review_manifest
from betun.store import Store
from betun.vision_worker import reference_times


def test_reference_sampling_prioritizes_host_speech_and_is_bounded():
    times = reference_times(900, [[10,20], [440,460], [880,899]])
    assert times[0] == 440
    assert len(times) <= 80 and len(times) == len(set(times))
    assert all(0 <= t < 900 for t in times)
    assert reference_times(.05, []) == [0.]


def test_only_real_smiles_or_nods_and_one_per_interval():
    p = episode()
    candidates = [dict(start=t,end=t+4,clip_id='host',score=score,cues=cues)
        for t,score,cues in [(15,.6,['smile']), (25,.9,['nod']), (35,1.,['reference look']), (45,.7,['smile'])]]
    assert [r.start for r in suggestions(p,candidates,interval=60,spacing=0)] == [25]
    assert [r.start for r in suggestions(p,candidates,interval=20,spacing=0)] == [15,25,45]


def test_interval_reselect_never_invokes_model_and_retains_decisions(tmp_path,monkeypatch):
    p=episode()
    candidates=[dict(start=t,end=t+4,clip_id='host',score=.8,cues=['nod']) for t in (15,25,45)]
    p.reaction_analysis=dict(candidates=candidates,interval=60,spacing=0,tolerance=10,measurements=[])
    p.reactions=suggestions(p,candidates,interval=60,spacing=0)
    p.reactions[0].decision='keep'
    store=Store(tmp_path);store.create(p)
    monkeypatch.setattr(pipeline.providers,'invoke',lambda *a,**kw:pytest.fail('must not rerun model'))
    pipeline.process(store,p,'reaction_select',dict(interval=20),tmp_path,lambda:False,lambda _:None)
    saved=store.get(p.id)
    assert [r.start for r in saved.reactions]==[15,25,45]
    assert saved.reactions[0].decision=='keep'
    assert saved.reaction_analysis['interval']==20
    assert saved.reaction_analysis['candidates']==candidates


def test_host_is_always_left_even_when_host_camera_is_b():
    p=episode();p.settings.host_speaker='SPEAKER_01'
    plan=review_manifest(p,edit=True)
    picture=next(v for span in plan['clips'] for v in span['video'] if v['side']=='review-bottom-A')
    assert picture['asset']=='guest'


def test_failed_reference_suggestions_return_partial_references(tmp_path,monkeypatch):
    p=episode();store=Store(tmp_path);store.create(p)
    monkeypatch.setattr(pipeline.media,'fingerprint',lambda path:'h')
    seen={}
    def invoke(task,payload,*a,**kw):
        seen.update(payload)
        return dict(references={},issues={'host':'Choose screenshot'})
    monkeypatch.setattr(pipeline.providers,'invoke',invoke)
    result=pipeline.process(store,p,'reference_suggestions',dict(references={},tolerance=10,interval=60,spacing=15),tmp_path,lambda:False,lambda _:None)
    assert result['issues']=={'host':'Choose screenshot'}
    assert seen['reference_windows']['host']==[]  # whole turn overlaps; broad fallback remains
    assert store.get(p.id).reactions==p.reactions


def test_pending_preview_shows_candidate_without_accepting_it():
    from betun.schema import Reaction
    from betun.podcast import TICKS
    p=episode();p.reactions=[Reaction(id='test',start=20,end=24,clip_id='host',score=.9,cues=['nod'])]
    plan=review_manifest(p,edit=True,reaction_id='test')
    span=next(s for s in plan['clips'] if s['start']==20*TICKS)
    assert next(v for v in span['video'] if v['side']=='review-top-full')['asset']=='host'
    assert p.reactions[0].decision=='pending'
    normal=review_manifest(p,edit=True)
    span=next(s for s in normal['clips'] if s['start']==20*TICKS)
    assert next(v for v in span['video'] if v['side']=='review-top-full')['asset']=='guest'


def test_worker_preserves_good_reference_when_another_shard_has_no_face(monkeypatch):
    import sys
    from types import SimpleNamespace as NS
    import numpy as np
    from betun.vision_worker import analyze
    points=[NS(x=.5,y=.5) for _ in range(474)]
    for i,x in [(33,.2),(133,.4),(468,.3),(362,.6),(263,.8),(473,.7)]:points[i]=NS(x=x,y=.5)
    class Face:
        def __enter__(self):return self
        def __exit__(self,*a):pass
        def detect(self,image):
            if not image.data.any():return NS(face_landmarks=[],face_blendshapes=[],facial_transformation_matrixes=[])
            return NS(face_landmarks=[points],face_blendshapes=[[NS(category_name='eyeBlinkLeft',score=0.)]],facial_transformation_matrixes=[np.eye(4)])
    class Capture:
        def __init__(self,path):self.good=path=='good'
        def isOpened(self):return True
        def set(self,*a):pass
        def read(self):return True,np.full((100,100,3),int(self.good),dtype=np.uint8)
        def release(self):pass
    monkeypatch.setitem(sys.modules,'cv2',NS(VideoCapture=Capture,CAP_PROP_POS_MSEC=1,COLOR_BGR2RGB=1,cvtColor=lambda x,*a:x, resize=lambda x,*a:x, IMWRITE_JPEG_QUALITY=1, imencode=lambda *a:(True,np.array([1,2,3],dtype=np.uint8))))
    monkeypatch.setitem(sys.modules,'mediapipe',NS(Image=lambda **kw:NS(**kw),ImageFormat=NS(SRGB=1),tasks=NS(BaseOptions=lambda **kw:kw,vision=NS(FaceLandmarkerOptions=lambda **kw:kw,RunningMode=NS(IMAGE=1),FaceLandmarker=NS(create_from_options=lambda opts:Face())))))
    result=analyze(dict(reference_only=True,clips=[dict(id='first',path='good',duration=60),dict(id='second',path='bad',duration=60)],reference_windows={'first':[[25,35]]}),dict(provider='mediapipe-reference',face_model='test'))
    assert 'first' in result['reference_candidates'] and 'first' not in result['issues']
    assert 'second' in result['issues'] and 'second' not in result['reference_candidates']


def test_candidate_frames_require_host_speech_and_vary_with_seed():
    from betun.vision_worker import reference_candidate_times
    a=reference_candidate_times(1000, [[0,10],[400,450],[600,650],[990,999]], 1)
    b=reference_candidate_times(1000, [[0,10],[400,450],[600,650],[990,999]], 2)
    assert a != b and 3 <= len(a) <= 80
    assert all(400 <= t <= 450 or 600 <= t <= 650 for t in a)
    assert reference_candidate_times(1000, [], 1) == []


def test_reference_candidates_do_not_mutate_calibration(tmp_path, monkeypatch):
    p=episode(); p.reaction_calibration={'references': {'host': 12.}}
    store=Store(tmp_path); store.create(p)
    monkeypatch.setattr(pipeline.media,'fingerprint',lambda path:'h')
    candidates={'host':[{'time':25.,'thumbnail':[1,2,3]}]}
    monkeypatch.setattr(pipeline.providers,'invoke',lambda *a,**kw:dict(reference_candidates=candidates))
    result=pipeline.process(store,p,'reference_suggestions',dict(references={},tolerance=10,interval=60,spacing=15),tmp_path,lambda:False,lambda _:None)
    assert result['reference_candidates']==candidates
    assert store.get(p.id).reaction_calibration=={'references':{'host':12.}}
