"""All edit decisions live in master/source seconds; cuts supply a separate time map."""
from .schema import Shot


def kept_ranges(project):
    cursor = 0.0
    ranges = []
    for cut in sorted((c for c in project.cuts if c.approved), key=lambda c: c.start):
        if cut.start > cursor:
            ranges.append((cursor, cut.start))
        cursor = max(cursor, cut.end)
    if cursor < project.duration:
        ranges.append((cursor, project.duration))
    return ranges


def time_map(project):
    out, edited = [], 0.0
    for start, end in kept_ranges(project):
        out.append(dict(source_start=start, source_end=end, edited_start=edited, edited_end=edited + end - start))
        edited += end - start
    return out


def covering(project, camera, start, end):
    return next((c for c in sorted(project.clips, key=lambda c: (c.offset, c.id))
                 if c.camera == camera and c.offset <= start + 1e-5
                 and c.offset + c.duration * c.rate >= end - 1e-5), None)


def available_layout(project, desired, start, end):
    a, b = covering(project, "A", start, end), covering(project, "B", start, end)
    if desired == "black":
        return "black", []
    if desired == "A" and a or desired == "B" and b or desired == "both" and a and b:
        return desired, []
    return ("A" if a else "B" if b else "black"), ["missing_footage"]


def generate_shots(project):
    locked = [s for s in project.shots if s.locked]
    # Find continuous true overlap before padding. Padding individual turns would
    # invent overlap at every ordinary speaker change.
    events = {}
    for activity in project.activity:
        camera = project.settings.speaker_camera.get(activity.speaker)
        if camera is None:
            raise ValueError("Assign every detected speaker to a camera before generating shots")
        events.setdefault(activity.start, []).append((camera, 1))
        events.setdefault(activity.end, []).append((camera, -1))
    counts = {'A': 0, 'B': 0}
    raw_overlap = []
    boundaries = sorted(events)
    for index, start in enumerate(boundaries[:-1]):
        for camera, delta in events[start]:
            counts[camera] += delta
        end = boundaries[index + 1]
        if counts['A'] > 0 and counts['B'] > 0:
            if raw_overlap and raw_overlap[-1][1] == start:
                raw_overlap[-1] = (raw_overlap[-1][0], end)
            else:
                raw_overlap.append((start, end))
    padded = []
    for start, end in raw_overlap:
        if end - start < project.settings.meaningful_overlap:
            continue
        start = max(0., start - (project.settings.overlap_before if project.settings.overlap_before is not None else project.settings.overlap_padding))
        end = min(project.duration, end + (project.settings.overlap_after if project.settings.overlap_after is not None else project.settings.overlap_padding))
        if padded and padded[-1][1] >= start:
            padded[-1] = (padded[-1][0], max(padded[-1][1], end))
        else:
            padded.append((start, end))
    points = {0.0, project.duration}
    for start, end in padded:
        points.update((start, end))
    for s in [*project.activity, *locked]:
        points.update((s.start, s.end))
    for c in project.clips:
        if c.camera != "master":
            points.update((max(0., min(project.duration, c.offset)),
                           max(0., min(project.duration, c.offset + c.duration * c.rate))))
    points = sorted(points)
    result = []
    previous = "both"
    last_solo = "both"
    for start, end in zip(points, points[1:]):
        lock = next((s for s in locked if s.start <= start and s.end >= end), None)
        active = {a.speaker for a in project.activity if a.start < end and a.end > start}
        cams = {project.settings.speaker_camera[s] for s in active if s in project.settings.speaker_camera}
        if active - project.settings.speaker_camera.keys():
            raise ValueError("Assign every detected speaker to a camera before generating shots")
        if len(cams) == 1:
            last_solo = next(iter(cams))
        desired = "both" if len(cams) > 1 else next(iter(cams), last_solo)
        in_overlap = any(a <= start and b >= end for a, b in padded)
        if in_overlap:
            desired = "both"
        if lock:
            # Emit a locked span exactly once, with its original ID and boundaries.
            if not result or result[-1].id != lock.id:
                result.append(lock.model_copy(deep=True))
            previous = lock.layout
            continue
        duration = end - start
        if not in_overlap and desired == "both" and duration < project.settings.meaningful_overlap:
            desired = previous
        elif not in_overlap and len(cams) == 1 and previous not in ('both', 'black') and duration < project.settings.acknowledgement:
            desired = previous
        if not in_overlap and result and not result[-1].locked and result[-1].layout not in ('both', 'black', desired):
            # Clear speaker turns can override minimum shot; short uncertain turns cannot.
            if start - result[-1].start < project.settings.minimum_shot and duration < project.settings.minimum_shot:
                desired = previous
        layout, warnings = available_layout(project, desired, start, end)
        if result and not result[-1].locked and result[-1].layout == layout and result[-1].warnings == warnings:
            result[-1].end = end
        else:
            result.append(Shot(start=start, end=end, layout=layout, warnings=warnings))
        # Missing footage at a sub-frame camera offset must not make black the
        # preferred layout for the following silence or a short speaker turn.
        previous = layout if layout != 'black' else 'both'
    return result


