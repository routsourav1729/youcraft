from betun.reaction_calibration import rescore_candidates, preserve_choices
from betun.schema import Reaction
from test_podcast_workflow import episode


def test_low_smile_signal_can_be_selected_without_reprocessing_video():
    row = dict(clip_id='host', start=15, end=19, score=.5, cues=['reference look'])
    analysis = dict(candidates=[row], measurements=[dict(clip_id='host', time=16, detected=True, smile=.22)])
    assert 'smile' not in rescore_candidates(analysis, .35)[0]['cues']
    assert 'smile' in rescore_candidates(analysis, .13)[0]['cues']
    assert row['cues'] == ['reference look']


def test_new_worker_peak_does_not_depend_on_sparse_overlay_samples():
    analysis = dict(candidates=[dict(start=15, end=19, smile_peak=.4, score=.5, cues=['nod'])], measurements=[])
    assert rescore_candidates(analysis, .3)[0]['cues'] == ['nod', 'smile']
    assert rescore_candidates(analysis, .5)[0]['cues'] == ['nod']


def test_kept_edits_and_rejected_ranges_survive_changed_candidate_boundaries():
    p = episode()
    p.reactions = [Reaction(clip_id='host', start=15, end=19, decision='keep', score=.8, cues=['nod']),
                   Reaction(clip_id='host', start=25, end=29, decision='reject', score=.8, cues=['smile'])]
    selected = [Reaction(clip_id='host', start=16, end=20, score=.8, cues=['nod']),
                Reaction(clip_id='host', start=26, end=30, score=.8, cues=['smile'])]
    history = {}
    result = preserve_choices(p, selected, history)
    assert [(r.start, r.decision) for r in result] == [(15, 'keep'), (26, 'reject')]
    p.reactions = [result[0]]
    assert preserve_choices(p, selected, history)[1].decision == 'reject'


def test_target_replaces_interval_limit_but_respects_spacing_and_kept_edits():
    from betun.podcast import suggestions
    p = episode()
    candidates = [dict(clip_id='host',start=t,end=t+4,score=.8,cues=['smile']) for t in (15,25,35,45)]
    assert len(suggestions(p,candidates,interval=60,spacing=2)) == 1
    assert len(suggestions(p,candidates,spacing=2,host_target=.4)) == 4
    p.reactions = [Reaction(clip_id='host',start=15,end=19,score=.8,cues=['manual'],decision='keep')]
    assert not suggestions(p,candidates,spacing=2,host_target=.2)


def test_buffers_trim_inserts_without_removing_analysis_or_choices():
    from betun.reaction_calibration import fit_reactions_to_shots
    from betun.schema import Shot
    p = episode()
    p.reactions = [Reaction(clip_id='host',start=15,end=25,score=.8,cues=['manual'],decision='keep')]
    p.shots = [Shot(start=0,end=17,layout='B'), Shot(start=17,end=22,layout='both'),Shot(start=22,end=60,layout='B')]
    fitted = fit_reactions_to_shots(p)
    assert [(r.start,r.end,r.decision) for r in fitted] == [(15,17,'keep'),(22,25,'keep')]
    assert len({r.id for r in fitted}) == 2


def test_nod_requires_return_stable_yaw_and_confirmed_look():
    from betun.vision_worker import is_nod
    samples = [(i*.2,dict(pitch=p,yaw=0,matching=i in (0,5))) for i,p in enumerate((0,3,8,9,4,1))]
    assert is_nod(samples)
    assert not is_nod([(t,dict(m,pitch=i*3)) for i,(t,m) in enumerate(samples)])
    assert not is_nod([(t,dict(m,yaw=i*5)) for i,(t,m) in enumerate(samples)])


def test_brief_overlap_has_independent_before_after_buffers():
    from betun.timeline import generate_shots
    from betun.schema import Activity
    p=episode()
    p.activity=[Activity(start=0,end=60,speaker='SPEAKER_01'),Activity(start=20,end=20.05,speaker='SPEAKER_00')]
    p.settings.meaningful_overlap=0
    p.settings.overlap_before=.5
    p.settings.overlap_after=1.5
    shots=generate_shots(p)
    both=[q for q in shots if q.layout=='both']
    assert [(q.start,q.end) for q in both] == [(19.5,21.55)]


def test_initial_subframe_camera_gap_does_not_leave_output_black():
    from betun.timeline import generate_shots
    from betun.schema import Activity
    p = episode()
    p.clips[1].offset = .0003
    p.clips[2].offset = .013
    p.activity = [Activity(start=10,end=60,speaker='SPEAKER_01')]
    p.shots = generate_shots(p)
    assert all(s.layout != 'black' for s in p.shots if s.start >= .0003)


def test_overlap_does_not_extend_through_following_pause_or_minimum_shot():
    from betun.timeline import generate_shots
    from betun.schema import Activity
    p = episode()
    p.settings.overlap_before = 1
    p.settings.overlap_after = 1
    p.settings.meaningful_overlap = 0
    p.activity = [Activity(start=0,end=20.2,speaker='SPEAKER_01'),
                  Activity(start=20,end=20.05,speaker='SPEAKER_00'),
                  Activity(start=23,end=60,speaker='SPEAKER_01')]
    assert [(q.start,q.end) for q in generate_shots(p) if q.layout=='both'] == [(19,21.05)]


def test_api_buffer_change_preserves_measured_signals_and_manual_decisions(tmp_path):
    from fastapi.testclient import TestClient
    from betun.api import create_app
    from betun.store import Store
    p=episode()
    p.reactions=[Reaction(clip_id='host',start=15,end=19,score=.8,cues=['manual'],decision='keep')]
    p.reaction_analysis={'candidates': [], 'measurements':[{'time':15,'smile':.22}]}
    store=Store(tmp_path)
    store.create(p)
    with TestClient(create_app(root=tmp_path,token='reaction-test-token')) as client:
        body=p.model_dump()
        body['settings'].update(overlap_before=1,overlap_after=1,meaningful_overlap=0)
        response=client.put('/v1/projects/'+p.id,json=body,headers={'Authorization':'Bearer reaction-test-token'})
        assert response.status_code==200,response.text
        saved=response.json()
        assert saved['reaction_analysis']==p.reaction_analysis
        assert saved['reactions'][0]['decision']=='keep'


def test_blink_gate_adapts_to_confirmed_side_view_without_accepting_full_blinks():
    from betun.vision_worker import calibrated_blink_limit
    assert calibrated_blink_limit([{'blink':.7},{'blink':.72},{'blink':.75}]) == .84
    assert calibrated_blink_limit([{'blink':.1}]) == .6
    assert calibrated_blink_limit([{'blink':.9}]) == .95
