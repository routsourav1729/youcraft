import json
import threading
import time
from .media import Cancelled
from .pipeline import process
from .schema import Project, uid


class Queue:
    """One durable worker: no Redis, broker, daemon farm, or GPU overcommit."""
    def __init__(self, store):
        self.store = store
        self.stopping = threading.Event()
        self.wake = threading.Event()
        self.thread = None
        self.lockfile = None

    def start(self):
        self.lockfile = (self.store.root / 'worker.lock').open('a+b')
        try:
            import os
            if os.name == 'nt':
                import msvcrt
                self.lockfile.seek(0)
                self.lockfile.write(b'0')
                self.lockfile.flush()
                self.lockfile.seek(0)
                msvcrt.locking(self.lockfile.fileno(), msvcrt.LK_NBLCK, 1)
            else:
                import fcntl
                fcntl.flock(self.lockfile, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except (OSError, BlockingIOError):
            self.lockfile.close()
            raise RuntimeError('Another Betun backend already uses this data directory')
        with self.store.connect() as db:
            db.execute("UPDATE jobs SET state='interrupted', error='Backend stopped; retry to resume from cache' WHERE state IN ('running','cancelling')")
        self.thread = threading.Thread(target=self.loop, name='betun-worker', daemon=True)
        self.thread.start()

    def stop(self):
        self.stopping.set()
        self.wake.set()
        if self.thread:
            self.thread.join(timeout=15)
        if self.lockfile and (not self.thread or not self.thread.is_alive()):
            self.lockfile.close()

    def submit(self, pid, kind, options):
        project = self.store.get(pid)
        job_id = uid()
        payload = json.dumps(dict(project=project.model_dump(), options=options))
        with self.store.connect() as db:
            db.execute('INSERT INTO jobs VALUES(?,?,?,?,?,?,?,?,?)',
                       (job_id, pid, kind, 'queued', 0., payload, None, None, time.time()))
        self.wake.set()
        return self.get(job_id)

    def get(self, job_id):
        with self.store.connect() as db:
            row = db.execute('SELECT * FROM jobs WHERE id=?', (job_id,)).fetchone()
        if row is None:
            raise KeyError(job_id)
        data = dict(row)
        data.pop('payload')
        if data['result']:
            data['result'] = json.loads(data['result'])
        return data

    def list(self, pid):
        with self.store.connect() as db:
            ids = [r[0] for r in db.execute('SELECT id FROM jobs WHERE project_id=? ORDER BY created DESC', (pid,))]
        return [self.get(i) for i in ids]

    def cancel(self, job_id):
        self.get(job_id)
        with self.store.connect() as db:
            db.execute("UPDATE jobs SET state=CASE WHEN state='queued' THEN 'cancelled' ELSE 'cancelling' END WHERE id=? AND state IN ('queued','running')", (job_id,))
        return self.get(job_id)

    def retry(self, job_id):
        with self.store.connect() as db:
            row = db.execute('SELECT * FROM jobs WHERE id=?', (job_id,)).fetchone()
        if row is None:
            raise KeyError(job_id)
        if row['state'] not in ('failed', 'cancelled', 'interrupted'):
            raise ValueError('Only failed, cancelled or interrupted jobs can be retried')
        return self.submit(row['project_id'], row['kind'], json.loads(row['payload'])['options'])

    def loop(self):
        while not self.stopping.is_set():
            with self.store.connect() as db:
                db.execute('BEGIN IMMEDIATE')
                row = db.execute("SELECT * FROM jobs WHERE state='queued' ORDER BY created LIMIT 1").fetchone()
                if row:
                    db.execute("UPDATE jobs SET state='running' WHERE id=?", (row['id'],))
            if not row:
                self.wake.wait(1)
                self.wake.clear()
                continue
            jid = row['id']
            def cancelled():
                return self.stopping.is_set() or self.get(jid)['state'] == 'cancelling'
            def progress(value):
                with self.store.connect() as db:
                    db.execute('UPDATE jobs SET progress=? WHERE id=?', (value, jid))
            try:
                payload = json.loads(row['payload'])
                project = Project.model_validate(payload['project'])
                directory = self.store.directory(project.id) / 'jobs' / jid
                directory.mkdir(parents=True, exist_ok=True)
                if cancelled():
                    raise Cancelled()
                result = process(self.store, project, row['kind'], payload['options'], directory, cancelled, progress)
                state, error = 'succeeded', None
            except Cancelled:
                state, result, error = ('interrupted' if self.stopping.is_set() else 'cancelled'), None, None
            except Exception as exc:
                state, result, error = 'failed', None, f'{type(exc).__name__}: {exc}'
            with self.store.connect() as db:
                db.execute('UPDATE jobs SET state=?,result=?,error=?,progress=? WHERE id=?',
                           (state, json.dumps(result) if result is not None else None, error, 1. if state == 'succeeded' else self.get(jid)['progress'], jid))
