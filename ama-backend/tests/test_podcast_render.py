import json
from pathlib import Path
import wave
import pytest
from betun import media
from betun.render import render
from betun.schema import Project, Shot


def test_camera_edit_exports_without_transcript(tmp_path):
    try:
        media.binary('ffmpeg')
        media.binary('ffprobe')
    except RuntimeError:
        pytest.skip('FFmpeg integration fixture unavailable')
    master = tmp_path / 'master.wav'
    with wave.open(str(master), 'wb') as audio:
        audio.setnchannels(1)
        audio.setsampwidth(2)
        audio.setframerate(16000)
        audio.writeframes(b'\x01\x00' * 32000)
    clips = [media.probe(str(master), 'master')]
    for camera, color in [('A', 'blue'), ('B', 'green')]:
        path = tmp_path / (camera + '.mp4')
        media.run([media.binary('ffmpeg'), '-v','error','-f','lavfi','-i',
                   f'color=c={color}:s=160x90:r=25:d=2', '-c:v','mpeg4',str(path)])
        clip = media.probe(str(path), camera)
        clip.sync_reviewed = True
        clips.append(clip)
    p = Project(name='No captions needed', duration=2, clips=clips,
                shots=[Shot(start=0,end=1,layout='A'), Shot(start=1,end=2,layout='both')])
    result = render(p, tmp_path/'render')
    assert 'episode.mp4' in result['files']
    info = json.loads(media.run([media.binary('ffprobe'), '-v','error','-show_streams',
                               '-show_format','-of','json',str(tmp_path/'render/episode.mp4')]))
    assert {s['codec_type'] for s in info['streams']} == {'audio','video'}
    assert abs(float(info['format']['duration']) - 2) < .15