def render_plan(project, start=0., end=None):
    end = project.duration if end is None else min(end, project.duration)
    plan = []
    edited = 0.
    for lo, hi in kept_ranges(project):
        lo, hi = max(lo, start), min(hi, end)
        if hi <= lo:
            continue
        points = {lo, hi}
        for shot in project.shots:
            points.update(x for x in (shot.start, shot.end) if lo < x < hi)
        for c in project.clips:
            if c.camera != "master":
                points.update(x for x in (c.offset, c.offset + c.duration * c.rate) if lo < x < hi)
        points = sorted(points)
        for a, b in zip(points, points[1:]):
            shot = next((s for s in project.shots if s.start <= a + 1e-5 and s.end >= b - 1e-5), None)
            desired = shot.layout if shot else "both"
            layout, warnings = available_layout(project, desired, a, b)
            warnings = list(dict.fromkeys((shot.warnings if shot else []) + warnings))
            sources = {}
            for camera in ([layout] if layout in ("A", "B") else ["A", "B"] if layout == "both" else []):
                clip = covering(project, camera, a, b)
                sources[camera] = dict(clip_id=clip.id, path=clip.path, start=max(0, (a - clip.offset) / clip.rate), rate=clip.rate)
            plan.append(dict(source_start=a, source_end=b, edited_start=edited,
                             edited_end=edited + b - a, layout=layout, sources=sources,
                             warnings=warnings, shot_id=shot.id if shot else None))
            edited += b - a
    return plan


def edited_captions(project):
    entries = []
    for cap in project.captions:
        for mapping in time_map(project):
            a, b = max(cap.start, mapping['source_start']), min(cap.end, mapping['source_end'])
            if b > a:
                entries.append(dict(id=cap.id, speaker=cap.speaker, text=cap.text,
                                    source_start=a, source_end=b,
                                    start=mapping['edited_start'] + a - mapping['source_start'],
                                    end=mapping['edited_start'] + b - mapping['source_start'],
                                    partial=a > cap.start or b < cap.end))
    return entries


def srt(project):
    entries = [c for c in edited_captions(project) if c['text'].strip()]
    boundaries = sorted({round(x, 3) for c in entries for x in (c['start'], c['end'])})
    cues = []
    for a, b in zip(boundaries, boundaries[1:]):
        active = [c for c in entries if c['start'] < b - .0001 and c['end'] > a + .0001]
        text = '\n'.join(f"{c['speaker']}: {' '.join(c['text'].split())}" for c in sorted(active, key=lambda x: (x['speaker'], x['id'])))
        if not text:
            continue
        if cues and cues[-1][2] == text and abs(cues[-1][1] - a) < .001:
            cues[-1] = (cues[-1][0], b, text)
        else:
            cues.append((a, b, text))

    def stamp(t):
        milliseconds = round(t * 1000)
        seconds, ms = divmod(milliseconds, 1000)
        minutes, sec = divmod(seconds, 60)
        hour, minute = divmod(minutes, 60)
        return f"{hour:02}:{minute:02}:{sec:02},{ms:03}"

    return '\n\n'.join(f"{i}\n{stamp(a)} --> {stamp(b)}\n{text}" for i, (a, b, text) in enumerate(cues, 1)) + ('\n' if cues else '')
