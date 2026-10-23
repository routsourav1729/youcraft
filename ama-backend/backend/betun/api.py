import hmac
import os
from pathlib import Path
import secrets
from contextlib import asynccontextmanager
from typing import Literal
from fastapi import FastAPI, HTTPException, Request
from fastapi.responses import FileResponse, JSONResponse, PlainTextResponse
from fastapi.staticfiles import StaticFiles
from fastapi.middleware.trustedhost import TrustedHostMiddleware
from pydantic import Field, ValidationError
from . import __version__
from .schema import Model, Project
from .store import Store, Conflict
from .jobs import Queue
from .timeline import time_map, render_plan, srt
from .providers import configuration
from .media import binary, fingerprint


class CreateProject(Model):
    name: str = Field(min_length=1, max_length=200)


class ReviewFile(Model):
    format: Literal['betun-review-v1']
    project: Project
    baseline: Project | None = None
    waveform: dict | None = None
    audio: str | None = None


class StorageFolder(Model):
    path: str = Field(min_length=1, max_length=32768)


class Revision(Model):
    revision: int = Field(ge=0)


class JobRequest(Model):
    kind: Literal['import', 'attach_cameras', 'sample', 'sync_preview', 'reference_frame', 'filmcraft_export', 'reaction_preview', 'proxies', 'reactions', 'reference_suggestions', 'sync', 'waveform', 'diarize', 'transcribe', 'shots', 'translate', 'transliterate', 'preview', 'export']
    options: dict = Field(default_factory=dict)


class ImportOptions(Model):
    master: str
    A: list[str] = Field(default_factory=list, max_length=1000)
    B: list[str] = Field(default_factory=list, max_length=1000)


class PreviewOptions(Model):
    start: float = Field(default=0, ge=0)
    end: float | None = Field(default=None, gt=0)


class CameraOptions(Model):
    A: list[str] = Field(default_factory=list, max_length=1000)
    B: list[str] = Field(default_factory=list, max_length=1000)


class DiarizeOptions(Model):
    replace_activity: bool = False


class FilmcraftExportOptions(Model):
    snapshot: dict
    sequence: int = Field(gt=0)


class ReactionPreviewOptions(Model):
    reaction_id: str


class ReferenceOptions(Model):
    clip_id: str
    time: float = Field(ge=0)


class SyncPreviewOptions(Model):
    clip_id: str
    position: float = Field(default=0.5, ge=0, le=1)


class ReactionOptions(Model):
    references: dict[str, float] = Field(default_factory=dict)
    tolerance: float = Field(default=10, gt=0, le=45)
    interval: float = Field(default=60, ge=10, le=600)
    spacing: float = Field(default=15, ge=0, le=120)


class TextOptions(Model):
    text: str = Field(min_length=1, max_length=10000)


def data_root():
    if os.environ.get('BETUN_DATA_DIR'):
        return Path(os.environ['BETUN_DATA_DIR'])
    if os.name == 'nt':
        return Path(os.environ.get('LOCALAPPDATA', Path.home())) / 'Betun'
    import sys
    if sys.platform == 'darwin':
        return Path.home() / 'Library' / 'Application Support' / 'Betun'
    return Path(os.environ.get('XDG_DATA_HOME', Path.home() / '.local' / 'share')) / 'betun'


