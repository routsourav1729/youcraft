"""Stable camera framing from sparse face samples; no per-frame tracking jitter."""
import math
from statistics import median


def stable_center(samples):
    valid = [p for p in samples[:24] if isinstance(p, list) and len(p) == 2
             and all(isinstance(v, (int, float)) and math.isfinite(v) and 0 <= v <= 1 for v in p)]
    if len(valid) < 3:
        return None
    x, y = median(p[0] for p in valid), median(p[1] for p in valid)
    # Multiple positions / moving camera need a human choice, never a sweeping crop.
    near = [p for p in valid if abs(p[0]-x) <= .12 and abs(p[1]-y) <= .12]
    if len(near) < max(3, math.ceil(len(valid)*.7)):
        return None
    return median(p[0] for p in near), median(p[1] for p in near)
