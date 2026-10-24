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
