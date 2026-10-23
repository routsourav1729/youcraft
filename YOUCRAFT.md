# Betun — one-person Odia podcast editor

**YouCraft is a Windows-only, locally running customized Filmcraft editor.**
The Lenovo runs the editor and Python analysis helper together. CPU mode supports
testing without an NVIDIA GPU; remote processing is only an optional development aid. Filmcraft's
existing playback, timeline, trimming, project editing and export are the product
foundation. Add the podcast workflow inside that editor; do not build another
editor around the Python backend. The local upstream checkout is `vendor/filmcraft`.

The earlier [`desktop/`](desktop/README.md) standalone shell is a superseded
prototype, retained for reference. It is not the application architecture or
release target. The V1 integration now lives in `vendor/filmcraft`: a docked Podcast
workspace, native edit import, linked dialogue/camera cuts, transcript correction
and edited SRT export. It is a development build, not a validated Windows release.

## Core workflow contract

The opening animation leads to Projects. New project starts an episode; a saved
project reopens its persisted analysis and last workflow page. CPU/NVIDIA setup is
separate from project content. NVIDIA setup writes an explicit CUDA device;
legacy NVIDIA `auto` configurations are treated as CUDA, never silent CPU fallback.

Navigation is a view change. It does not regenerate transcription, synchronization,
reaction suggestions or camera edits, and does not rebuild a review sequence.
Continue from Cameras opens saved Transcript content. Generation/regeneration is
explicit; replacing existing generated results requires confirmation. Accepting an
existing transcript preserves saved camera edits. Review preview rebuilding remains
an explicit Reload action or a follow-up to a newly completed processing job.

Corrections autosave after a short idle interval. Each project gets its own atomic
recovery draft before a save; failed autosaves retry after five seconds. Reopening
recovers a draft when its base revision still matches. Revision conflicts never
silently overwrite newer work. A crash before the debounce interval may lose the
most recent keystrokes. SQLite persists analysis results; Filmcraft owns native edits.

Transcript review displays one segment in a three-line text area with Previous/Next
and playback following. Latin-to-Odia conversion is attached to that segment: select
a word to replace it, or leave the selection empty to replace the segment after
checking the Odia preview. Timing, speaker labels and processing controls are secondary.

On CUDA OOM the job stops, keeping saved outputs. The user can retry GPU, retry
transcription with shorter segments, or explicitly select slower CPU execution.
The transcription segment limit is 8–28 seconds, not a model batch-size setting.
Shorter segments reduce audio-dependent work but cannot make an oversized model fit.
The 4.5 GiB PyTorch budget is not a guarantee against driver/editor allocations or OOM.
Target acceptance remains Windows x64 CPU and Lenovo LOQ RTX 3050 6 GB; other hardware
must not be advertised as validated. No universal-PC or zero-OOM guarantee is made.

Development checks use `work/check-core-workflow.sbatch`: CPU-only Slurm allocation,
existing debug build cache, backend regressions and the podcast UI target. No full
workspace release rebuild is part of an ordinary workflow iteration. Windows setup,
real model inference and long-episode performance remain separate acceptance tests.

Validation on 2026-10-23: Slurm job 362408 on cn36-a40 passed 86 backend tests
and the podcast UI target (seven active interaction tests; the optional live-helper
case returned without running because its endpoint was not configured). The cached
Rust target prepared in 9.15 seconds and the UI tests completed in 1.17 seconds.
The preceding screenshot-enabled run compiled successfully but could not create a
graphics adapter. Visual screenshots, Windows installation and RTX 3050 inference
remain unverified; these source changes are not in the existing installer.

## Product direction

- Add a default Podcast workspace: master audio → diarization → speaker samples
  and camera assignment → sync review → camera edit → reaction review → export.
- Reuse Filmcraft's viewer and timeline throughout. Put workflow controls in a
  dockable panel; keep advanced tools available through other workspaces.
- Keep Python as an analysis worker for diarization, sync and face/gaze cues.
  Convert accepted results into undoable Filmcraft edits. Filmcraft's project is
  the authoritative edit; avoid maintaining a competing editable backend timeline.
- Use text branding initially. Replace upstream trademark artwork before shipping
  customized branding, while preserving source licenses and required attribution.
