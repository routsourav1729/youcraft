"""Exercise the real converter with the formats used in the downloadable fixture."""
import hashlib
import wave
import pytest
from betun import media

@pytest.mark.parametrize('extension', ['wav', 'mp3', 'mp4', 'mov'])
def test_supported_inputs_normalize_without_changing_original(tmp_path, extension):
    try:
        ffmpeg = media.binary('ffmpeg')
        media.binary('ffprobe')
    except RuntimeError:
        pytest.skip('External FFmpeg tools unavailable')
    source = tmp_path / ('input.' + extension)
    args = [ffmpeg, '-v', 'error', '-y', '-f', 'lavfi', '-i', 'sine=frequency=440:sample_rate=48000:duration=1.2']
    if extension in ('mp4', 'mov'):
        args += ['-f', 'lavfi', '-i', 'color=c=blue:s=160x90:r=25:d=1.2', '-c:v', 'libx264', '-pix_fmt', 'yuv420p', '-c:a', 'aac']
    args += ['-ac', '2', str(source)]
    media.run(args)
    original = hashlib.sha256(source.read_bytes()).digest()
    clip = media.probe(source, 'master' if extension in ('wav', 'mp3') else 'A')
    prepared = media.audio_copy(clip, tmp_path)
    with wave.open(str(prepared), 'rb') as audio:
        assert (audio.getnchannels(), audio.getframerate(), audio.getsampwidth()) == (1, 16000, 2)
        assert abs(audio.getnframes() / 16000 - 1.2) < .06
    assert hashlib.sha256(source.read_bytes()).digest() == original
    assert media.audio_copy(clip, tmp_path) == prepared
