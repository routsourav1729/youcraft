import json
import sqlite3
from contextlib import contextmanager
from pathlib import Path
from .schema import Project


class Conflict(Exception):
    pass


class Store:
    def __init__(self, root: Path):
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
                PRAGMA user_version=1;
            ''')

    @contextmanager
    def connect(self):
        with sqlite3.connect(self.db, timeout=30) as db:
            db.row_factory = sqlite3.Row
            yield db

    def create(self, project):
        with self.connect() as db:
            db.execute("INSERT INTO projects VALUES(?,?,?)", (project.id, project.revision, project.model_dump_json()))
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
        return project

    def undo(self, pid, expected):
        with self.connect() as db:
            row = db.execute("SELECT body FROM history WHERE project_id=? AND revision=?", (pid, expected - 1)).fetchone()
        if row is None:
            raise ValueError("No previous revision")
        return self.save(Project.model_validate_json(row[0]), expected)

    def directory(self, pid):
        self.get(pid)  # IDs originate in the database, never use arbitrary request paths.
        if len(pid) != 32 or any(c not in "0123456789abcdef" for c in pid):
            raise ValueError("Invalid project ID")
        path = self.root / "projects" / pid
        path.mkdir(parents=True, exist_ok=True)
        return path