- Translation, TTS and lip-sync remain outside this phase.

Reaction review should offer short, synchronized host shots during guest speech,
grouped into configurable intervals. Show a few ranked looping previews with
observable cue labels and Keep/Skip controls. Accepted shots retain their original
time and the master audio. Respect manual edits, turn boundaries and minimum
spacing; optionally shuffle among qualified candidates. Track host speaking time,
added reaction time and total visibility separately. A visibility target must not
force low-quality filler when insufficient candidates exist.

Start evaluation with MediaPipe Face Landmarker for face visibility, smile/blink
blendshapes and head movement, then L2CS-Net for gaze direction if it improves
selection on real footage. User-selected reference frames calibrate preferred
directions. The optional worker adapters are implemented; model installation, real-footage quality and laptop performance remain unvalidated.
Sources: [MediaPipe](https://developers.google.com/edge/mediapipe/solutions/vision/face_landmarker),
[L2CS-Net](https://github.com/Ahmednull/L2CS-Net).
OpenFace 3.0 is not the default distribution choice: its
[license](https://raw.githubusercontent.com/CMU-MultiComp-Lab/OpenFace-3.0/main/LICENSE)
restricts use to noncommercial research and prohibits redistribution.

## Windows development from this Linux server

Develop and test portable logic here. Build and package the actual Filmcraft fork
on a Windows MSVC CI runner, then download its artifact for laptop testing.
The user download is one `YouCraft-Setup-x64.exe` installer. Model packs remain
separate because their weights/runtimes are much larger than the editor.
A Windows-only CI workflow is supplied at
`vendor/filmcraft/.github/workflows/ama-windows.yml`. GitHub Actions records the
Windows build status and installer artifact. Hosted Windows CPU tests do not establish
NVIDIA playback, encoder or CUDA compatibility.

Use allocated Linux GPU jobs for model experiments, measuring peak memory and
processing time. Final Windows/CUDA and playback checks require the target laptop
or another Windows GPU machine. The RTX 3050 6 GB remains an unvalidated target;
load analysis models sequentially and measure real jobs before promising fit.
See [GitHub runner documentation](https://docs.github.com/en/actions/reference/runners/github-hosted-runners).

Use `scripts/check_laptop_gpu.py` in the laptop's ML environment to report CUDA
availability; its optional `--request` mode measures a real model worker job.

The backend and audio reviewer live here. Earlier transcription experiments in
`src/`, `work/`, and `venv/` are preserved. This is a working development build;
Windows installer and hardware validation remain separate release work.


## V1 development handoff

- Source: `vendor/filmcraft` is the actual modified editor. Existing tools remain available.
- Backend: `backend/betun`, with one durable job queue and disposable model processes.
- Native commands: `podcast.assemble`, `podcast.correct`, `podcast.exportSrt`. These
  are accessible through the existing CLI/MCP bridge and ordinary undo history.
- Server export: set `BETUN_FILMCRAFT_CLI` to the matching CLI binary. The app submits
  a Filmcraft project snapshot; the server relinks registered media to originals and
  uses Filmcraft for rendering. Extra media must be registered before remote export.
- Browse server recordings: set `BETUN_MEDIA_ROOT` (for this workspace, `betun/raw`).
  The service binds to loopback; use an SSH tunnel from Windows. Originals stay here;
  local proxies provide playback. Server-file selection never uploads the originals.
- Build source bundle: `python scripts/stage_windows_source.py`. The ZIP contains
  the working fork, backend and Windows CI workflow, excluding recordings/weights.
  Put its contents at the root of your fork, then run **YouCraft Windows V1**.
- Model configuration: see `packaging/models-3050.example.json`. Replace all paths and
  placeholder revisions with installed, verified bundles. This is configuration,
  not an installer or proof of laptop compatibility. The default is Nemotron,
  with a pyannote adapter also available. Indic Core runs sequentially afterward.
- GPU jobs exit before the next task; cancellation terminates the job process tree.
  GPU out-of-memory errors stop processing and offer explicit retry choices. CPU
  processing is never an automatic fallback. The VRAM budget
  limits PyTorch allocations, not total GPU usage from the OS/editor/other programs.
- Odia correction: native editable text and an IndicXlit worker returning suggested
  Odia spelling for a Romanized word; explicit replacement leaves timing intact.
  IndicXlit needs its own compatible Python runtime and a preinstalled local bundle.
- Timing limitation: Indic Core supplies segment times. Native word positions are
  explicitly labelled **estimated**, not forced aligned. Listen before word-level
  cuts; overlap text needs manual review/combining, not invented transcription.
- Empty/malformed diarization intervals are removed with a count. Empty ASR text is
  not silence; review/remove prediction controls do not delete source recordings.
- Reaction analysis requires per-shard reference times, optional MediaPipe/L2CS
  model bundles and exactly one visible host face. Suggestions exclude host speech,
  overlap/combined shots and protected cuts. Preview with master audio; keep/reject.
- Windows GPU runtime, model adapters, full-episode rendering and original/proxy
  visual parity require acceptance testing before handing this to an editor.

V2 backlog: translation and reviewed dubbed tracks, TTS, optional lip-sync,
SoundCraft tools where they fill an actual gap, YouTube publishing/analytics,
PhotoCraft thumbnail handoff. Local MCP is already inherited from Filmcraft;
ChatGPT/Claude model inference is not automatically local merely because MCP is.

## Test the actual audio on a Windows laptop

Download `work/archives/betun-audio-review.zip`, extract the entire archive, and open
`Start review.cmd` or `index.html` in Edge or Chrome. The ZIP contains the actual
75-minute MP3, Nemotron speaker predictions, Indic-Transcribe Core text, waveform
and original model result. You need no Python or model installation to review it.

- Click a speaker segment in the waveform to replay and change its boundaries.
- Edit Odia transcript segments, speaker labels and timing; mark checked segments Reviewed.
- Flag and replay overlap. The initial run leaves mixed-voice overlap text blank.
- Use the original model text to compare with your correction.
- Export corrections before moving browsers/laptops; browser storage is local.
- Compare reviewed text for preliminary WER/CER. Word spacing and spelling affect Odia WER.

Return exported `betun-corrections.json` to this server, then apply it with:

```sh
.venv/bin/python scripts/apply_review.py /path/to/betun-corrections.json
```

Applying a review rejects stale revisions rather than overwriting newer changes.
Model output is a hypothesis for review; speaker labels are not verified identities.
The test recording has effectively identical left and right channels, not isolated mics.
See `review-data/channel-report.json` for the measured evidence.

## Run the backend and connected reviewer

Requires Python 3.11+ and FFmpeg/ffprobe on PATH or the BETUN_FFMPEG / BETUN_FFPROBE
variables. Previews and proxies require an FFmpeg build with libx264. Install the
project into a virtual environment:

```sh
python -m venv .venv
.venv/bin/python -m pip install -e '.[test]'
.venv/bin/python -m betun --data-dir review-data/server
```

Open `http://127.0.0.1:8765/`. The backend token is stored in the selected data
directory's `api-token` file. Enter it in the reviewer. For this Linux server,
forward port 8765 over SSH before opening the URL from your laptop.
The server binds to loopback and supports a single job worker.

## App and models

Keep the desktop app, ML runtimes, model packs and user projects separate on disk.
The UI presents them as one app. Model packs have pinned revisions and can be
updated without replacing the application. No model code or weights are compiled
into the app executable. Only a model being used needs to be loaded in memory.

Built-in model worker providers: nemotron-transformers, nemotron (NeMo), pyannote,
indic-core, and optional Aksharamukha transliteration. Translation and future
modules use a configured command protocol, not an assumed paid API.
See `docs/models.md`. Raw model output is cached with provider configuration and
source inputs; reviewed text and locked decisions survive regeneration.

## Existing backend prototype

The following describes the existing standalone backend, not the final Filmcraft
edit/export integration. Retain its analysis and review work while moving editing
ownership to Filmcraft.

SQLite stores project revisions, undo history and durable job metadata. FFmpeg
handles media probing, processing audio, previews and MP4 export. The editable
project keeps master/source seconds, overlapping activity and separate approved
cuts; `timeline` supplies the mapping to edited seconds. Rendering uses original
camera sources and master audio. Final export requires reviewed sync and a camera
edit. Transcription is optional; existing captions must be reviewed before export.
Jobs include import, waveform, sync, diarize, transcribe, shots, translate,
transliterate, preview and export. `/docs` describes the API. Projects can contain
only master audio during initial accuracy trials.

## Validation

### Engineering review — 23 October 2026

The current implementation is the native YouCraft/Filmcraft fork plus the local
Python helper. The guided workflow covers model setup, master import, speaker
review, camera attachment and sync review, transcript correction, reaction review,
and native editing/export. Earlier desktop and dubbing prototypes are not the V1
release path. Existing CPU and L40S measurements are recorded in
`vendor/filmcraft/docs/ama-validation.md`; they are short server trials, not Windows
laptop acceptance results.
The saved Windows CI log `work/windows-job-117970291200.log` records an installed
UI/project/audio-controls smoke check and MCP discovery on 19 October, followed
by a successful installer artifact upload. `dist/YouCraft-Setup-x64.exe` is that
earlier development artifact; the source fixes below are not included in it.

This review fixed these recovery problems:

- CPU/GPU selection remains available on the Audio page after import, allowing
  an existing episode to recover from an unavailable GPU without reimporting.
- A CUDA request without a usable CUDA runtime reports how to select CPU or
  install the NVIDIA runtime. Explicit GPU requests do not silently change device.
- The initial review tightened automatic CPU fallback; the subsequent core-flow
  revision removes it entirely, including for old configurations. GPU errors
  require an explicit retry/device choice. Incomplete results are discarded and
  cancellation prevents publishing worker results into the cache.
- Jobs whose project revision changed while waiting fail before expensive work.
  Retry explicitly captures the latest project; the existing save-time revision
  guard still protects edits made during inference. Queuing dependent tasks is
  not an automatic workflow chain: submit the next step after reviewing its input.
- SQLite connections close on successful and failed transactions, preventing
  connection/file-handle accumulation during long sessions and frequent polling.

Backend validation for this review: 80 tests passed, including ten new recovery
regressions and the external FFmpeg media integration tests. No real model/GPU
inference was run during this review.
The native `podcast_ui` suite also passed all four tests, with a rendered and
visually inspected CPU-switch screenshot in `work/review-ui-20261023/`.
Its optional live-helper case returns early without a configured endpoint; this
run verifies the synthetic UI/control paths, not live model inference.
Workspace formatting and release-profile Clippy checks with warnings treated as
errors passed. The review also corrected existing formatting/lint issues and
updated the older UI fixture to open an episode past the Projects chooser.
Dependency layering and asset metadata checks passed (71 assets); existing logo
permission records were preserved while correcting their metadata format.
The full release workspace test build was stopped before test execution on the
user's instruction to keep heavy CPU work off login nodes. Full CI is incomplete;
resume it only in a Slurm compute allocation. No new Windows installer was built.

The design should retain one authoritative native edit, one sequential analysis
queue, and disposable model processes. Remaining acceptance work is full-episode
CPU memory/throughput, Windows NVIDIA installation and recovery, synced playback,
long native exports, and human review of speaker/transcript/reaction quality.
The backend analysis document and native edit remain separate representations;
future workflow changes must explicitly define how revised analysis affects an
already assembled, manually edited sequence. Automatic reassembly must not be
treated as a safe merge of manual edits.

Run `.venv/bin/python -m pytest -q tests` from this directory. The audio trial uses
actual model inference in `scripts/audio_trial.sbatch`. Review diagnostics are
against your corrected reference, not a formal published diarization benchmark.
A Windows executable must be built and tested on Windows; this Linux server does
not produce a validated Windows EXE.

## Local Windows inference (development setup)

The separate `dist/betun-backend-source.zip` contains a Windows launcher and
model setup scripts. This path requires Python 3.11, Git and FFmpeg/ffprobe.
Run `packaging/install_models_windows.cmd` to create a separate CPU runtime, sign
in to Hugging Face, and download the pinned models. Then run
`packaging/Start local backend.cmd`, connect in the browser, and import the audio
using its full local path. Diarize first, reload, then transcribe.
This Windows inference path and PyInstaller build have not been executed on this
Linux server. The earlier offline reviewer is archived under `work/archives/`; the current product is the native Filmcraft fork.
