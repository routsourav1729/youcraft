import json
import sqlite3
import os
import shutil
import logging
from contextlib import contextmanager
from contextlib import closing
from pathlib import Path
from .schema import Project


class Conflict(Exception):
    pass


class Store:
    def __init__(self, root: Path, user_folders=False):
        self.user_folders = user_folders
        self.root = root.resolve()
        self.root.mkdir(parents=True, exist_ok=True)
        self.db = self.root / "betun.sqlite3"
        with self.connect() as db:
            version = db.execute("PRAGMA user_version").fetchone()[0]
            if version > 1:
                raise RuntimeError("Database is newer than this app; upgrade Betun")
            db.executescript('''
                PRAGMA journal_mode=WAL;
                CREATE TABLE IF NOT EXISTS projects(id TEXT PRIMARY KEY, revision INTEGER, body TEXT);
                CREATE TABLE IF NOT EXISTS history(project_id TEXT, revision INTEGER, body TEXT,
                    PRIMARY KEY(project_id, revision));
                CREATE TABLE IF NOT EXISTS jobs(id TEXT PRIMARY KEY, project_id TEXT, kind TEXT,
                    state TEXT, progress REAL, payload TEXT, result TEXT, error TEXT, created REAL);
                CREATE TABLE IF NOT EXISTS project_folders(project_id TEXT PRIMARY KEY, path TEXT);
                PRAGMA user_version=1;
            ''')

    @contextmanager
    def connect(self):
        # The SQLite transaction context commits/rolls back, but does not close.
        with closing(sqlite3.connect(self.db, timeout=30)) as db, db:
            db.row_factory = sqlite3.Row
            yield db

    def create(self, project):
        with self.connect() as db:
            db.execute("INSERT INTO projects VALUES(?,?,?)", (project.id, project.revision, project.model_dump_json()))
        self.snapshot(project)
        return project

    def get(self, pid):
        with self.connect() as db:
            row = db.execute("SELECT body FROM projects WHERE id=?", (pid,)).fetchone()
        if row is None:
            raise KeyError(pid)
        return Project.model_validate_json(row[0])

    def list(self):
        with self.connect() as db:
            return [json.loads(r[0]) for r in db.execute("SELECT body FROM projects ORDER BY rowid DESC")]

    def save(self, project, expected):
        # Validate even internally mutated instances before committing.
        project = Project.model_validate(project.model_dump())
        with self.connect() as db:
            db.execute("BEGIN IMMEDIATE")
            row = db.execute("SELECT revision,body FROM projects WHERE id=?", (project.id,)).fetchone()
            if row is None:
                raise KeyError(project.id)
            if row[0] != expected:
                raise Conflict("Project changed; reload before saving or applying job result")
            db.execute("INSERT INTO history VALUES(?,?,?)", (project.id, row[0], row[1]))
            project.revision = expected + 1
            db.execute("UPDATE projects SET revision=?,body=? WHERE id=?", (project.revision, project.model_dump_json(), project.id))
        self.snapshot(project)
        return project

    def undo(self, pid, expected):
        with self.connect() as db:
            row = db.execute("SELECT body FROM history WHERE project_id=? AND revision=?", (pid, expected - 1)).fetchone()
        if row is None:
            raise ValueError("No previous revision")
        return self.save(Project.model_validate_json(row[0]), expected)

    def default_project_home(self):
        if os.name == 'nt' and self.user_folders:
            # Respect redirected Documents (including OneDrive), not a hardcoded username.
            import ctypes
            buffer = ctypes.create_unicode_buffer(32768)
            if ctypes.windll.shell32.SHGetFolderPathW(None, 5, None, 0, buffer) == 0:
                return Path(buffer.value) / 'YouCraft' / 'Projects'
        return self.root / 'projects'

    def directory(self, pid):
        self.get(pid)
        if len(pid) != 32 or any(c not in "0123456789abcdef" for c in pid):
            raise ValueError('Invalid project ID')
        with self.connect() as db:
            row = db.execute('SELECT path FROM project_folders WHERE project_id=?', (pid,)).fetchone()
            if row:
                path = Path(row[0])
            else:
                legacy = self.root / 'projects' / pid
                path = legacy if legacy.exists() else self.default_project_home() / pid
                db.execute('INSERT INTO project_folders VALUES(?,?)', (pid, str(path)))
        path.mkdir(parents=True, exist_ok=True)
        return path

    def snapshot(self, project):
        # SQLite is authoritative; a backup failure must not turn a committed save
        # into a misleading revision conflict on retry.
        try:
            directory = self.directory(project.id)
            temporary = directory / 'project.partial.json'
            temporary.write_text(project.model_dump_json(indent=2), encoding='utf-8')
            temporary.replace(directory / 'project.betun.json')
            (directory / 'backup-error.txt').unlink(missing_ok=True)
        except OSError as error:
            logging.error('Project backup failed for %s: %s', project.id, error)
            try:
                (directory / 'backup-error.txt').write_text(str(error), encoding='utf-8')
            except (OSError, UnboundLocalError):
                pass

    def storage(self, pid):
        directory = self.directory(pid)
        backup = directory / 'project.betun.json'
        project = self.get(pid)
        try:
            saved = json.loads(backup.read_text(encoding='utf-8'))
            current = saved.get('revision') == project.revision
        except (OSError, ValueError):
            current = False
        return dict(path=str(directory), backup_current=current)

    def move_directory(self, pid, parent):
        project = self.get(pid)
        source = self.directory(pid)
        parent = Path(parent).expanduser()
        if not parent.is_absolute():
            raise ValueError('Choose an absolute folder path')
        target = parent.resolve() / pid
        if target == source.resolve():
            return self.storage(pid)
        if target.is_relative_to(source.resolve()):
            raise ValueError('Choose a folder outside the current project')
        with self.connect() as db:
            db.execute('BEGIN IMMEDIATE')
            if db.execute("SELECT 1 FROM jobs WHERE project_id=? AND state IN ('queued','running','cancelling')", (pid,)).fetchone():
                raise ValueError('Finish or cancel processing before moving the project')
            if target.exists():
                raise ValueError('A project folder already exists at that destination')
            for clip in project.clips:
                if Path(clip.path).resolve().is_relative_to(source.resolve()):
                    raise ValueError('This folder contains source recordings; keep it in place')
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copytree(source, target)
            db.execute('UPDATE project_folders SET path=? WHERE project_id=?', (str(target), pid))
        # Keep the old folder as a recovery copy; never silently destroy a user's files.
        return dict(**self.storage(pid), previous_path=str(source))

    def delete(self, pid, expected):
        project = self.get(pid)
        directory = self.directory(pid)
        with self.connect() as db:
            db.execute('BEGIN IMMEDIATE')
            revision = db.execute('SELECT revision FROM projects WHERE id=?', (pid,)).fetchone()[0]
            if revision != expected:
                raise Conflict('Project changed; reopen it before deleting')
            if db.execute("SELECT 1 FROM jobs WHERE project_id=? AND state IN ('queued','running','cancelling')", (pid,)).fetchone():
                raise ValueError('Finish or cancel processing before deleting the project')
            if directory.is_symlink() or any(Path(c.path).resolve().is_relative_to(directory.resolve()) for c in project.clips):
                raise ValueError('Project folder contains source recordings; deletion stopped to preserve them')
            # Only app-owned entries, never arbitrary files the user placed here.
            for name in ('cache', 'jobs'):
                path = directory / name
                if path.is_symlink():
                    path.unlink()
                elif path.is_dir():
                    shutil.rmtree(path)
            for name in ('project.betun.json', 'project.partial.json', 'baseline.json', 'backup-error.txt'):
                (directory / name).unlink(missing_ok=True)
            for table in ('jobs', 'history', 'project_folders'):
                db.execute(f'DELETE FROM {table} WHERE project_id=?', (pid,))
            db.execute('DELETE FROM projects WHERE id=?', (pid,))
        if not any(directory.iterdir()):
            directory.rmdir()
        return dict(deleted=pid)
