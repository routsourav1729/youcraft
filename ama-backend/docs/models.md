# Windows quick setup

Open **Models** in YouCraft. On first launch the setup page opens automatically.
Choose **Install · CPU** for a CPU laptop or **Install · NVIDIA** for the RTX 3050.
Setup checks Python 3.11 x64, downloads the signed official installer if needed,
creates an isolated speech runtime, installs the pinned PyTorch and Transformers
build, and verifies imports and GPU availability. FFmpeg/ffprobe are installed
externally through Windows Package Manager when absent; they are not bundled.
Missing Windows App Installer is reported as an actionable setup error.

The pinned Nemotron and Indic Core packs download from Hugging Face into
`%LOCALAPPDATA%\YouCraft`. Several GB beyond the small installer are required.
Setup progress, cancellation, retry, and **Setup log** are available in the app.
The app reads the completed configuration without a restart. Inference is offline.

The Odia model may require accepting its Hugging Face access conditions. Open
**Odia model access**, grant access with your own account, and enter a read token
in the setup page if needed. The token is passed only to that setup process,
cleared from the UI, and excluded from workflow saves and MCP inspection.
If Odia download fails, the already installed speaker model remains configured.
CPU and NVIDIA runtimes use separate folders. Retries reuse cached downloads.

Start with `cpu-smoke-60s/master.wav`. MP3, WAV and camera audio from MP4/MOV are
converted internally to 16 kHz mono PCM WAV for speech models. Video containers
stay as video; there is no need to convert the whole video to a special AI format.
The full 20-minute set tests shard sync. 480p reduces transfer/decode cost, not
speech-model work. Workers exit between stages, releasing their model memory.

The installer does not yet install the optional gaze or Romanized spelling packs.
Their adapters exist but require separate runtime preparation and validation.
Direct Odia corrections and normal editing work independently of those packs.

# Model packs and workers

A lightweight backend does not import Torch. `BETUN_MODEL_CONFIG` points to a
JSON file whose task entries select a provider, Python executable, local model
path, device and revision. The server test configuration is generated at
`models/server-config.json`; its GPU device is intended for a Slurm allocation.
Do not start that CUDA worker on the login node.

Example local laptop configuration:

```json
{
  "diarize": {
    "provider": "nemotron-transformers",
    "python": "C:/Betun/model-runtime/Scripts/python.exe",
    "model_path": "C:/Betun/models/nemotron",
    "device": "cpu",
    "revision": "f667ed73aee57d40cc39428eb768b4fd87a0a29e"
  },
  "transcribe": {
    "provider": "indic-core",
    "python": "C:/Betun/model-runtime/Scripts/python.exe",
    "model_path": "C:/Betun/models/indic-core",
    "device": "cpu",
    "revision": "e083ab7dfeeddcb5c0b4a2d7df53f53e7b0f1537"
  }
}
```

The actual server trial uses Torch from the existing sravaani environment and an
isolated overlay `.model-venv`. Its Transformers build is pinned to Git commit
`536ecc007387a50e77603bb5d92100e9b07514cc`, because this new Nemotron architecture
requires current Transformers support. Nothing in the existing environment was
replaced. For a self-contained Windows runtime, create a fresh Python environment
with compatible Torch, soundfile, sentencepiece and the tested Transformers revision.
CPU execution is supported by adapters, but laptop throughput/memory need testing.

Model repository source and weights:
- https://huggingface.co/nvidia/Nemotron-3-Diarization
- https://huggingface.co/bodhan-ai/indic-transcribe-core
- https://huggingface.co/pyannote/speaker-diarization-community-1

Indic Core and Pyannote require accepted repository access conditions. We do not
accept those conditions on the user's behalf. Download pinned revisions with
`scripts/download_models.py` after authorization. Worker inference defaults to
offline mode and does not download replacement weights implicitly.

The Indic model bundle contains Python code, which is executed from the explicitly
configured model directory. Model packs must come from trusted, pinned sources.
Record attribution/license notices when redistributing packs; do not assume all
weights use the application's license. The portable audio reviewer includes
predictions and no model weights.

## External provider protocol

Instead of `python`, configure `command` as an argument array. It receives two
arguments: request JSON file and result JSON file. This enables another runtime
or a server adapter without changing the project format. Credentials belong in
the runtime's own configuration, not in project exports.

Request: `{"task": "diarize", "config": {...}, "payload": {"audio": "..."}}`.
Diarization result: `{"activity": [{"start": 1, "end": 2, "speaker": "0"}]}`.
Transcription requests contain chunks with stable IDs and local audio paths;
results must return every requested ID exactly once as `{"captions": [{"id": "...", "text": "..."}]}`.
Translation requests contain `entries` with IDs and corrected text; results return
matching `entries` with English text. Translation must not replace the original Odia.

## Optional V1 runtimes

The `mediapipe-l2cs` adapter needs MediaPipe, OpenCV, L2CS-Net and its upstream
RetinaFace dependency, plus the Face Landmarker and gaze weights. Prepare and
verify the complete detector cache before offline use. The example configuration
is not a model installer, and this runtime still needs real-footage acceptance testing.
Upstream dependency list: https://github.com/Ahmednull/L2CS-Net/blob/main/pyproject.toml

The `indicxlit` adapter uses a separate compatible runtime and a local bundle with
`transformer/indicxlit.pt`, `corpus-bin`, and the parent `lang_list.txt`. Fairseq
installation is a separate compatibility constraint on Windows. Odia text can
always be edited directly; Romanized spelling suggestions require this runtime.

The default laptop configuration now selects Nemotron and Indic Core sequentially,
with a 4.5 GiB allocation budget and optional fresh-process CPU retry. See the
fork's `docs/ama-validation.md` for measured server GPU results and their limits.

## Windows model setup (0.1.4)

The guided installer includes media tools, Nemotron speaker diarization and Indic
Transcribe Core ASR. It does not install the optional reaction or transliteration
runtimes above. Translation and TTS remain future packs.

Downloads report file, bytes, speed and waiting-for-data status. The access button
opens the model page in the default browser; setup logs open in Notepad. Storage
is derived from `%LOCALAPPDATA%/YouCraft`, never a developer's Windows username.

Completed packs have pinned revisions and a file inventory. Unchanged complete
packs are reused offline; incomplete packs reuse the Hugging Face cache and HTTP
Range resume. Partial files are deliberately retained for retry, not marked ready.
Downloaded weights are checked against upstream SHA-256 metadata. Damaged files
are replaced; completed weights are not erased on failure. Only a complete pack
updates its manifest. A validated runtime marker avoids reinstalling dependencies
on every retry. Changing CPU/NVIDIA mode uses the corresponding separate runtime
and shared model cache. Upgrading from an older installer performs one validation
pass to create these records. Runtime dependency changes must bump the marker.
