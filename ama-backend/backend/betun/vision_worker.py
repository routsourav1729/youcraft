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
    rows, references = [], {}
    with mp.tasks.vision.FaceLandmarker.create_from_options(opts) as face:
        def measure(frame):
            if frame.shape[1] > 960:
                frame = cv2.resize(frame, (960, max(1, round(frame.shape[0] * 960 / frame.shape[1]))))
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
                blink=max(scores.get('eyeBlinkLeft',0), scores.get('eyeBlinkRight',0)), pitch=head_pitch)
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
                    for second in np.linspace(0, max(0., clip['duration'] - 1), 20):
                        cap.set(cv2.CAP_PROP_POS_MSEC, second * 1000)
                        ok, image = cap.read()
                        measured = measure(image) if ok else None
                        if measured and measured['blink'] < .4:
                            samples.append((float(second), measured))
                    if not samples:
                        raise ValueError('No clear single-face reference found; select another recording')
                    center = np.median([m['gaze'] for _, m in samples], axis=0)
                    references[clip['id']] = min(samples, key=lambda x: float(np.linalg.norm(np.asarray(x[1]['gaze']) - center)))[0]
                    continue
                if reference is None:
                    raise ValueError('Choose or suggest a reference frame for every host recording')
                cap.set(cv2.CAP_PROP_POS_MSEC, reference*1000)
                ok, image = cap.read()
                ref = measure(image) if ok else None
                if ref is None:
                    raise ValueError('Reference must show exactly one visible face with usable eyes')
                fps = float(cap.get(cv2.CAP_PROP_FPS))
                if not math.isfinite(fps) or fps <= 0:
                    raise ValueError('Invalid camera frame rate')
                cap.set(cv2.CAP_PROP_POS_FRAMES, 0)
                stride = max(1, round(fps / 8))  # Enough temporal samples to distinguish a nod.
                streak, missed, index = [], 0, 0
                def finish():
                    if len(streak) < 16:
                        return
                    pitches = [s[1]['pitch'] for s in streak]
                    # A nod goes away and returns; a static down-looking pose is not a nod.
                    nod = max(pitches)-min(pitches) >= 7 and abs(pitches[-1]-pitches[0]) < 5
                    smile = max(s[1]['smile'] for s in streak) > .35
                    cues = ['reference look' if lightweight else 'reference gaze'] + (['nod'] if nod else []) + (['smile'] if smile else [])
                    rows.append(dict(clip_id=clip['id'], start=clip['offset']+streak[0][0]*clip['rate'],
                        end=clip['offset']+(streak[-1][0]+stride/fps)*clip['rate'],
                        score=.5 + .25*nod + .25*smile, cues=cues))
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
                    if lightweight:
                        match = m and np.linalg.norm(np.asarray(m['gaze']) - ref['gaze']) * 100 <= payload['tolerance'] and abs(m['yaw'] - ref['yaw']) <= 20
                    else:
                        match = m and angle(m['gaze'], ref['gaze']) <= payload['tolerance']
                    good = match and m['blink'] < .6
                    if good:
                        streak.append((seconds,m)); missed=0
                    else:
                        missed += 1
                    if missed > 2 or len(streak) >= 48:
                        finish(); streak=[]; missed=0
                finish()
            finally:
                cap.release()
                finished_seconds += clip['duration']
    return dict(candidates=rows, references=references)
