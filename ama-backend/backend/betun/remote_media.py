"""Small preview media generated on the server, then cached by the Windows editor."""
import json
from . import media
from .podcast import manifest


def make_proxies(project, cache, directory, cancel, progress):
    plan = manifest(project)
    files = []
    for index, clip in enumerate(project.clips):
        if media.fingerprint(clip.path) != clip.fingerprint:
            raise ValueError('Source changed; reimport before generating proxies')
        suffix = '.mp4' if clip.has_video else '.wav'
        filename = f'{clip.fingerprint}-preview-v1{suffix}'
        path = cache / filename
        if not path.exists():
            partial = path.with_name(path.stem + '.partial' + suffix)
            args = [media.binary('ffmpeg'), '-v', 'error', '-nostdin', '-y', '-i', clip.path]
            if clip.has_video:
                args += ['-map', '0:v:0', '-an', '-vf', 'scale=960:540:force_original_aspect_ratio=decrease,pad=960:540:(ow-iw)/2:(oh-ih)/2,setsar=1',
                         '-c:v', 'libx264', '-preset', 'fast', '-crf', '27', '-pix_fmt', 'yuv420p', '-movflags', '+faststart']
            else:
                args += ['-vn', '-ac', '2', '-ar', '48000', '-c:a', 'pcm_s16le']
            try:
                media.run([*args, partial], cancel, timeout=86400)
                partial.replace(path)
            finally:
                partial.unlink(missing_ok=True)
        asset = next(a for a in plan['assets'] if a['id'] == clip.id)
        asset['proxy'] = filename
        files.append(filename)
        progress((index+1)/len(project.clips))
    (directory/'filmcraft.json').write_text(json.dumps(plan,ensure_ascii=False),encoding='utf-8')
    return dict(files=['filmcraft.json'], proxy_files=files)


def export_originals(project, options, directory, cancel, progress):
    """Render the exact native project snapshot with the same Filmcraft CLI, not Betun's renderer."""
    import os
    from pathlib import Path
    cli = os.environ.get('BETUN_FILMCRAFT_CLI')
    if not cli or not Path(cli).is_file():
        raise ValueError('Set BETUN_FILMCRAFT_CLI to the matching Filmcraft CLI build for server export')
    snapshot = options['snapshot']
    if len(json.dumps(snapshot)) > 32*1024*1024:
        raise ValueError('Project snapshot exceeds 32 MB')
    originals = {f'{c.fingerprint}-preview-v1' + ('.mp4' if c.has_video else '.wav'): c for c in project.clips}
    originals.update({c.path:c for c in project.clips})
    # Voice/reaction previews may be in the bin but are not part of the edited sequence.
    # Follow only referenced sequence/subclip media; unrelated bin items need no upload.
    items = snapshot.get('items', {})
    reachable, pending = set(), [str(options['sequence'])]
    while pending:
        item_id = pending.pop()
        if item_id in reachable:
            continue
        if len(reachable) > 100000:
            raise ValueError('Project reference graph is too large')
        reachable.add(item_id)
        kind = items.get(item_id, {}).get('kind', {})
        sequence = kind.get('Sequence')
        if sequence:
            for track in sequence.get('video_tracks', []) + sequence.get('audio_tracks', []):
                pending.extend(str(c['item']) for c in track.get('items', []))
        subclip = kind.get('Subclip')
        if subclip:
            pending.append(str(subclip['parent']))
    if str(options['sequence']) not in items:
        raise ValueError('Selected sequence is missing from project snapshot')
    relinks = []
    for item_id, item in snapshot.get('items', {}).items():
        m = item.get('kind', {}).get('Media')
        if not m:
            continue
        source = m.get('media', {}).get('File')
        if source is None:
            continue
        path = source['path']
        # Paths are produced on Windows and consumed on Linux.
        name = path.replace('\\','/').rsplit('/',1)[-1]
        clip = originals.get(path) or originals.get(name)
        if clip is None and str(item_id) not in reachable:
            m['offline'] = True
            continue
        if clip is None:
            raise ValueError('Upload/import all added media before remote export: ' + name)
        if media.fingerprint(clip.path) != clip.fingerprint:
            raise ValueError('Original source changed: ' + clip.path)
        source['path'] = clip.path
        m['proxy'] = None
        m['offline'] = False
        relinks.append(dict(id='media.relink', params=dict(item=int(item_id), path=clip.path, force=True, relinkOthers=False)))
    document = directory/'snapshot.fcproj'
    document.write_text(json.dumps(dict(format='filmcraft.project',schema_version=15,
        generator='Ama Bartalap',project=snapshot)),encoding='utf-8')
    script=directory/'render.jsonl'
    commands=[dict(id='sequence.open',params=dict(item=options['sequence'])),*relinks,
        dict(id='podcast.exportSrt',params=dict(path=str(directory/'episode.or.srt'))),
        dict(id='file.exportMedia',params=dict(path=str(directory/'episode.mp4'),format='h264',wait=True))]
    script.write_text('\n'.join(json.dumps(c) for c in commands),encoding='utf-8')
    progress(.05)
    media.run([cli,'--project',document,'run',script],cancel,timeout=86400)
    if not (directory/'episode.mp4').is_file():
        raise ValueError('Filmcraft did not produce the expected video')
    return dict(files=['episode.mp4','episode.or.srt','snapshot.fcproj'])
