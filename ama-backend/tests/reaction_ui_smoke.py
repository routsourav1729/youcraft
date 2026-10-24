"""Installed Windows reaction workspace smoke; synthetic footage, no model download."""
import hashlib
import json
import os
from pathlib import Path
import socket
import time


def control_token():
    explicit = os.environ.get("FILMCRAFT_CONTROL_TOKEN")
    if explicit:
        return explicit
    root = Path(os.environ.get("BETUN_DATA_DIR", str(Path(os.environ["LOCALAPPDATA"]) / "Betun")))
    secret = (root / "api-token").read_text(encoding="utf-8").strip()
    return hashlib.sha256(b"youcraft-control-v1\0" + secret.encode()).hexdigest()


def ask(method, **params):
    with socket.create_connection(('127.0.0.1',9876),timeout=30) as connection:
        connection.sendall((json.dumps(dict(method=method,params=params,token=control_token()))+'\n').encode())
        result=json.loads(connection.makefile('rb').readline())
    assert result.get('ok'),result
    return result['result']


def ready(predicate=lambda s:True):
    deadline=time.monotonic()+60
    while time.monotonic()<deadline:
        state=ask('podcast.inspect')
        if not state['busy'] and predicate(state):
            assert not state['error'],state['error']
            return state
        time.sleep(.2)
    raise AssertionError(state)


media=Path(os.environ['RUNNER_TEMP'])/'youcraft-media'
ready()
ask('podcast.configure',name='Reaction layout check',master=str(media/'master.wav'),camera_a=[str(media/'host.mp4')],camera_b=[str(media/'guest.mp4')])
ask('podcast.action',action='create',options={'identify':False})
ready(lambda s:s['project'].get('duration',0)>0)
# A voice audition must stay on Speakers before any camera is attached.
ask('podcast.review',activity=[dict(id='sample-host',start=0,end=4,speaker='host')])
ready()
ask('ui.set',podcastStep=2)
ask('podcast.action',action='samples')
ready(lambda s:bool(s['samples']))
ask('ui.click',id='podcast.step.4')
assert ask('podcast.inspect')['step']==2, 'Transcript opened before cameras were ready'
ask('ui.click',id='podcast.listen.host')
ready()
ui=ask('ui.inspect')
assert ui['ui']['workspace']=='Podcast' and ui['ui']['focused']=='Podcast',ui
assert any(e['label']=='Stop sample' for e in ask('ui.elements',prefix='podcast.listen.'))
ask('ui.click',id='podcast.listen.host')
assert not ask('ui.inspect')['sourcePlayback']['playing']
ask('podcast.action',action='attach_cameras')
state=ready(lambda s:len(s['project']['clips'])==3)
p=state['project'];settings=p['settings']
settings.update(host_speaker='host',roles_locked=True,speaker_camera={'host':'A','guest':'B'},overlap_padding=2)
for clip in p['clips']:
    if clip['camera']!='master':clip['sync_reviewed']=True
ask('podcast.review',settings=settings,clips=p['clips'],activity=[dict(id='h',start=0,end=1.5,speaker='host'),dict(id='g',start=1.5,end=4,speaker='guest')])
ready()
host=next(c['id'] for c in p['clips'] if c['camera']=='A')
ask('podcast.review',shots=[dict(id='s1',start=0,end=1.5,layout='A'),dict(id='s2',start=1.5,end=4,layout='B')],reactions=[dict(id='candidate',clip_id=host,start=2,end=3,score=.8,cues=['smile'],decision='pending')])
ready()
ask('ui.set',podcastStep=5)
ask('podcast.action',action='review_playback',options={'edit':True})
ready(lambda s:s['status'].startswith('Review ready'))
ask('ui.resize',width=1366,height=768)
time.sleep(1)
notices=ask('ui.elements',prefix='graphics.notice')
if any(e['id']=='graphics.notice.close' for e in notices):ask('ui.click',id='graphics.notice.close')
elements=ask('ui.elements',prefix='podcast.')
ids={e['id'] for e in elements}
for name in ['video_height','suggest_references','reference_current','reference_image']:
    assert 'podcast.'+name in ids,(name,ids)
transport={e['id'] for e in ask('ui.elements',prefix='program.transport.')}
assert 'program.transport.playback.toggle' in transport
assert not any('sequence.extract' in item or 'sequence.lift' in item for item in transport)
assert 'podcast.preview_suggestions' in ids
assert 'podcast.reaction_marker.candidate' in ids
ask('engine.execute',command='playhead.set',params={'seconds':2.1})
time.sleep(.5)
assert any(e['id']=='podcast.active_reaction_remove' for e in ask('ui.elements',prefix='podcast.'))
# Capture the installed product, not a separately rebuilt mock UI.
out=Path('release/ui-check');out.mkdir(parents=True,exist_ok=True)
ask('ui.screenshot',path=str((out/'reactions-1366.png').resolve()))
print('Installed reaction workspace: continuous review, resizable video area, horizontal controls and screenshot reference available.')
