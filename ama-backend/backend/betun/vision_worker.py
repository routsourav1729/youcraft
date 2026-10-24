"""Optional bounded-memory MediaPipe + L2CS analysis, loaded only in a disposable worker."""
import math
import numpy as np


def estimate_gaze(gaze, frame):
    import torch
    # Upstream L2CS stacks an empty detection list when no face is found.
    # Treat that specific case as an unusable frame, not a failed episode.
    try:
        with torch.inference_mode():
            return gaze.step(frame)
    except ValueError as exc:
        if 'need at least one array to stack' in str(exc):
            return None
        raise


def angle(a, b):
    # L2CS returns pitch/yaw in radians. Compare directions on the unit sphere.
    def vector(p):
        pitch, yaw = p
        return np.array([math.cos(pitch)*math.sin(yaw), math.sin(pitch), math.cos(pitch)*math.cos(yaw)])
    return math.degrees(math.acos(float(np.clip(np.dot(vector(a), vector(b)), -1, 1))))


def analyze(payload, config, progress=lambda *args, **kwargs: None):
    import cv2
    import mediapipe as mp
    lightweight = config['provider'] == 'mediapipe-reference'
    gaze = None
    if not lightweight:
        import torch
        from l2cs import Pipeline
        from pathlib import Path
        gaze = Pipeline(weights=Path(config['gaze_weights']), arch='ResNet50', device=torch.device(config.get('device', 'cpu')))
    opts = mp.tasks.vision.FaceLandmarkerOptions(
        base_options=mp.tasks.BaseOptions(model_asset_path=config['face_model']),
        running_mode=mp.tasks.vision.RunningMode.IMAGE, num_faces=2,
        output_face_blendshapes=True, output_facial_transformation_matrixes=True)
    rows, references, issues, measurements = [], {}, {}, []
    with mp.tasks.vision.FaceLandmarker.create_from_options(opts) as face:
        def measure(frame):
            if frame.shape[1] > 1280:
                frame = cv2.resize(frame, (1280, max(1, round(frame.shape[0] * 1280 / frame.shape[1]))))
            f = face.detect(mp.Image(image_format=mp.ImageFormat.SRGB, data=cv2.cvtColor(frame, cv2.COLOR_BGR2RGB)))
            if len(f.face_landmarks) != 1 or not f.face_blendshapes or not f.facial_transformation_matrixes:
                return None
            if lightweight:
                points = f.face_landmarks[0]
                descriptors = []
                for center, left, right in [(468, 33, 133), (473, 362, 263)]:
                    def point(i):
                        return np.array([points[i].x * frame.shape[1], points[i].y * frame.shape[0]])
                    a, b, iris = point(left), point(right), point(center)
                    axis = b - a
                    width = np.linalg.norm(axis)
                    if width < 2:
                        return None
                    axis = axis / width
                    relative = iris - (a + b) / 2
                    descriptors.append([np.dot(relative, axis) / width, np.dot(relative, [-axis[1], axis[0]]) / width])
                direction = np.mean(descriptors, axis=0)
            else:
                result = estimate_gaze(gaze, frame)
                if result is None or len(result.pitch) != 1:
                    return None
                direction = (float(result.pitch[0]), float(result.yaw[0]))
            scores = {c.category_name:c.score for c in f.face_blendshapes[0]}
            mat = np.asarray(f.facial_transformation_matrixes[0])
            head_pitch = math.degrees(math.atan2(-mat[2,1], mat[2,2]))
            return dict(gaze=direction, yaw=math.degrees(math.atan2(mat[2,0], mat[2,2])),
                smile=(scores.get('mouthSmileLeft',0)+scores.get('mouthSmileRight',0))/2,
                blink=max(scores.get('eyeBlinkLeft',0), scores.get('eyeBlinkRight',0)), pitch=head_pitch, aspect=frame.shape[1]/frame.shape[0], eyes=[[float(f.face_landmarks[0][i].x), float(f.face_landmarks[0][i].y)] for i in (468, 473)],
                nose=[float(f.face_landmarks[0][1].x), float(f.face_landmarks[0][1].y)])
        image_ref = None
        if payload.get('reference_image'):
            image = cv2.imread(payload['reference_image'])
            image_ref = measure(image) if image is not None else None
            if image_ref is None:
                raise ValueError('Screenshot has no usable single face. Choose a clear host screenshot with both eyes visible.')
        total_seconds = sum(c['duration'] for c in payload['clips'])
        finished_seconds = 0.
        for clip in payload['clips']:
            reference = payload.get('references', {}).get(clip['id'])
            cap = cv2.VideoCapture(clip['path'])
            try:
                if not cap.isOpened():
                    raise ValueError('Cannot decode host camera ' + clip['path'])
                if payload.get('reference_only'):
                    samples = []
                    times = reference_times(clip['duration'], payload.get('reference_windows', {}).get(clip['id'], []))
                    for sample_index, second in enumerate(times):
                        progress('Choosing host reference', completed=sample_index, total=len(times), units='frames')
                        cap.set(cv2.CAP_PROP_POS_MSEC, second * 1000)
                        ok, image = cap.read()
                        measured = measure(image) if ok else None
                        if measured and measured['blink'] < .4:
                            samples.append((float(second), measured))
                            if len(samples) >= 16:
                                break
                    if not samples:
                        issues[clip['id']] = 'No clear host face found. Use a visible host frame or upload a reference screenshot; other references were kept.'
                        continue
                    center = np.median([m['gaze'] for _, m in samples], axis=0)
                    references[clip['id']] = min(samples, key=lambda x: float(np.linalg.norm(np.asarray(x[1]['gaze']) - center)))[0]
                    continue
                sample_times = payload.get('reference_samples', {}).get(clip['id'], [])
                if len(sample_times) > 8 or any(not math.isfinite(t) or not 0 <= t < clip['duration'] for t in sample_times):
                    raise ValueError('Reference samples must be within the host recording (maximum eight)')
                if sample_times:
                    reference = sample_times[0]
                if reference is None and image_ref is None:
                    raise ValueError('Choose a host reference frame or upload a reference screenshot')
                ref = image_ref
                if ref is None:
                    cap.set(cv2.CAP_PROP_POS_MSEC, reference*1000)
                    ok, image = cap.read()
                    ref = measure(image) if ok else None
                if ref is None:
                    raise ValueError('Host reference has no usable single face. Suggest another reference or upload a clear host screenshot with both eyes visible.')
                reference_faces = [ref]
                if image_ref is None:
                    for second in sample_times[1:]:
                        cap.set(cv2.CAP_PROP_POS_MSEC, second * 1000)
                        ok, image = cap.read()
                        extra = measure(image) if ok else None
                        if extra is None or extra['blink'] >= .6:
                            raise ValueError('A confirmed look example is unclear or blinking. Remove it and choose a clear frame.')
                        reference_faces.append(extra)
                fps = float(cap.get(cv2.CAP_PROP_FPS))
                if not math.isfinite(fps) or fps <= 0:
                    raise ValueError('Invalid camera frame rate')
                cap.set(cv2.CAP_PROP_POS_FRAMES, 0)
                stride = max(1, round(fps / 8))  # Enough temporal samples to distinguish a nod.
                streak, missed, index, last_measurement = [], 0, 0, -1
                def finish():
                    if len(streak) < 16:
                        return
                    while streak and not streak[-1][1].get('matching', False):
                        streak.pop()
                    if len(streak) < 16 or sum(bool(m.get('matching')) for _, m in streak) / len(streak) < .75:
                        return
                    pitches = [s[1]['pitch'] for s in streak]
                    # A nod goes away and returns; a static down-looking pose is not a nod.
                    nod = is_nod(streak)
                    smile_peak = max(s[1]['smile'] for s in streak)
                    smile = smile_peak >= (payload.get('smile_threshold') if payload.get('smile_threshold') is not None else .35)
                    cues = ['reference look' if lightweight else 'reference gaze'] + (['nod'] if nod else []) + (['smile'] if smile else [])
                    rows.append(dict(clip_id=clip['id'], start=clip['offset']+streak[0][0]*clip['rate'],
                        end=clip['offset']+(streak[-1][0]+stride/fps)*clip['rate'],
                        score=.5 + .25*nod + .25*smile, cues=cues, smile_peak=smile_peak))
                while True:
                    ok = cap.grab()
                    if not ok:
                        break
                    if index % stride:
                        index += 1
                        continue
                    ok, image = cap.retrieve()
                    seconds = index/fps
                    index += 1
                    m = measure(image) if ok else None
                    if index % max(1, round(fps * 2)) < stride:
                        progress('Finding reactions', completed=int(finished_seconds + seconds), total=int(total_seconds), units='seconds', fraction=(finished_seconds + seconds) / max(1, total_seconds))
                    closest = min(reference_faces, key=lambda r: float(np.linalg.norm(np.asarray(m['gaze']) - r['gaze']))) if m else ref
                    look_distance = None if not m else (float(np.linalg.norm(np.asarray(m['gaze']) - closest['gaze']) * 100) if lightweight else angle(m['gaze'], closest['gaze']))
                    match = m and look_distance <= payload['tolerance'] and (not lightweight or abs(m['yaw'] - closest['yaw']) <= 20)
                    if m:
                        m['matching'] = bool(match and m['blink'] < .6)
                    good = match and m['blink'] < .6
                    if int(seconds) != last_measurement and len(measurements) < 86400:
                        last_measurement = int(seconds)
                        row = dict(time=clip['offset']+seconds*clip['rate'], clip_id=clip['id'], detected=m is not None, matching=bool(good),
                                   look_distance=look_distance, look_unit='eye_width_percent' if lightweight else 'degrees',
                                   head_difference=None if not m else abs(m['yaw'] - closest['yaw']))
                        if m:
                            row.update({key:m[key] for key in ('eyes', 'nose', 'pitch', 'yaw', 'smile', 'blink', 'aspect')})
                        measurements.append(row)
                    if good:
                        streak.append((seconds,m)); missed=0
                    else:
                        missed += 1
                        # Allow a short look excursion during a nod, but require
                        # confirmed listening before/after and stable head yaw.
                        if streak and m and missed <= 6 and abs(m['yaw'] - streak[0][1]['yaw']) <= 10:
                            streak.append((seconds, m))
                    if missed > 6 or len(streak) >= 48:
                        finish(); streak=[]; missed=0
                finish()
            finally:
                cap.release()
                finished_seconds += clip['duration']
    return dict(candidates=rows, references=references, issues=issues, measurements=measurements)