def create_app(root=None, token=None):
    store = Store(Path(root) if root else data_root(), user_folders=root is None and not os.environ.get("BETUN_DATA_DIR"))
    token_path = store.root / 'api-token'
    if token is None:
        token = os.environ.get('BETUN_TOKEN')
        if not token:
            if not token_path.exists():
                fd = os.open(token_path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
                with os.fdopen(fd, 'w') as f:
                    f.write(secrets.token_urlsafe(32))
            token = token_path.read_text(encoding='utf-8').strip()
    if not token or len(token) < 16:
        raise ValueError('API token must have at least 16 characters')
    queue = Queue(store)

    @asynccontextmanager
    async def lifespan(app):
        queue.start()
        yield
        queue.stop()

    app = FastAPI(title='Betun local backend', version=__version__, lifespan=lifespan)
    app.state.store, app.state.queue = store, queue
    app.add_middleware(TrustedHostMiddleware, allowed_hosts=['127.0.0.1', 'localhost', '[::1]', 'testserver'])

    @app.middleware('http')
    async def authenticate(request: Request, call_next):
        if request.url.path not in ('/', '/review-data.js', '/health', '/docs', '/openapi.json', '/docs/oauth2-redirect') and not request.url.path.startswith('/ui/'):
            candidate = request.headers.get('authorization', '')
            cookie = request.cookies.get('betun_session', '')
            bearer_ok = hmac.compare_digest(candidate.encode(), ('Bearer ' + token).encode())
            cookie_ok = hmac.compare_digest(cookie.encode(), token.encode())
            if cookie_ok and not bearer_ok and request.method not in ('GET', 'HEAD'):
                origin = request.headers.get('origin')
                expected = str(request.base_url).rstrip('/')
                if request.headers.get('x-betun-review') != '1' or (origin and origin != expected):
                    return JSONResponse(status_code=403, content={'detail': 'Same-origin review request required'})
            if not bearer_ok and not cookie_ok:
                return JSONResponse(status_code=401, content={'detail': 'Bearer token required'})
        # No wildcard CORS. The future desktop wrapper should proxy local requests or
        # explicitly authorize its own origin; websites cannot use ambient cookies.
        response = await call_next(request)
        response.headers['Cache-Control'] = 'no-store'
        return response

    @app.exception_handler(KeyError)
    async def missing(request, error):
        return JSONResponse(status_code=404, content={'detail': 'Project, job or artifact not found'})

    @app.exception_handler(Conflict)
    async def conflict(request, error):
        return JSONResponse(status_code=409, content={'detail': str(error)})

    @app.exception_handler(ValueError)
    async def invalid(request, error):
        return JSONResponse(status_code=422, content={'detail': str(error)})

    ui_directory = Path(__file__).with_name('ui')
    app.mount('/ui', StaticFiles(directory=ui_directory), name='ui')

    @app.get('/')
    def review_ui():
        return FileResponse(ui_directory / 'index.html')

    @app.get('/review-data.js')
    def review_dataset():
        return FileResponse(ui_directory / 'review-data.js', media_type='application/javascript')

    @app.post('/v1/session')
    def session():
        response = JSONResponse({'connected': True})
        response.set_cookie('betun_session', token, httponly=True, samesite='strict', max_age=86400)
        return response

    @app.get('/health')
    def health():
        return dict(status='ok', service='youcraft', protocol_version=1, version=__version__, schema_version=1)

    @app.get('/v1/capabilities')
    def capabilities():
        tools = {}
        for name in ('ffmpeg', 'ffprobe'):
            try:
                binary(name)
                tools[name] = True
            except RuntimeError:
                tools[name] = False
        configured = configuration()
        installed = {}
        for task in ('diarize', 'transcribe'):
            entry = configured.get(task, {})
            command = entry.get('command', [])
            if command:
                installed[task] = bool(isinstance(command, list) and Path(command[0]).is_file())
            else:
                model = Path(entry.get('model_path', ''))
                installed[task] = bool(entry.get('model_path') and model.is_dir()
                    and (model / 'config.json').is_file() and Path(entry.get('python', '')).is_file())
        vision = configured.get('reactions', {})
        installed['reactions'] = bool(vision.get('face_model') and Path(vision['face_model']).is_file() and Path(vision.get('python', '')).is_file())
        return dict(media=tools, installed_providers=installed, reaction_provider=vision.get('provider'),
                    ready=all(tools.values()) and all(installed.get(task, False) for task in ('diarize', 'transcribe')),
                    configured_providers={k: v.get('provider', 'external-command') for k, v in configured.items()},
                    model_status='configured does not mean installed or validated', remote_inference=False)

    @app.post('/v1/projects', status_code=201)
    def create(body: CreateProject):
        return store.create(Project(name=body.name))

    @app.get('/v1/projects')
    def projects():
        return [dict(id=p['id'], name=p['name'], revision=p['revision'], duration=p['duration'], project_folder=str(store.directory(p['id']))) for p in store.list()]

    @app.get('/v1/projects/{pid}')
    def get(pid: str):
        return store.get(pid)

    @app.put('/v1/projects/{pid}')
    def save(pid: str, body: Project):
        old = store.get(pid)
        if pid != body.id:
            raise ValueError('Project ID cannot change')
        protected = ('id', 'path', 'camera', 'duration', 'channels', 'has_video', 'frame_rate', 'fingerprint')
        original = {c.id: tuple(getattr(c, f) for f in protected) for c in old.clips}
        changed = {c.id: tuple(getattr(c, f) for f in protected) for c in body.clips}
        if original != changed:
            raise ValueError('Source media metadata is immutable; use project restore for relinking')
        # A timing change invalidates prior review unless the client explicitly reviews it
        # in a subsequent save. Camera locks remain an explicit user action.
        for clip in body.clips:
            previous = next(c for c in old.clips if c.id == clip.id)
            if (clip.offset, clip.rate) != (previous.offset, previous.rate):
                clip.sync_reviewed = False
        for caption in body.captions:
            previous = next((c for c in old.captions if c.id == caption.id), None)
            if previous and (caption.text, caption.start, caption.end, caption.speaker) != (previous.text, previous.start, previous.end, previous.speaker):
                caption.translation = None
        if body.settings.roles_locked:
            speakers = {a.speaker for a in body.activity}
            if len(speakers) != 2 or body.settings.host_speaker not in speakers:
                raise ValueError('Lock requires exactly two reviewed speakers and a host')
            if set(body.settings.speaker_camera.get(s) for s in speakers) != {'A', 'B'}:
                raise ValueError('Assign the two speakers to distinct cameras')
        if body.activity != old.activity:
            for caption in body.captions:
                caption.reviewed = False
        if body.activity != old.activity or body.settings != old.settings:
            body.reactions = []
            body.shots = [s for s in body.shots if s.locked]
        if any((c.offset,c.rate) != (next(x for x in old.clips if x.id==c.id).offset,
                                    next(x for x in old.clips if x.id==c.id).rate) for c in body.clips):
            body.reactions = []
            body.shots = [s for s in body.shots if s.locked]
        return store.save(body, body.revision)

    @app.post('/v1/projects/restore', status_code=201)
    def restore(body: Project):
        from .schema import uid
        for clip in body.clips:
            if fingerprint(clip.path) != clip.fingerprint:
                raise ValueError('Missing or changed source media; restore at the original paths first')
        body.id, body.revision = uid(), 0
        return store.create(body)

    @app.post('/v1/projects/{pid}/undo')
    def undo(pid: str, body: Revision):
        return store.undo(pid, body.revision)

    @app.post('/v1/projects/{pid}/review')
    def apply_review(pid: str, body: ReviewFile):
        return save(pid, body.project)

    @app.post('/v1/projects/{pid}/evaluate')
    def evaluate(pid: str, baseline: Project):
        from .evaluation import compare
        return compare(baseline, store.get(pid))

    @app.get('/v1/projects/{pid}/waveform')
    def waveform_file(pid: str):
        directory = store.directory(pid)
        path = directory / 'trial' / 'waveform' / 'waveform.json'
        if path.is_file():
            return FileResponse(path)
        for job in queue.list(pid):
            if job['kind'] == 'waveform' and job['state'] == 'succeeded':
                candidate = directory / 'jobs' / job['id'] / 'waveform.json'
                if candidate.is_file():
                    return FileResponse(candidate)
        raise KeyError('waveform')

    @app.get('/v1/projects/{pid}/baseline')
    def baseline(pid: str):
        path = store.directory(pid) / 'baseline.json'
        if not path.is_file():
            raise KeyError('baseline')
        return Project.model_validate_json(path.read_text(encoding='utf-8'))

    @app.get('/v1/storage')
    def storage_locations():
        home = store.default_project_home()
        home.mkdir(parents=True, exist_ok=True)
        return dict(projects_home=str(home), database=str(store.db), data_root=str(store.root))

    @app.get('/v1/projects/{pid}/storage')
    def project_storage(pid: str):
        return store.storage(pid)

    @app.post('/v1/projects/{pid}/storage')
    def move_project_storage(pid: str, body: StorageFolder):
        return store.move_directory(pid, body.path)

    @app.delete('/v1/projects/{pid}')
    def delete_project(pid: str, body: Revision):
        return store.delete(pid, body.revision)

    @app.get('/v1/projects/{pid}/speaker-samples')
    def samples(pid: str):
        from .podcast import speaker_samples
        return speaker_samples(store.get(pid))

    def playback_plan(pid, plan):
        from . import media
        project = store.get(pid)
        master = next(c for c in project.clips if c.camera == 'master')
        path = media.playback_audio(master, store.directory(pid) / 'cache')
        for asset in plan['assets']:
            if asset['id'] == master.id:
                asset['path'] = str(path)
                asset['fingerprint'] = media.fingerprint(path)
        return plan

    @app.get('/v1/projects/{pid}/filmcraft')
    def filmcraft(pid: str):
        from .podcast import manifest
        return playback_plan(pid, manifest(store.get(pid)))

    @app.get('/v1/projects/{pid}/review-plan')
    def review_plan(pid: str, edit: bool = False, clip_id: str | None = None):
        from .podcast import review_manifest
        return playback_plan(pid, review_manifest(store.get(pid), edit, clip_id))

    @app.get('/v1/projects/{pid}/proxies/{filename}')
    def proxy(pid: str, filename: str):
        project = store.get(pid)
        allowed = {f'{c.fingerprint}-preview-v1' + ('.mp4' if c.has_video else '.wav') for c in project.clips}
        if filename not in allowed:
            raise KeyError(filename)
        path = store.directory(pid) / 'cache' / filename
        if not path.is_file():
            raise KeyError(filename)
        return FileResponse(path)

    @app.get('/v1/server-media')
    def server_media(path: str = ''):
        # Explicitly configured root; never browse unrelated server directories.
        configured = os.environ.get('BETUN_MEDIA_ROOT')
        if not configured:
            raise ValueError('Set BETUN_MEDIA_ROOT to enable server media browsing')
        root = Path(configured).resolve(strict=True)
        directory = (root / path).resolve(strict=True)
        if not directory.is_relative_to(root) or not directory.is_dir():
            raise ValueError('Directory outside media root')
        entries = sorted(directory.iterdir(), key=lambda p: (not p.is_dir(),p.name.lower()))
        return [dict(name=p.name, relative=str(p.relative_to(root)), path=str(p), directory=p.is_dir())
                for p in entries[:1000] if p.resolve().is_relative_to(root)]

    @app.put('/v1/uploads/{filename}', status_code=201)
    async def upload(filename: str, request: Request):
        from .schema import uid
        if Path(filename).name != filename or len(filename)>200:
            raise ValueError('Invalid upload name')
        folder = store.root / 'uploads'
        folder.mkdir(exist_ok=True)
        destination = folder / (uid() + '-' + filename)
        partial = destination.with_suffix(destination.suffix + '.partial')
        total = 0
        try:
            with partial.open('wb') as out:
                async for chunk in request.stream():
                    total += len(chunk)
                    if total > 200 * 1024**3:
                        raise ValueError('Upload exceeds 200 GB')
                    out.write(chunk)
            if not total:
                raise ValueError('Empty upload')
            partial.replace(destination)
        finally:
            partial.unlink(missing_ok=True)
        return dict(path=str(destination), bytes=total)

    @app.get('/v1/projects/{pid}/timeline')
    def timeline(pid: str):
        project = store.get(pid)
        return dict(time_map=time_map(project), segments=render_plan(project))

    @app.get('/v1/projects/{pid}/captions.srt', response_class=PlainTextResponse)
    def captions(pid: str):
        return srt(store.get(pid))

    @app.get('/v1/projects/{pid}/project.json')
    def project_file(pid: str):
        return store.get(pid)

    @app.get('/v1/projects/{pid}/media/{clip_id}')
    def source_media(pid: str, clip_id: str):
        clip = next((c for c in store.get(pid).clips if c.id == clip_id), None)
        if clip is None or not Path(clip.path).is_file():
            raise KeyError(clip_id)
        return FileResponse(clip.path)

    @app.post('/v1/projects/{pid}/jobs', status_code=202)
    def submit(pid: str, body: JobRequest):
        options_class = {'import': ImportOptions, 'attach_cameras': CameraOptions, 'sample': PreviewOptions, 'preview': PreviewOptions, 'diarize': DiarizeOptions, 'transliterate': TextOptions, 'reactions': ReactionOptions, 'reference_suggestions': ReactionOptions, 'filmcraft_export': FilmcraftExportOptions, 'reaction_preview': ReactionPreviewOptions, 'reference_frame': ReferenceOptions, 'sync_preview': SyncPreviewOptions}.get(body.kind, Model)
        options = options_class.model_validate(body.options).model_dump()
        if body.kind in ('preview', 'sample') and options['end'] is not None and options['end'] <= options['start']:
            raise ValueError('Preview end must be after start')
        return queue.submit(pid, body.kind, options)

    @app.get('/v1/projects/{pid}/jobs')
    def jobs(pid: str):
        store.get(pid)
        return queue.list(pid)

    @app.get('/v1/jobs/{jid}')
    def job(jid: str):
        return queue.get(jid)

    @app.post('/v1/jobs/{jid}/cancel')
    def cancel(jid: str):
        return queue.cancel(jid)

    @app.post('/v1/jobs/{jid}/retry', status_code=202)
    def retry(jid: str):
        return queue.retry(jid)

    @app.get('/v1/jobs/{jid}/files/{filename}')
    def artifact(jid: str, filename: str):
        job = queue.get(jid)
        if job['state'] != 'succeeded' or filename not in (job['result'] or {}).get('files', []):
            raise KeyError(filename)
        if Path(filename).name != filename:
            raise KeyError(filename)
        path = store.directory(job['project_id']) / 'jobs' / jid / filename
        if not path.is_file():
            raise KeyError(filename)
        return FileResponse(path)

    return app
