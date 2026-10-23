import numpy as np
import pytest
from betun import media, pipeline, sync
from betun.podcast import review_manifest, TICKS
from betun.schema import Clip, Project
from betun.store import Store


def episode():
    return Project(name='Independent shards', duration=3600, clips=[
        Clip(id='master',path='master.wav',camera='master',duration=3600,fingerprint='m'),
        Clip(id='late',path='late.mp4',camera='A',duration=900,offset=2400,fingerprint='l',has_video=True,sync_score=40),
        Clip(id='early',path='early.mp4',camera='A',duration=900,offset=10,fingerprint='e',has_video=True,sync_score=41),
        Clip(id='guest',path='guest.mp4',camera='B',duration=1200,offset=1800,fingerprint='g',has_video=True)])


def test_selected_shard_preview_has_only_its_picture_and_continuous_master():
    p=episode(); before=p.model_dump()
    plan=review_manifest(p, clip_id='late')
    assert plan['master']=='master'
    assert plan['clips'][0]['start']==0 and plan['clips'][-1]['end']==3600*TICKS
    pictures=[v for span in plan['clips'] for v in span['video']]
    assert pictures and all(v['asset']=='late' and v['side']=='full' for v in pictures)
    first=next(span for span in plan['clips'] if span['video'])
    assert first['start']==2400*TICKS and first['video'][0]['source_in']==0
    assert p.model_dump()==before
    with pytest.raises(ValueError,match='not found'):
        review_manifest(p,clip_id='missing')


def test_sync_searches_each_shard_independently_and_clears_stale_failure(tmp_path,monkeypatch):
    store=Store(tmp_path);p=episode();store.create(p)
    monkeypatch.setattr(media,'playback_audio',lambda *a,**kw: None)
    monkeypatch.setattr(media,'audio_copy',lambda clip,*a,**kw:clip.id)
    monkeypatch.setattr(media,'read_wav',lambda path:(np.zeros(20,dtype=np.float32),8000))
    calls=[]
    def match(*args,**kwargs):
        assert kwargs.get('minimum_offset') is None
        calls.append(kwargs)
        if len(calls)==2: raise ValueError('No reliable match')
        return dict(offset=2400 if len(calls)==1 else 1800,rate=1.,score=40.,residual_seconds=.002,
                    inlier_count=4,anchors=[(0,0,40,2)]*5,warnings=['sync_review_required'])
    monkeypatch.setattr(sync,'synchronize',match)
    pipeline.process(store,p,'sync',{},tmp_path,lambda:False,lambda _:None)
    saved=store.get(p.id)
    assert len(calls)==3
    assert saved.clips[1].sync_anchor_count==4
    assert saved.clips[1].sync_residual_seconds==.002
    assert saved.clips[2].sync_score is None and saved.clips[2].sync_anchor_count==0
    assert 'manual_sync_required' in saved.clips[2].warnings
    assert saved.clips[3].offset==1800
