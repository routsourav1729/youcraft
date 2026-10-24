import hashlib
import json
import re
import wave
from pathlib import Path
import numpy as np
from . import media, providers
from .schema import Activity, Caption
from .timeline import generate_shots, time_map, render_plan
from .render import render


def natural_key(path):
    return [int(x) if x.isdigit() else x.lower() for x in re.split(r'(\d+)', path)]


def chunk_turns(project, audio_paths, directory):
    """Diarization supplies speech detection; split long turns near low-energy pauses."""
    chunks, captions = [], []
    segment_seconds = project.settings.transcription_segment_seconds
    arrays = {}
    for turn in sorted(project.activity, key=lambda a: (a.start, a.speaker)):
        boundaries = {turn.start, turn.end}
        for other in project.activity:
            if other.speaker != turn.speaker:
                boundaries.update(x for x in (other.start, other.end) if turn.start < x < turn.end)
        boundaries = sorted(boundaries)
        channel = project.settings.speaker_channel.get(turn.speaker)
        audio = audio_paths.get(channel, audio_paths[None])
        for start, end in zip(boundaries, boundaries[1:]):
            overlap = any(t.speaker != turn.speaker and t.start < end and t.end > start for t in project.activity)
            cursor = start
            while cursor < end - .001:
                stop = min(cursor + segment_seconds, end)
                if stop < end:
                    if str(audio) not in arrays:
                        arrays[str(audio)] = media.read_wav(audio)[0]
                    samples = arrays[str(audio)]
                    # Find a low-energy split within the chosen segment limit; no speech is removed.
                    candidates = np.arange(cursor + min(20, segment_seconds * .75), stop - .1, .1)
                    energies = [float(np.mean(samples[int((x - .1) * 16000):int((x + .1) * 16000)] ** 2)) for x in candidates]
                    if energies:
                        stop = float(candidates[int(np.argmin(energies))])
                cid = hashlib.sha256(f'{turn.speaker}:{cursor:.6f}:{stop:.6f}'.encode()).hexdigest()[:32]
                caption = Caption(id=cid, start=cursor, end=stop, speaker=turn.speaker, overlap=overlap)
                # Preserve corrected/locked text even after diarization boundaries move.
                protected = any(c.speaker == turn.speaker and (c.locked or c.reviewed) and c.start < stop and c.end > cursor for c in project.captions)
                if not protected:
                    captions.append(caption)
                    if not overlap or channel is not None:
                        path = directory / f'chunk-{cid}-{channel}.wav'
                        # Pad within this speaker's turn; keep reported source timestamps unpadded.
                        a, b = max(turn.start, cursor - .15), min(turn.end, stop + .15)
                        with wave.open(str(audio), 'rb') as source:
                            source.setpos(int(a * 16000))
                            raw = source.readframes(int((b - a) * 16000))
                        with wave.open(str(path), 'wb') as destination:
                            destination.setnchannels(1)
                            destination.setsampwidth(2)
                            destination.setframerate(16000)
                            destination.writeframes(raw)
                        chunks.append(dict(id=cid, audio=str(path), start=cursor, end=stop, speaker=turn.speaker))
                cursor = stop
    return chunks, captions


