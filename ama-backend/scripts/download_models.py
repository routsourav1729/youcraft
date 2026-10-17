"""Explicitly download pinned speech packs; never log credentials."""
import argparse
import json
from pathlib import Path
MODELS = {
    'indic-core': ('bodhan-ai/indic-transcribe-core', 'e083ab7dfeeddcb5c0b4a2d7df53f53e7b0f1537'),
    'nemotron': ('nvidia/Nemotron-3-Diarization', 'f667ed73aee57d40cc39428eb768b4fd87a0a29e'),
}

def main():
    from huggingface_hub import snapshot_download
    parser = argparse.ArgumentParser()
    parser.add_argument('--directory', type=Path, default=Path(__file__).resolve().parents[1] / 'models')
    args = parser.parse_args()
    args.directory.mkdir(parents=True, exist_ok=True)
    manifest = {}
    for name, (repo, revision) in MODELS.items():
        print('Downloading', name, flush=True)
        path = snapshot_download(repo, revision=revision, cache_dir=str(args.directory / 'hf-cache'),
                                 ignore_patterns=['nemo/*', '*.nemo'], max_workers=2)
        manifest[name] = dict(repo=repo, revision=revision, path=path)
    temporary = args.directory / 'manifest.tmp'
    temporary.write_text(json.dumps(manifest, indent=2), encoding='utf-8')
    temporary.replace(args.directory / 'manifest.json')
    print('Speech packs ready.', flush=True)

if __name__ == '__main__':
    main()
