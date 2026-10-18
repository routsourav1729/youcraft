"""Small CPU reaction pack; pinned official model, resumable download and offline reuse."""
import argparse
import hashlib
import json
from pathlib import Path
import sys
import urllib.request
from download_models import Progress, atomic_json, read_json

URL = 'https://storage.googleapis.com/mediapipe-models/face_landmarker/face_landmarker/float16/1/face_landmarker.task'
SHA256 = '64184e229b263107bc2b804c6625db1341ff2bb731874b0bcc2fe6544e0bc9ff'
SIZE = 3758596


def checksum(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def install(root, status_file=None):
    directory = root / 'models' / 'face-landmarker-v1'
    directory.mkdir(parents=True, exist_ok=True)
    target = directory / 'face_landmarker.task'
    partial = target.with_suffix('.partial')
    with Progress(status_file, 'Reaction model') as progress:
        if not target.is_file() or checksum(target) != SHA256:
            offset = partial.stat().st_size if partial.exists() else 0
            if offset >= SIZE:
                if offset == SIZE and checksum(partial) == SHA256:
                    partial.replace(target)
                else:
                    partial.unlink()
                offset = 0
            if not target.is_file() or checksum(target) != SHA256:
                request = urllib.request.Request(URL, headers={'Range': f'bytes={offset}-'} if offset else {})
                progress.update(stage='Downloading reaction model', file=target.name, downloaded_bytes=offset, total_bytes=SIZE)
                with urllib.request.urlopen(request, timeout=30) as response:
                    if response.status != 206:
                        offset = 0
                    with partial.open('ab' if offset else 'wb') as out:
                        while data := response.read(65536):
                            out.write(data)
                            offset += len(data)
                            progress.update(downloaded_bytes=offset)
                if checksum(partial) != SHA256:
                    partial.unlink(missing_ok=True)
                    raise ValueError('Reaction model checksum mismatch; retry to download a clean copy')
                partial.replace(target)
        progress.update(stage='Checking reaction model')
        progress.publish()
        import mediapipe as mp
        options = mp.tasks.vision.FaceLandmarkerOptions(
            base_options=mp.tasks.BaseOptions(model_asset_path=str(target)), num_faces=2,
            output_face_blendshapes=True, output_facial_transformation_matrixes=True)
        with mp.tasks.vision.FaceLandmarker.create_from_options(options):
            pass
        config = read_json(root / 'models.json')
        config['reactions'] = dict(provider='mediapipe-reference', python=sys.executable,
            device='cpu', model_path=str(directory), face_model=str(target), revision=SHA256)
        atomic_json(root / 'models.json', config)
        progress.update(stage='Reaction model ready', state='succeeded', downloaded_bytes=SIZE, total_bytes=SIZE)
        progress.publish()


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--root', type=Path, required=True)
    parser.add_argument('--status-file', type=Path)
    args = parser.parse_args()
    install(args.root, args.status_file)
