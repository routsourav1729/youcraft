"""Write a validated, portable configuration without replacing it on setup failure."""
import argparse
import json
import sys
from pathlib import Path

def configuration(directory, device, python, partial=False):
    manifest = json.loads((directory / 'manifest.json').read_text(encoding='utf-8'))
    config = {}
    for task, provider, name in [('diarize', 'nemotron-transformers', 'nemotron'), ('transcribe', 'indic-core', 'indic-core')]:
        if partial and name not in manifest:
            continue
        entry = manifest[name]
        if not Path(entry['path']).is_dir():
            raise ValueError(f'Missing model directory: {name}')
        config[task] = dict(provider=provider, python=str(python), device=device,
            model_path=entry['path'], revision=entry['revision'], cpu_threads=4,
            max_vram_gb=4.5, allow_cpu_fallback=True)
    return config

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--directory', type=Path, default=Path(__file__).resolve().parents[1] / 'models')
    parser.add_argument('--output', type=Path)
    parser.add_argument('--device', choices=['cpu', 'auto', 'cuda'], default='cpu')
    parser.add_argument('--partial', action='store_true')
    args = parser.parse_args()
    output = args.output or args.directory / 'local-config.json'
    config = configuration(args.directory, args.device, sys.executable, args.partial)
    if output.exists():
        existing = json.loads(output.read_text(encoding='utf-8'))
        if not isinstance(existing, dict):
            raise ValueError('Existing model configuration must be an object')
        existing.update(config)
        config = existing
    output.parent.mkdir(parents=True, exist_ok=True)
    temporary = output.with_suffix('.tmp')
    temporary.write_text(json.dumps(config, indent=2), encoding='utf-8')
    temporary.replace(output)
    print('Model configuration saved:', output)

if __name__ == '__main__':
    main()
