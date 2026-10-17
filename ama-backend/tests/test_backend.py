import time
import wave
from pathlib import Path
import pytest
from fastapi.testclient import TestClient
from betun.api import create_app
from betun.schema import Project, Clip, Caption, Activity, Cut, Shot
from betun.timeline import time_map, srt, generate_shots, render_plan
from betun.evaluation import compare
from betun import media

TOKEN='test-token-at-least-16-characters'
@pytest.fixture
def client(tmp_path):
    with TestClient(create_app(tmp_path,token=TOKEN)) as c:
        c.headers['Authorization']='Bearer '+TOKEN
        yield c

def project():
    return Project(name='test',duration=20,clips=[Clip(camera='master',path='master.wav',duration=20,channels=2,fingerprint='m'),Clip(camera='A',path='a.mp4',duration=20,fingerprint='a',has_video=True,sync_reviewed=True),Clip(camera='B',path='b.mp4',duration=8,offset=4,fingerprint='b',has_video=True,sync_reviewed=True)])

def test_health_identifies_helper_before_authentication(client):
    client.headers.pop('Authorization')
    health = client.get('/health')
    assert health.status_code == 200
    assert health.json() == dict(status='ok', service='youcraft', protocol_version=1,
                                 schema_version=1, version='0.1.0')
    assert client.get('/v1/projects').status_code == 401

def test_ui_and_cookie_auth(client):
    assert client.get('/').status_code==200
    assert client.get('/ui/app.js').status_code==200
    assert client.get('/review-data.js').status_code==200
    assert client.post('/v1/session').status_code==200
    client.headers.pop('Authorization')
    assert client.get('/v1/projects').status_code==200
    assert client.post('/v1/projects',json={'name':'cookie'}).status_code==403
    assert client.post('/v1/projects',json={'name':'cookie'},headers={'X-Betun-Review':'1'}).status_code==201
    assert client.post('/v1/projects',json={'name':'bad'},headers={'X-Betun-Review':'1','Origin':'https://example.org'}).status_code==403

def test_revisions_and_undo(client):
    p=client.post('/v1/projects',json={'name':'before'}).json();p['name']='after'
    assert client.put('/v1/projects/'+p['id'],json=p).status_code==200
    assert client.put('/v1/projects/'+p['id'],json=p).status_code==409
    result=client.post('/v1/projects/'+p['id']+'/undo',json={'revision':1}).json()
    assert result['name']=='before' and result['revision']==2

def test_audio_only_import(client,tmp_path):
    try:
        media.binary('ffprobe')
    except RuntimeError:
        pytest.skip('ffprobe unavailable')
    path=tmp_path/'input.wav'
    with wave.open(str(path),'wb') as w:
        w.setnchannels(1);w.setsampwidth(2);w.setframerate(16000);w.writeframes(b'\0\0'*16000)
    p=client.post('/v1/projects',json={'name':'audio only'}).json()
    response=client.post('/v1/projects/'+p['id']+'/jobs',json={'kind':'import','options':{'master':str(path)}})
    assert response.status_code==202
    jid=response.json()['id'];deadline=time.monotonic()+80
    while time.monotonic()<deadline:
        job=client.get('/v1/jobs/'+jid).json()
        if job['state'] not in ('queued','running'):break
        time.sleep(.1)
    assert job['state']=='succeeded',job
    loaded=client.get('/v1/projects/'+p['id']).json()
    assert len(loaded['clips'])==1 and loaded['duration']==1

def test_cut_mapping_and_overlap_captions():
    p=project();p.cuts=[Cut(start=5,end=7,approved=True)]
    p.captions=[Caption(start=3,end=8,speaker='A',text='ଓଡ଼ିଆ'),Caption(start=4,end=9,speaker='B',text='ପରୀକ୍ଷା')]
    assert time_map(p)[1]['source_start']==7 and time_map(p)[1]['edited_start']==5
    text=srt(p)
    assert 'A: ଓଡ଼ିଆ\nB: ପରୀକ୍ଷା' in text
    assert '00:00:03,000' in text and '00:00:07,000' in text

def test_locked_shot_and_missing_camera():
    p=project();p.activity=[Activity(start=0,end=20,speaker='SPEAKER_01')]
    lock=Shot(start=5,end=9,layout='both',locked=True);p.shots=[lock]
    result=generate_shots(p);kept=next(s for s in result if s.id==lock.id)
    assert (kept.start,kept.end,kept.layout,kept.locked)==(5,9,'both',True)
    p.shots=result;plan=render_plan(p)
    assert plan[0]['layout']=='A' and 'missing_footage' in plan[0]['warnings']

def test_bad_channel_mapping_rejected():
    data=project().model_dump();data['settings']['speaker_channel']={'SPEAKER_00':2}
    with pytest.raises(ValueError):Project.model_validate(data)

def test_reviewed_reference_metrics():
    p=project();p.activity=[Activity(start=0,end=5,speaker='SPEAKER_00')]
    p.captions=[Caption(start=0,end=5,speaker='SPEAKER_00',text='one wrong')]
    reference=p.model_copy(deep=True);reference.captions[0].text='one word';reference.captions[0].reviewed=True
    assert compare(p,reference)['word_error_rate']==.5
    assert compare(p,reference)['speech_missed_seconds']==0
    reference.captions[0].reviewed=False
    assert compare(p,reference)['word_error_rate'] is None


def test_review_import_preserves_project_identity(client):
    p=client.post('/v1/projects',json={'name':'review import'}).json()
    p['name']='corrected review'
    response=client.post('/v1/projects/'+p['id']+'/review',json={'format':'betun-review-v1','project':p,'baseline':p,'waveform':None})
    assert response.status_code==200
    assert response.json()['name']=='corrected review'
    assert response.json()['revision']==1
    assert client.post('/v1/projects/'+p['id']+'/review',json={'format':'betun-review-v1','project':p}).status_code==409


def test_setup_status_distinguishes_configured_from_installed(client, tmp_path, monkeypatch):
    import sys
    from betun import api
    model = tmp_path/'missing-model'
    monkeypatch.setattr(api, 'configuration', lambda: {'diarize':dict(model_path=str(model), python=sys.executable)})
    status = client.get('/v1/capabilities').json()
    assert status['configured_providers'] == {'diarize':'external-command'}
    assert status['installed_providers'] == {'diarize':False,'transcribe':False}
    assert not status['ready']
    model.mkdir()
    (model/'config.json').write_text('{}')
    assert client.get('/v1/capabilities').json()['installed_providers']['diarize']
