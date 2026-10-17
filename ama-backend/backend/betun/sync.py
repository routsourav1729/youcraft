"""Independent audio matching for every camera file, with affine clock-drift fit."""
import numpy as np


def match_window(master, window):
    from scipy.signal import correlate
    n = len(window)
    if n < 100 or n > len(master) or np.std(window) < 1e-5:
        raise ValueError('Insufficient usable audio for synchronization')
    window = window.astype(np.float64) - np.mean(window)
    reference = master.astype(np.float64)
    numerator = correlate(reference, window, mode='valid', method='fft')
    sums = np.concatenate(([0.], np.cumsum(reference)))
    squares = np.concatenate(([0.], np.cumsum(reference ** 2)))
    variance = np.maximum(squares[n:] - squares[:-n] - (sums[n:] - sums[:-n]) ** 2 / n, 1e-12)
    scores = np.abs(numerator) / np.sqrt(variance * np.sum(window ** 2))
    index = int(np.argmax(scores))
    # Competing peaks several seconds apart indicate repeating audio/ambiguous matches.
    excluded = scores.copy()
    excluded[max(0, index - 2000):index + 2001] = 0
    return index, float(scores[index]), float(scores[index] - np.max(excluded))


def synchronize(master, camera, sr=2000):
    # Inputs at 2 kHz keep full-episode correlation small; short windows tolerate drift.
    duration = len(camera) / sr
    length = min(8., duration / 3)
    if length < 1.:
        raise ValueError('Camera audio too short to synchronize')
    positions = np.linspace(0, max(0, duration - length), 5)
    anchors = []
    for position in positions:
        window = camera[int(position * sr):int((position + length) * sr)]
        try:
            index, score, margin = match_window(master, window)
            if score >= .12 and margin >= .025:
                anchors.append((position + length / 2, index / sr + length / 2, score))
        except ValueError:
            continue
    if not anchors:
        raise ValueError('No reliable match; set manual offset and review sync')
    # Consensus affine fit rejects matches to repeated music and unrelated audio.
    best = []
    for x1, y1, _ in anchors:
        for x2, y2, _ in anchors:
            if x2 <= x1:
                continue
            rate = (y2 - y1) / (x2 - x1)
            if not .98 <= rate <= 1.02:
                continue
            offset = y1 - rate * x1
            inliers = [a for a in anchors if abs(a[1] - (offset + rate * a[0])) < .12]
            if len(inliers) > len(best):
                best = inliers
    warnings = []
    if len(best) >= 3:
        rate, offset = np.polyfit([a[0] for a in best], [a[1] for a in best], 1)
        if not .98 <= rate <= 1.02:
            raise ValueError('Implausible clock drift; manual sync required')
        residual = float(max(abs(y - (offset + rate * x)) for x, y, _ in best))
        if abs(rate - 1) > .0001:
            warnings.append('clock_drift_detected')
    else:
        chosen = max(anchors, key=lambda a: a[2])
        offset, rate, residual = chosen[1] - chosen[0], 1., None
        warnings.append('drift_not_established')
        best = [chosen]
    return dict(offset=float(offset), rate=float(rate), score=float(np.mean([a[2] for a in best])),
                residual_seconds=residual, anchors=anchors, warnings=warnings + ['sync_review_required'])