def process(store, project, kind, options, directory, cancel, progress):
    base_revision = project.revision
    cache = store.directory(project.id) / 'cache'
    cache.mkdir(exist_ok=True)
    if kind == 'import':
        if project.clips:
            raise ValueError('Import into an empty project; create a new project for another recording')
        groups = [(options['master'], 'master')]
        groups += [(p, cam) for cam in ('A', 'B') for p in options[cam]]
        clips = []
        for i, (path, camera) in enumerate(groups):
            clips.append(media.probe(path, camera, cancel))
            progress((i + 1) / len(groups))
        project.clips = clips
        project.duration = clips[0].duration
        store.save(project, base_revision)
        return dict(revision=base_revision + 1, clips=len(clips))
    if not project.clips:
        raise ValueError('Import media first')
    master = next(c for c in project.clips if c.camera == 'master')
    if kind == 'sample':
        start = options['start']
        end = min(project.duration, start + 15, options.get('end') or start + 15)
        if end <= start:
            raise ValueError('Sample must lie inside the recording')
        destination = directory / 'voice-sample.wav'
        media.run([media.binary('ffmpeg'), '-v', 'error', '-nostdin', '-y', '-ss', str(start),
                   '-i', master.path, '-t', str(end - start), '-vn', '-ac', '1', '-ar', '16000',
                   str(destination)], cancel)
        return dict(files=[destination.name], start=start, end=end)
    if kind == 'attach_cameras':
        groups = [(p, cam) for cam in ('A', 'B') for p in options[cam]]
        if not groups:
            raise ValueError('Choose at least one camera file')
        existing = {str(Path(c.path).resolve()) for c in project.clips}
        additions = []
        for index, (path, camera) in enumerate(groups):
            resolved = str(Path(path).resolve())
            if resolved in existing:
                raise ValueError('Camera file already imported: ' + path)
            clip = media.probe(path, camera, cancel)
            if not clip.has_video:
                raise ValueError('Camera sources must contain video: ' + path)
            additions.append(clip)
            existing.add(resolved)
            progress((index + 1) / len(groups))
        project.clips.extend(additions)
        project.cuts = [cut for cut in project.cuts if not cut.id.startswith('gap-')]
        project.reactions = []
        project.reaction_analysis = {}
        # Source coverage changed; preserve deliberate manual edits only.
        project.shots = [shot for shot in project.shots if shot.locked]
        store.save(project, base_revision)
        return dict(revision=base_revision + 1, clips_added=len(additions))
    if kind == 'sync':
        from .sync import synchronize
        project.cuts = [cut for cut in project.cuts if not cut.id.startswith('gap-')]
        project.reactions = []
        project.reaction_analysis = {}
        project.shots = [shot for shot in project.shots if shot.locked]
        progress(dict(stage='Preparing master playback'))
        media.playback_audio(master, cache, cancel)
        progress(dict(stage='Matching each camera shard to master'))
        master_audio, sr = media.read_wav(media.audio_copy(master, cache, rate=8000, cancel=cancel))
        results = {}
        cameras = [c for camera in ('A', 'B') for c in project.clips if c.camera == camera]
        for index, clip in enumerate(cameras):
            if clip.sync_locked:
                continue
            try:
                signal, _ = media.read_wav(media.audio_copy(clip, cache, rate=sr, cancel=cancel))
                # Every shard must find its own interval in the continuous master.
                # File picker order and a previous shard's match are not timing evidence.
                result = synchronize(master_audio, signal, sr, directory=directory, cancel=cancel,
                                     on_progress=lambda value: progress(dict(stage='Matching each camera shard to master',
                                         completed=index*5+round(value*5), total=len(cameras)*5, units='anchors',
                                         fraction=(index+value)/max(1,len(cameras)))))
                clip.offset, clip.rate, clip.sync_score = result['offset'], result['rate'], result['score']
                clip.sync_residual_seconds = result['residual_seconds']
                clip.sync_anchor_count = result.get('inlier_count', len(result['anchors']))
                clip.sync_reviewed = False
                clip.warnings = result['warnings']
                results[clip.id] = result
            except (ValueError, RuntimeError) as error:
                clip.sync_reviewed = False
                clip.sync_score = None
                clip.sync_residual_seconds = None
                clip.sync_anchor_count = 0
                clip.warnings = ['manual_sync_required', str(error)]
                results[clip.id] = dict(error=str(error))
            progress((index + 1) / max(1, len(cameras)))
        store.save(project, base_revision)
        return dict(revision=base_revision + 1, matches=results)
    if kind == 'waveform':
        path = media.audio_copy(master, cache, rate=2000, cancel=cancel)
        output = directory / 'waveform.json'
        output.write_text(json.dumps(media.waveform(path)))
        return dict(files=[output.name])
    if kind == 'diarize':
        progress(dict(stage='Preparing audio'))
        audio = media.audio_copy(master, cache, cancel=cancel)
        response = providers.invoke('diarize', dict(audio=str(audio)), cache, cancel, device=project.settings.compute_device, progress=progress)
        from .podcast import clean_activity
        turns, removed = clean_activity(response['activity'], project.duration)
        if not turns:
            raise ValueError('No usable speaker activity; review the recording or provider')
        labels = sorted({a.speaker for a in turns}, key=lambda s: min(a.start for a in turns if a.speaker == s))
        mapping = {label: f'SPEAKER_{i:02}' for i, label in enumerate(labels)}
        for turn in turns:
            turn.speaker = mapping[turn.speaker]
            turn.end = min(turn.end, project.duration)
        # Existing manual activity requires explicit replacement in job options.
        if project.activity and not options.get('replace_activity', False):
            raise ValueError('Activity already exists; set replace_activity=true only to replace it intentionally')
        project.activity = turns
        project.settings.roles_locked = False
        project.reactions = []
        project.reaction_analysis = {}
        project.shots = [shot for shot in project.shots if shot.locked]
        for caption in project.captions:
            caption.reviewed = False
        store.save(project, base_revision)
        return dict(revision=base_revision + 1, speakers=mapping, execution=response.get("_execution"), removed_invalid_predictions=removed, turns=len(turns), speaker_count_review_required=len(labels) != 2)
    if kind == 'filmcraft_export':
        from .remote_media import export_originals
        return export_originals(project, options, directory, cancel, progress)
    if kind == 'reference_frame':
        clip = next((c for c in project.clips if c.id == options['clip_id'] and c.has_video), None)
        if clip is None or not 0 <= options['time'] < clip.duration:
            raise ValueError('Choose a frame within a camera shard')
        if media.fingerprint(clip.path) != clip.fingerprint:
            raise ValueError('Source changed; reimport before previewing')
        media.run([media.binary('ffmpeg'), '-v', 'error', '-nostdin', '-y', '-ss', str(options['time']),
                   '-i', clip.path, '-frames:v', '1', '-vf', 'scale=960:-2', directory/'reference.png'], cancel)
        return dict(files=['reference.png'])
    if kind in ('reaction_preview', 'sync_preview'):
        if kind == 'reaction_preview':
            reaction = next((r for r in project.reactions if r.id == options['reaction_id']), None)
            if reaction is None:
                raise ValueError('Reaction no longer exists')
            clip = next(c for c in project.clips if c.id == reaction.clip_id)
            start, end = reaction.start, reaction.end
        else:
            clip = next((c for c in project.clips if c.id == options['clip_id'] and c.camera != 'master' and c.has_video), None)
            if clip is None:
                raise ValueError('Choose a camera recording')
            first, last = max(0., clip.offset), min(project.duration, clip.offset + clip.duration * clip.rate)
            start = first + max(0., last - first - 12.) * options.get('position', .5)
            end = min(last, start + 12.)
        if end <= start:
            raise ValueError('Camera and master audio do not overlap; correct the offset first')
        if media.fingerprint(clip.path) != clip.fingerprint:
            raise ValueError('Source changed; reimport before previewing')
        progress(dict(stage='Preparing synced preview'))
        destination = directory / 'reaction.mp4'
        length = end - start
        media.run([media.binary('ffmpeg'), '-v','error','-nostdin','-y',
            '-ss', str((start-clip.offset)/clip.rate), '-i', clip.path,
            '-ss', str(start), '-i', master.path,
            '-filter_complex', f'[0:v]setpts={clip.rate}*(PTS-STARTPTS),scale=640:-2[v]',
            '-map','[v]','-map','1:a:0','-t',str(length),'-c:v','libx264','-preset','fast',
            '-c:a','aac','-movflags','+faststart',destination],cancel)
        return dict(files=['reaction.mp4'])
    if kind == 'proxies':
        from .remote_media import make_proxies
        return make_proxies(project, cache, directory, cancel, progress)
    if kind == 'reaction_select':
        from .podcast import suggestions
        from .reaction_calibration import rescore_candidates, preserve_choices
        analysis = project.reaction_analysis
        if not analysis:
            raise ValueError('Find reactions first; interval changes then reuse the saved analysis')
        threshold = options.get('smile_threshold')
        if threshold is None:
            threshold = analysis.get('smile_threshold', .35)
        candidates = rescore_candidates(analysis, threshold)
        selected = suggestions(project, candidates, interval=options['interval'], spacing=options.get('spacing', analysis['spacing']), host_target=options.get('host_target'))
        project.reactions = preserve_choices(project, selected, analysis)
        analysis['selection_counts'] = dict(spans=len(candidates), matching=sum(any(c in row.get('cues', []) for c in ('smile', 'nod')) for row in candidates))
        analysis['interval'] = options['interval']
        analysis['smile_threshold'] = threshold
        analysis['spacing'] = options.get('spacing', analysis['spacing'])
        store.save(project, base_revision)
        return dict(revision=base_revision+1, suggestions=len(project.reactions))
    if kind in ('reactions', 'reference_suggestions'):
        from .podcast import suggestions
        if not project.settings.roles_locked or not project.shots:
            raise ValueError('Lock speakers, review sync and generate base cuts first')
        host_camera = project.settings.speaker_camera.get(project.settings.host_speaker)
        clips = [c for c in project.clips if c.camera == host_camera]
        if not clips or any(not c.sync_reviewed for c in clips):
            raise ValueError('Review host camera synchronization before reaction analysis')
        for clip in clips:
            samples = options.get('reference_samples', {}).get(clip.id, [])
            if len(samples) > 8 or any(not isinstance(t, (int, float)) or not 0 <= t < clip.duration for t in samples):
                raise ValueError('Use up to eight reference samples within each host recording')
            reference = samples[0] if samples else options['references'].get(clip.id)
            if kind == 'reactions' and not options.get('reference_image') and (reference is None or not 0 <= reference < clip.duration):
                raise ValueError('Choose a reference frame within every host camera shard')
            if media.fingerprint(clip.path) != clip.fingerprint:
                raise ValueError('Host source changed; reimport before analysis')
        options = dict(options)
        if options.get('reference_image'):
            image = Path(options['reference_image'])
            if image.suffix.lower() not in ('.png', '.jpg', '.jpeg') or not image.is_file() or image.stat().st_size > 20*1024*1024:
                raise ValueError('Choose a PNG/JPEG reference screenshot smaller than 20 MB')
            options['reference_fingerprint'] = media.fingerprint(image)
        host = project.settings.host_speaker
        windows = {c.id: [[max(0., (a.start-c.offset)/c.rate), min(c.duration, (a.end-c.offset)/c.rate)]
            for a in project.activity if a.speaker == host and a.end > c.offset and a.start < c.offset+c.duration*c.rate
            and not any(b.speaker != host and b.start < a.end and b.end > a.start for b in project.activity)] for c in clips}
        response = providers.invoke('reactions', dict(clips=[c.model_dump() for c in clips], reference_windows=windows, reference_only=kind == 'reference_suggestions', **options), cache, cancel, device=project.settings.compute_device, progress=progress)
        if kind == 'reference_suggestions':
            return dict(reference_candidates=response.get('reference_candidates', {}), issues=response.get('issues', {}))
        from .reaction_calibration import preserve_choices
        project.reaction_analysis = dict(candidates=response['candidates'], measurements=response.get('measurements', []), references=options['references'], reference_image=options.get('reference_image', ''), tolerance=options['tolerance'], interval=options['interval'], spacing=options['spacing'],
                                        smile_threshold=options.get('smile_threshold') if options.get('smile_threshold') is not None else .35,
                                        decisions=project.reaction_analysis.get('decisions', {}))
        selected = suggestions(project, response['candidates'], interval=options['interval'], spacing=options['spacing'], host_target=options.get('host_target'))
        project.reactions = preserve_choices(project, selected, project.reaction_analysis)
        project.reaction_analysis['selection_counts'] = dict(spans=len(response['candidates']), matching=sum(any(c in row.get('cues', []) for c in ('smile', 'nod')) for row in response['candidates']))
        store.save(project, base_revision)
        return dict(revision=base_revision+1, suggestions=len(project.reactions))
    if kind == 'shots':
        project.reactions = []
        project.reaction_analysis = {}
        project.shots = generate_shots(project)
        store.save(project, base_revision)
        return dict(revision=base_revision + 1, shots=len(project.shots))
    if kind == 'transcribe':
        progress(dict(stage='Preparing speech segments'))
        if not project.activity:
            raise ValueError('Diarize or enter speaker activity first')
        audio_paths = {None: media.audio_copy(master, cache, cancel=cancel)}
        for channel in set(project.settings.speaker_channel.values()):
            audio_paths[channel] = media.audio_copy(master, cache, channel=channel, cancel=cancel)
        chunks, captions = chunk_turns(project, audio_paths, cache)
        if chunks:
            response = providers.invoke('transcribe', dict(chunks=chunks, audio_fingerprint=master.fingerprint), cache, cancel, device=project.settings.compute_device, progress=progress)
            rows = response['captions']
            by_id = {row['id']: row['text'] for row in rows}
            if len(by_id) != len(rows) or set(by_id) != {c['id'] for c in chunks}:
                raise ValueError('Transcription worker must return every requested ID exactly once')
            if any(not isinstance(text, str) for text in by_id.values()):
                raise ValueError('Transcription text must be strings')
            for caption in captions:
                caption.text = by_id.get(caption.id, '')
        project.captions = sorted([c for c in project.captions if c.locked or c.reviewed] + captions, key=lambda c: (c.start, c.speaker))
        saved = store.save(project, base_revision)
        baseline_path = store.directory(project.id) / 'baseline.json'
        if not baseline_path.exists():
            baseline_path.write_text(saved.model_dump_json(indent=2), encoding='utf-8')
        return dict(revision=base_revision + 1, captions=len(project.captions), overlap_review=sum(c.overlap for c in captions))
    if kind == 'translate':
        selected = [c for c in project.captions if c.reviewed and not c.locked]
        if not selected:
            raise ValueError('No reviewed, unlocked captions to translate')
        response = providers.invoke('translate', dict(source_language='or', target_language='en',
                                    entries=[dict(id=c.id, text=c.text) for c in selected]), cache, cancel)
        rows = response['entries']
        by_id = {r['id']: r['text'] for r in rows}
        if len(by_id) != len(rows) or set(by_id) != {c.id for c in selected} or any(not isinstance(t, str) for t in by_id.values()):
            raise ValueError('Translation worker must return each requested ID exactly once with string text')
        for caption in selected:
            caption.translation = by_id[caption.id]
        store.save(project, base_revision)
        return dict(revision=base_revision + 1)
    if kind == 'transliterate':
        result = providers.invoke(kind, dict(text=options['text']), cache, cancel)
        if not isinstance(result.get('text'), str):
            raise ValueError('Transliteration provider must return text')
        return result
    if kind in ('preview', 'export'):
        return render(project, directory, preview=kind == 'preview', start=options.get('start', 0),
                      end=options.get('end'), cancel=cancel, progress=progress)
    raise ValueError('Unknown job kind')
