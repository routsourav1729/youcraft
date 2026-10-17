"""Download pinned model files without putting credentials into logs or bundles."""
import json
from pathlib import Path
from huggingface_hub import snapshot_download
ROOT = Path(__file__).resolve().parents[1]
MODELS = {
    'indic-core': ('bodhan-ai/indic-transcribe-core', 'e083ab7dfeeddcb5c0b4a2d7df53f53e7b0f1537'),
    'nemotron': ('nvidia/Nemotron-3-Diarization', 'f667ed73aee57d40cc39428eb768b4fd87a0a29e'),
}
manifest = {}
for name, (repo, revision) in MODELS.items():
    print('Downloading', name, revision, flush=True)
    path = snapshot_download(repo, revision=revision, cache_dir=str(ROOT / 'models' / 'hf-cache'),
                             ignore_patterns=['nemo/*', '*.nemo'], max_workers=2)
    manifest[name] = dict(repo=repo, revision=revision, path=path)
    (ROOT / 'models' / 'manifest.json').write_text(json.dumps(manifest, indent=2))
    print('Ready', name, flush=True)
