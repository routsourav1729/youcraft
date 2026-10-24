"""Re-score cached face signals without interpreting blendshape intensity as probability."""
import math


def rescore_candidates(analysis, threshold):
    if not math.isfinite(threshold) or not 0 <= threshold <= 1:
        raise ValueError('Smile threshold must be between 0 and 1')
    measured = {}
    for sample in analysis.get('measurements', []):
        value = sample.get('smile')
        if sample.get('detected') and isinstance(value, (int, float)) and math.isfinite(value):
            measured.setdefault(sample.get('clip_id'), []).append(sample)
    result = []
    for original in analysis.get('candidates', []):
        row = dict(original)
        peak = row.get('smile_peak')
        if peak is None:
            values = [s['smile'] for s in measured.get(row.get('clip_id'), [])
                      if row['start'] <= s['time'] < row['end']]
            peak = max(values) if values else None
        if peak is not None:
            cues = [c for c in row.get('cues', []) if c != 'smile']
            if peak >= threshold:
                cues.append('smile')
            row.update(cues=cues, score=.5 + .25 * ('nod' in cues) + .25 * ('smile' in cues))
        result.append(row)
    return result


def preserve_choices(project, selected, analysis):
    """Kept/manual edits survive recalculation; rejected spans cannot silently return."""
    if not isinstance(analysis.get('decisions'), dict):
        analysis['decisions'] = {}
    history = analysis['decisions']
    history.update({f'{r.clip_id}:{r.start}:{r.end}': r.decision for r in project.reactions})
    preserved = [r for r in project.reactions if r.decision == 'keep' or 'manual' in r.cues]
    rejected = []
    for key, decision in history.items():
        if decision != 'reject':
            continue
        try:
            clip, start, end = key.rsplit(':', 2)
            start, end = float(start), float(end)
            if math.isfinite(start) and math.isfinite(end) and end > start:
                rejected.append((clip, start, end))
        except (ValueError, TypeError):
            continue
    result = list(preserved)
    for reaction in selected:
        if any(reaction.start < r.end and reaction.end > r.start for r in preserved):
            continue
        reaction.decision = history.get(f'{reaction.clip_id}:{reaction.start}:{reaction.end}', 'pending')
        if any(reaction.clip_id == clip and reaction.start < end and reaction.end > start for clip, start, end in rejected):
            reaction.decision = 'reject'
        result.append(reaction)
    return sorted(result, key=lambda r: r.start)


def fit_reactions_to_shots(project):
    """Keep decisions, trimming inserts where newly padded speech must take priority."""
    host_camera = project.settings.speaker_camera.get(project.settings.host_speaker)
    result = []
    for reaction in project.reactions:
        pieces = []
        for shot in project.shots:
            if shot.locked or shot.layout in (host_camera, 'both', 'black'):
                continue
            start, end = max(reaction.start, shot.start), min(reaction.end, shot.end)
            if end > start:
                pieces.append((start, end))
        for index, (start, end) in enumerate(pieces):
            result.append(reaction.model_copy(update=dict(start=start, end=end,
                id=reaction.id if index == 0 else reaction.id + '-part-' + str(index))))
    return result
