"""Podcast analysis decisions and Filmcraft handoff; no second editable timeline."""
import math
from .schema import Activity, Reaction

TICKS = 254_016_000_000


def clean_activity(rows, duration):
    """Drop malformed/empty predictions, not quiet speech; report every removal."""
    clean, removed = [], 0
    for row in rows:
        try:
            start, end = float(row['start']), float(row['end'])
            if not isinstance(row['speaker'], str):
                raise ValueError('invalid speaker')
            speaker = row['speaker'].strip()
            if not all(math.isfinite(v) for v in (start, end)) or not speaker:
                raise ValueError('invalid activity')
            start, end = max(0, start), min(duration, end)
            if end <= start:
                raise ValueError('empty activity')
            clean.append(Activity(start=start, end=end, speaker=speaker))
        except (KeyError, TypeError, ValueError):
            removed += 1
    return sorted(clean, key=lambda a: (a.start, a.end)), removed


def speaker_samples(project):
    """Longest uninterrupted solo span, capped at 12 seconds, one per speaker."""
    result = []
    for speaker in sorted({a.speaker for a in project.activity}):
        spans = []
        for a in project.activity:
            if a.speaker != speaker:
                continue
            boundaries = sorted({a.start, a.end} | {x for b in project.activity if b.speaker != speaker
                for x in (b.start, b.end) if a.start < x < a.end})
            for start, end in zip(boundaries, boundaries[1:]):
                if not any(b.speaker != speaker and b.start < end and b.end > start for b in project.activity):
                    spans.append((start, end))
        if spans:
            merged = []
            for start, end in sorted(spans):
                if merged and start <= merged[-1][1]:
                    merged[-1] = (merged[-1][0], max(end, merged[-1][1]))
                else:
                    merged.append((start,end))
            start, end = max(merged, key=lambda p: p[1] - p[0])
            result.append(dict(speaker=speaker, start=start, end=min(end, start + 12)))
    return result


def eligible(project, start, end, margin=0.5):
    """Only guest-only footage; protect host shots, overlap, manual locks and missing media."""
    host = project.settings.host_speaker
    host_camera = project.settings.speaker_camera.get(host)
    if not project.settings.roles_locked or not host_camera or end <= start:
        return False
    if any(a.speaker == host and a.start < end + margin and a.end > start - margin for a in project.activity):
        return False
    guest = [a for a in project.activity if a.speaker != host and a.start <= start and a.end >= end]
    if not guest:
        return False
    return not any(s.start < end and s.end > start and (s.locked or s.layout in ('both', host_camera)) for s in project.shots)


