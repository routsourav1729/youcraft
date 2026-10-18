"""Ordered shard matching using Filmcraft's native GCC-PHAT implementation."""
import json
import os
from pathlib import Path
import shutil
import sys
import numpy as np
from . import media


def executable():
    candidates = [os.environ.get('YOUCRAFT_CLI'), os.environ.get('BETUN_FILMCRAFT_CLI')]
    if getattr(sys, 'frozen', False):
        candidates.append(str(Path(sys.executable).resolve().parent.parent / 'filmcraft-cli.exe'))
    local = os.environ.get('LOCALAPPDATA')
    if local:
        candidates.append(str(Path(local) / 'Programs/YouCraft/filmcraft-cli.exe'))
    candidates.append(shutil.which('filmcraft-cli'))
    for path in candidates:
        if path and Path(path).is_file():
            return path
    raise RuntimeError('Filmcraft sync engine unavailable. Install the matching YouCraft build or set YOUCRAFT_CLI for development.')


def synchronize(master, camera, sr=8000, *, directory, minimum_offset=None, cancel=lambda: False, on_progress=lambda _: None):
    """Search the complete remaining master in bounded windows, then fit clock drift.

    Ordering constrains each camera separately. Matching is exclusively native;
    Python only orchestrates searches and checks consistency across five anchors.
    """
    engine = executable()
    directory = Path(directory)
    reference = directory / 'sync-master.f32'
    source = directory / 'sync-camera.f32'
    request = directory / 'sync-request.json'
    duration = len(camera) / sr
    length = min(12., duration / 3)
    if length < 1:
        raise ValueError('Camera recording is too short for automatic synchronization')
    anchors = []
    try:
        np.asarray(master, dtype='<f4').tofile(reference)
        np.asarray(camera, dtype='<f4').tofile(source)
        for anchor_index, position in enumerate(np.linspace(0, max(0, duration-length), 5)):
            on_progress(anchor_index/5)
            matches = []
            lower = max(0., (minimum_offset or 0.) + .98*position)
            # Overlap blocks by the sampled window to avoid missing boundary matches.
            for start in np.arange(lower, len(master)/sr, 600.):
                count = min(len(master)-round(start*sr), round((600+length)*sr))
                if count < length*sr:
                    continue
                payload = dict(sample_rate=sr,
                    master=dict(path=str(reference), start=round(start*sr), count=count),
                    camera=dict(path=str(source), start=round(position*sr), count=round(length*sr)))
                request.write_text(json.dumps(payload), encoding='utf-8')
                result = json.loads(media.run([engine, 'sync-audio', str(request)], cancel, timeout=120))
                offset = start + result['lag_seconds'] - position
                # A partial overlap at a search-block edge is not a complete anchor.
                at = start + result['lag_seconds']
                if (result['reliable'] and at >= start-.001 and at+length <= start+count/sr+.001
                        and (minimum_offset is None or at >= minimum_offset+.98*position-.001)):
                    matches.append((position+length/2, at+length/2, result['confidence'], result['distinct']))
            if matches:
                matches.sort(key=lambda row: row[2], reverse=True)
                # The same match may appear in two overlapping search blocks.
                matches = [matches[0]] + [m for m in matches[1:] if abs(m[1]-matches[0][1]) > .08]
                if len(matches) == 1 or matches[0][2] >= matches[1][2]*1.2:
                    anchors.append(matches[0])
        on_progress(1.)
        if len(anchors) < 3:
            raise ValueError('Fewer than three consistent audio anchors; review this shard manually')
        best = []
        for x1,y1,*_ in anchors:
            for x2,y2,*_ in anchors:
                if x2 <= x1:
                    continue
                rate = (y2-y1)/(x2-x1)
                if not .98 <= rate <= 1.02:
                    continue
                offset = y1-rate*x1
                inliers = [a for a in anchors if abs(a[1]-(offset+rate*a[0])) < .08]
                if len(inliers) > len(best):
                    best = inliers
        if len(best) < 3:
            raise ValueError('Audio anchors disagree; review sync instead of guessing an offset')
        rate, offset = np.polyfit([a[0] for a in best], [a[1] for a in best], 1)
        if not .98 <= rate <= 1.02 or (minimum_offset is not None and offset < minimum_offset-.05):
            raise ValueError('Match conflicts with shard order or has implausible drift')
        residual = max(abs(y-(offset+rate*x)) for x,y,*_ in best)
        return dict(offset=float(offset), rate=float(rate), score=float(np.mean([a[2] for a in best])),
                    residual_seconds=float(residual), anchors=anchors, engine='filmcraft-gcc-phat',
                    warnings=['sync_review_required'] + (['clock_drift_detected'] if abs(rate-1)>.0001 else []))
    finally:
        for path in (reference,source,request):
            path.unlink(missing_ok=True)