def reference_times(duration, speaking_windows):
    """Bounded sampling, prioritized by host speech and distance from recording edges."""
    end = max(0., duration - .1)
    windows = sorted(speaking_windows, key=lambda w: abs((w[0]+w[1])/2-duration/2))
    preferred = [float(t) for a,b in windows[:16] if b>a for t in np.linspace(max(0.,a), min(end,b), 3)]
    broad = [float(t) for t in np.linspace(duration*.1, end*.9, 32)]
    seen, result = set(), []
    for t in preferred + broad:
        t = min(end, max(0.,t))
        key = round(t, 2)
        if key not in seen:
            seen.add(key); result.append(t)
    return result


def is_nod(samples):
    """A brief pitch excursion and return with stable yaw, bracketed by listening."""
    for begin in range(len(samples)):
        start, baseline = samples[begin]
        if not baseline.get('matching'):
            continue
        window = [(t, m) for t, m in samples[begin:begin+21] if t-start <= 2.5]
        if len(window) < 4:
            continue
        if max(m['yaw'] for _, m in window) - min(m['yaw'] for _, m in window) > 10:
            continue
        excursion = False
        for t, measured in window[1:]:
            delta = abs(measured['pitch'] - baseline['pitch'])
            excursion = excursion or delta >= 7
            if excursion and .3 <= t-start <= 2.5 and delta <= 3 and measured.get('matching'):
                return True
    return False
