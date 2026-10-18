import pytest
from betun.schema import Project, Caption, Clip
from betun.store import Store
from betun.jobs import Queue


def test_saved_odia_text_survives_new_store_and_portable_snapshot(tmp_path):
    store = Store(tmp_path/'data')
    p = store.create(Project(name='Saved episode', duration=10))
    p.captions = [Caption(start=0,end=3,speaker='host',text='ନମସ୍କାର',reviewed=False)]
    saved = store.save(p, p.revision)
    reopened = Store(tmp_path/'data').get(p.id)
    assert reopened.captions[0].text == 'ନମସ୍କାର'
    assert reopened.revision == saved.revision
    assert store.storage(p.id)['backup_current']
    backup = Project.model_validate_json((store.directory(p.id)/'project.betun.json').read_text(encoding='utf-8'))
    assert backup.captions == reopened.captions


def test_move_and_delete_never_remove_original_media(tmp_path):
    source = tmp_path/'original.wav'; source.write_bytes(b'original')
    store = Store(tmp_path/'data')
    p = store.create(Project(name='Move', duration=10,clips=[Clip(path=str(source),camera='master',duration=10,fingerprint='original')]))
    old = store.directory(p.id)
    (old/'cache').mkdir(); (old/'cache'/'generated.wav').write_bytes(b'cache')
    moved = store.move_directory(p.id,str(tmp_path/'chosen'))
    assert moved['previous_path'] == str(old)
    assert store.directory(p.id) == tmp_path/'chosen'/p.id
    (store.directory(p.id)/'personal-note.txt').write_text('keep')
    store.delete(p.id,p.revision)
    assert source.read_bytes() == b'original'
    assert (tmp_path/'chosen'/p.id/'personal-note.txt').read_text() == 'keep'
    with pytest.raises(KeyError): store.get(p.id)


def test_cannot_delete_project_with_pending_job(tmp_path):
    store=Store(tmp_path)
    p=store.create(Project(name='Busy'))
    Queue(store).submit(p.id,'transcribe',{})
    with pytest.raises(ValueError,match='Finish or cancel'): store.delete(p.id,p.revision)
    assert store.get(p.id).id == p.id


def test_progress_file_lock_does_not_fail_processing(tmp_path, monkeypatch):
    import time
    from pathlib import Path
    store = Store(tmp_path)
    project = store.create(Project(name='Progress lock'))
    original = Path.replace
    def replace(path, target):
        if path.name == 'progress.tmp':
            raise PermissionError('Windows reader temporarily holds progress.json')
        return original(path, target)
    monkeypatch.setattr(Path, 'replace', replace)
    def process(store, project, kind, options, directory, cancel, progress):
        progress({'stage':'Transcribing', 'fraction':.5})
        return {'ok':True}
    monkeypatch.setattr('betun.jobs.process', process)
    queue = Queue(store)
    queue.start()
    try:
        job = queue.submit(project.id, 'transcribe', {})
        deadline = time.monotonic() + 5
        while time.monotonic() < deadline:
            job = queue.get(job['id'])
            if job['state'] not in ('queued','running'):
                break
            time.sleep(.02)
        assert job['state'] == 'succeeded', job
    finally:
        queue.stop()
