import json
import os
from pathlib import Path
from .media import binary, run, fingerprint
from .timeline import render_plan, srt, edited_captions, time_map


def encoder():
    requested = os.environ.get('BETUN_VIDEO_ENCODER')
    if requested:
        if requested not in ('libx264', 'libopenh264', 'h264_nvenc', 'h264_videotoolbox'):
            raise ValueError('Unsupported BETUN_VIDEO_ENCODER')
        return requested
    encoders = run([binary('ffmpeg'), '-hide_banner', '-encoders']).decode()
    for name in ('libx264', 'libopenh264'):
        if name in encoders:
            return name
    raise RuntimeError('FFmpeg needs an H.264 encoder (libx264 or libopenh264)')


def render(project, directory, preview=False, start=0., end=None, cancel=lambda: False, progress=lambda p: None):
    if not preview:
        if any(not c.sync_reviewed for c in project.clips if c.camera != 'master'):
            raise ValueError('Review camera synchronization before final export')
        if any(not c.reviewed for c in project.captions):
            raise ValueError('Review all transcript segments before final export')
        if not project.shots:
            raise ValueError('Final export needs a camera edit')
    for clip in project.clips:
        if fingerprint(clip.path) != clip.fingerprint:
            raise ValueError('Source media changed; reimport before rendering')
    if preview:
        end = min(project.duration, start + 30, end if end is not None else project.duration)
    plan = render_plan(project, start if preview else 0., end if preview else None)
    if not plan:
        raise ValueError('Nothing to render')
    directory.mkdir(parents=True, exist_ok=True)
    width, height, fps = (640, 360, 25) if preview else (1920, 1080, 25)
    codec = encoder()
    parts = []
    for index, segment in enumerate(plan):
        if cancel():
            from .media import Cancelled
            raise Cancelled()
        # Cumulative rounding prevents per-shot frame errors accumulating over an episode.
        frames = round(segment['edited_end'] * fps) - round(segment['edited_start'] * fps)
        if frames <= 0:
            continue
        duration = frames / fps
        destination = directory / f'part-{index:06}.mp4'
        partial = destination.with_suffix('.partial.mp4')
        args = [binary('ffmpeg'), '-v', 'error', '-nostdin', '-y', '-filter_complex_threads', '1']
        filters = []
        sources = list(segment['sources'].values())
        for source in sources:
            args += ['-ss', str(source['start']), '-i', source['path']]
        if not sources:
            args += ['-f', 'lavfi', '-i', f'color=c=black:s={width}x{height}:r={fps}']
            filters = ['[0:v]setsar=1[v]']
        else:
            for i, source in enumerate(sources):
                w = width // 2 if len(sources) == 2 else width
                filters.append(f'[{i}:v:0]setpts=(PTS-STARTPTS)*{source["rate"]},scale={w}:{height}:force_original_aspect_ratio=decrease,pad={w}:{height}:(ow-iw)/2:(oh-ih)/2,setsar=1,fps={fps},tpad=stop_mode=clone:stop_duration=1[v{i}]')
            filters.append('[v0][v1]hstack=inputs=2[v]' if len(sources) == 2 else '[v0]null[v]')
        args += ['-filter_complex', ';'.join(filters), '-map', '[v]', '-an', '-frames:v', str(frames), '-c:v', codec, '-threads', '2', '-pix_fmt', 'yuv420p']
        if codec == 'libx264':
            args += ['-preset', 'veryfast' if preview else 'medium', '-crf', '27' if preview else '19']
        else:
            args += ['-b:v', '1M' if preview else '8M']
        args += [partial]
        run(args, cancel)
        partial.replace(destination)
        parts.append(destination)
        progress(.85 * (index + 1) / len(plan))
    if not parts:
        raise ValueError('Render is shorter than one frame')
    listing = directory / 'concat.txt'
    # Only generated safe basenames appear in the concat file.
    listing.write_text('\n'.join(f"file '{p.name}'" for p in parts), encoding='utf-8')
    master = next(c for c in project.clips if c.camera == 'master')
    ranges = []
    for segment in plan:
        a, b = segment['source_start'], segment['source_end']
        if ranges and abs(ranges[-1][1] - a) < 1e-6:
            ranges[-1] = (ranges[-1][0], b)
        else:
            ranges.append((a, b))
    # A single AAC encode avoids an encoder-delay gap at every camera cut.
    filters = [f'[1:a:0]atrim=start={a}:end={b},asetpts=PTS-STARTPTS[a{i}]' for i, (a, b) in enumerate(ranges)]
    filters.append(''.join(f'[a{i}]' for i in range(len(ranges))) + f'concat=n={len(ranges)}:v=0:a=1[a]')
    script = directory / 'audio-filter.txt'
    script.write_text(';'.join(filters), encoding='utf-8')
    output = directory / ('preview.mp4' if preview else 'episode.mp4')
    partial = output.with_suffix('.partial.mp4')
    args = [binary('ffmpeg'), '-v', 'error', '-nostdin', '-y', '-f', 'concat', '-safe', '1', '-i', listing,
            '-i', master.path, '-/filter_complex', script, '-map', '0:v:0', '-map', '[a]',
            '-c:v', 'copy', '-c:a', 'aac', '-b:a', '192k', '-movflags', '+faststart', partial]
    run_filter_file(args, cancel)
    partial.replace(output)
    for part in parts:
        part.unlink()
    files = [output.name]
    if not preview:
        (directory / 'odia.srt').write_text(srt(project), encoding='utf-8')
        (directory / 'project.betun.json').write_text(project.model_dump_json(indent=2), encoding='utf-8')
        (directory / 'transcript.json').write_text(json.dumps(dict(entries=edited_captions(project), time_map=time_map(project)), ensure_ascii=False, indent=2), encoding='utf-8')
        (directory / 'transcript.txt').write_text('\n'.join(f'[{c.start:.3f}–{c.end:.3f}] {c.speaker}: {c.text}' for c in project.captions), encoding='utf-8')
        files += ['odia.srt', 'project.betun.json', 'transcript.json', 'transcript.txt']
    progress(1.)
    return dict(files=files, revision=project.revision, plan=plan, frame_rate=fps)


def run_filter_file(args, cancel):
    try:
        run(args, cancel)
    except RuntimeError as error:
        # FFmpeg 7+ reads option values from files with -/; older builds used
        # -filter_complex_script, which newer releases have removed.
        if "Unrecognized option '/filter_complex'" not in str(error):
            raise
        legacy = ['-filter_complex_script' if arg == '-/filter_complex' else arg for arg in args]
        run(legacy, cancel)
