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


def test_catalog_reports_persistent_project_location(tmp_path):
    from fastapi.testclient import TestClient
    from betun.api import create_app
    root = tmp_path / 'data'
    with TestClient(create_app(root=root, token='stage-one-test-token')) as client:
        headers = {'Authorization': 'Bearer stage-one-test-token'}
        created = client.post('/v1/projects', json={'name': 'Stage one'}, headers=headers).json()
        catalog = client.get('/v1/projects', headers=headers).json()
        directory = catalog[0]['project_folder']
        locations = client.get('/v1/storage', headers=headers).json()
        assert locations['projects_home'] == str(root / 'projects')
    with TestClient(create_app(root=root, token='stage-one-test-token')) as client:
        catalog = client.get('/v1/projects', headers=headers).json()
        assert catalog[0]['id'] == created['id']
        assert catalog[0]['project_folder'] == directory


def test_default_home_is_persistent_and_existing_projects_stay_put(tmp_path):
    store = Store(tmp_path/'data')
    old = store.create(Project(name='First'))
    original = store.directory(old.id)
    home = tmp_path/'chosen'
    store.set_project_home(str(home))
    reopened = Store(tmp_path/'data')
    assert reopened.default_project_home() == home
    assert reopened.directory(old.id) == original
    new = reopened.create(Project(name='Interview: Friday / CON'))
    directory = reopened.directory(new.id)
    assert directory.parent == home and 'Interview' in directory.name
    assert ':' not in directory.name and '/' not in directory.name
    assert (directory/'project.betun.json').is_file()
    with pytest.raises(ValueError, match='absolute'):
        reopened.set_project_home('relative-folder')
