from betun.schema import Project, Clip, Activity, Shot
from betun.timeline import generate_shots


def episode():
    p = Project(name='overlap', duration=30, clips=[
        Clip(path=c, camera=c, duration=30, fingerprint=c, has_video=True)
        for c in ['master', 'A', 'B']])
    p.settings.speaker_camera = {'host':'A', 'guest':'B'}
    p.settings.minimum_shot = 0
    p.settings.acknowledgement = 0
    p.settings.overlap_padding = 2
    p.settings.meaningful_overlap = .6
    return p


def test_padding_actual_overlap_only():
    p = episode()
    p.activity = [Activity(start=0,end=12,speaker='host'), Activity(start=10,end=30,speaker='guest')]
    shots = generate_shots(p)
    assert [(s.start,s.end,s.layout) for s in shots] == [(0,8,'A'),(8,14,'both'),(14,30,'B')]
    p.activity[0].end = 10
    assert all(s.layout != 'both' for s in generate_shots(p))


def test_lock_overrides_padded_overlap_and_short_overlap_is_ignored():
    p = episode()
    p.activity = [Activity(start=0,end=12,speaker='host'), Activity(start=10,end=30,speaker='guest')]
    lock = Shot(start=10,end=11,layout='A',locked=True)
    p.shots = [lock]
    assert next(s for s in generate_shots(p) if s.id==lock.id).layout == 'A'
    p.shots = []
    p.activity[0].end = 10.2
    assert all(s.layout != 'both' for s in generate_shots(p))