def suggestions(project, rows, interval=60, spacing=15, minimum=3, maximum=6):
    """Rank measured contiguous spans, at most one per interval; never synthesize gaze scores."""
    ranked = []
    for row in rows:
        if not any(cue in row.get('cues', []) for cue in ('smile', 'nod', 'manual')):
            continue
        clip = next((c for c in project.clips if c.id == row.get('clip_id')), None)
        if clip is None or clip.camera != project.settings.speaker_camera.get(project.settings.host_speaker):
            continue
        a, b = float(row['start']), float(row['end'])
        if not all(math.isfinite(x) for x in (a, b, float(row['score']))):
            continue
        b = min(b, a + maximum, project.duration, clip.offset + clip.duration * clip.rate)
        a = max(a, 0, clip.offset)
        if b - a < minimum or not eligible(project, a, b):
            continue
        ranked.append(Reaction(start=a, end=b, clip_id=clip.id,
            score=min(1, max(0, row['score'])), cues=row.get('cues', [])))
    chosen, counts = [], {}
    for r in sorted(ranked, key=lambda r: (-r.score, r.start)):
        bucket = int(r.start // interval)
        if counts.get(bucket, 0) >= 1 or any(r.start < x.end + spacing and r.end > x.start - spacing for x in chosen):
            continue
        chosen.append(r)
        counts[bucket] = counts.get(bucket, 0) + 1
    return sorted(chosen, key=lambda r: r.start)


def manifest(project):
    """Source-time plan; native editor becomes owner after this one-way import."""
    if not project.settings.roles_locked:
        raise ValueError('Choose and lock the host first')
    cameras = [c for c in project.clips if c.camera != 'master']
    if not cameras or any(not c.sync_reviewed for c in cameras):
        raise ValueError('Review synchronization for every camera shard')
    if not project.shots:
        raise ValueError('Generate and review camera shots first')
    if any(not c.reviewed for c in project.captions):
        raise ValueError('Review or remove every transcript segment before editing')
    if any(r.decision == 'pending' for r in project.reactions):
        raise ValueError('Keep or reject each reaction suggestion')
    master = next(c for c in project.clips if c.camera == 'master')
    keep = [r for r in project.reactions if r.decision == 'keep']
    for r in keep:
        if not eligible(project, r.start, r.end):
            raise ValueError('A kept reaction conflicts with changed speech/cuts; review it again')
    tolerance = .5 / (cameras[0].frame_rate or 25)
    boundaries = {0., project.duration}
    for s in [*project.shots, *keep]:
        boundaries.update((s.start, s.end))
    for c in cameras:
        boundaries.update((max(0, c.offset), min(project.duration, c.offset + c.duration * c.rate)))
    for cut in project.cuts:
        if cut.approved:
            boundaries.update((cut.start,cut.end))
    spans = sorted(t for t in boundaries if 0 <= t <= project.duration)
    clips = []
    cursor = 0
    for a, b in zip(spans, spans[1:]):
        mid = (a + b) / 2
        if any(c.approved and c.start <= mid < c.end for c in project.cuts):
            continue
        shot = next((s for s in project.shots if s.start <= mid < s.end), None)
        layout = shot.layout if shot else 'both'
        reaction = next((r for r in keep if r.start <= mid < r.end), None)
        if reaction:
            layout = project.settings.speaker_camera[project.settings.host_speaker]
        video = []
        for camera in ('A', 'B') if layout == 'both' else (layout,):
            source = next((c for c in cameras if c.camera == camera and c.offset <= a + tolerance
                           and c.offset + c.duration * c.rate >= b - tolerance), None)
            if source:
                video.append(dict(asset=source.id, source_in=round(max(0.,(a-source.offset)/source.rate)*TICKS),
                                  speed=1/source.rate, side=camera if layout == 'both' else 'full'))
        if not video:
            fallback = next((c for c in cameras if c.offset <= a+tolerance and c.offset+c.duration*c.rate >= b-tolerance),None)
            if fallback:
                video.append(dict(asset=fallback.id,source_in=round(max(0.,(a-fallback.offset)/fallback.rate)*TICKS),speed=1/fallback.rate,side='full'))
        if len(video) == 1:
            video[0]['side'] = 'full'
        if not video:
            raise ValueError(f'No camera covers {a:.2f}–{b:.2f}s; correct synchronization or coverage')
        length = round(b*TICKS)-round(a*TICKS)
        clips.append(dict(start=cursor, end=cursor+length, source_start=round(a*TICKS), video=video))
        cursor += length
    # Indic Core supplies segment times, not word alignment. Keep the approximation explicit.
    speakers = sorted({c.speaker for c in project.captions})
    captions = [c for c in sorted(project.captions, key=lambda c:c.start) if c.text.strip()]
    groups = []
    for caption in captions:
        if groups and caption.start < groups[-1]['end']:
            groups[-1]['end'] = max(groups[-1]['end'],caption.end)
            groups[-1]['captions'].append(caption)
        else:
            groups.append(dict(start=caption.start,end=caption.end,captions=[caption]))
    words = []
    # Native Filmcraft requires non-overlapping words. Preserve both speakers' text
    # inside their shared segment; original caption times remain in the project.
    for group in groups:
        tokens = [(word,c.speaker) for c in group['captions'] for word in c.text.split()]
        start,end = round(group['start']*TICKS),round(group['end']*TICKS)
        for i,(token,speaker) in enumerate(tokens):
            words.append(dict(text=token,start=start+(end-start)*i//len(tokens),
                end=start+(end-start)*(i+1)//len(tokens),speaker=speakers.index(speaker),confidence=0.0))
    return dict(version=1, source_duration=round(project.duration*TICKS), name=project.name, fps=cameras[0].frame_rate or 25, master=master.id, clips=clips,
                assets=[dict(id=c.id, path=c.path, fingerprint=c.fingerprint) for c in project.clips],
                transcript=dict(language='or', source='indic-core; estimated word timing from reviewed segments',
                    speakers=[dict(name=('Host' if s == project.settings.host_speaker else 'Guest') + ' · ' + s) for s in speakers], words=words))


def review_manifest(project, edit=False, clip_id=None, reaction_id=None):
    """Unsaved native review sequence: one clock, master audio, all camera shards.

    Reviewing never marks sync, captions or reactions as approved.
    """
    if reaction_id is not None and not any(r.id == reaction_id for r in project.reactions):
        raise ValueError('Reaction suggestion not found')
    master = next((c for c in project.clips if c.camera == 'master'), None)
    cameras = [c for c in project.clips if c.camera in ('A', 'B') and c.has_video]
    if clip_id is not None:
        cameras = [c for c in cameras if c.id == clip_id]
        if not cameras:
            raise ValueError('Camera shard not found')
        edit = False
    if master is None or not cameras or project.duration <= 0:
        raise ValueError('Add master audio and camera recordings first')
    tolerance = .5 / (cameras[0].frame_rate or 25)
    boundaries = {0., project.duration}
    for clip in cameras:
        boundaries.update((max(0., clip.offset), min(project.duration, clip.offset + clip.duration * clip.rate)))
    if edit:
        for item in [*project.shots, *project.reactions]:
            boundaries.update((item.start, item.end))
    spans = sorted(t for t in boundaries if 0 <= t <= project.duration)
    clips = []
    for start, end in zip(spans, spans[1:]):
        mid = (start + end) / 2
        sources = {camera: next((c for c in cameras if c.camera == camera and c.offset <= start + tolerance
                   and c.offset + c.duration * c.rate >= end - tolerance), None) for camera in ('A', 'B')}
        video = []
        def picture(camera, side):
            source = sources[camera]
            if source:
                video.append(dict(asset=source.id, source_in=round(max(0.,(start-source.offset)/source.rate)*TICKS),
                                  speed=1/source.rate, side=side))
        for camera in ('A', 'B'):
            picture(camera, 'full' if clip_id is not None else 'review-bottom-' + ('A' if camera == project.settings.speaker_camera.get(project.settings.host_speaker, 'A') else 'B') if edit else camera)
        if edit:
            shot = next((s for s in project.shots if s.start <= mid < s.end), None)
            layout = shot.layout if shot else 'both'
            if any((r.decision == 'keep' or r.id == reaction_id) and r.start <= mid < r.end for r in project.reactions):
                layout = project.settings.speaker_camera.get(project.settings.host_speaker, layout)
            if layout in sources and sources[layout] is None:
                layout = next((key for key,value in sources.items() if value is not None),'both')
            for camera in ('A', 'B') if layout == 'both' else (layout,):
                if camera in sources:
                    picture(camera, 'review-top-' + (camera if layout == 'both' else 'full'))
        clips.append(dict(start=round(start*TICKS), end=round(end*TICKS), video=video))
    return dict(version=1, review=True, review_edit=edit, name=project.name + (' · Edit review' if edit else ' · Camera review'),
                fps=cameras[0].frame_rate or 25, master=master.id, clips=clips,
                assets=[dict(id=c.id, path=c.path, fingerprint=c.fingerprint) for c in project.clips],
                transcript=dict(language='or', source='review', speakers=[], words=[]))
