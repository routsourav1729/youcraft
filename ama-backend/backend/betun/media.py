import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import signal
from fractions import Fraction
import time
import numpy as np
from .schema import Clip


class Cancelled(Exception):
    pass


def binary(name):
    configured = os.environ.get('BETUN_' + name.upper())
    path = configured or shutil.which(name)
    if not path or not Path(path).is_file():
        raise RuntimeError(f"{name} unavailable; install it or set BETUN_{name.upper()} to its executable")
    return str(path)


def run(args, cancel=lambda: False, timeout=3600):
    # File-backed output prevents pipe deadlocks and unbounded RAM for noisy tools.
    import tempfile
    with tempfile.TemporaryFile() as output, tempfile.TemporaryFile() as errors:
        with subprocess.Popen([str(x) for x in args], stdin=subprocess.DEVNULL,
                              stdout=output, stderr=errors, start_new_session=os.name != 'nt',
                              creationflags=subprocess.CREATE_NEW_PROCESS_GROUP if os.name == 'nt' else 0) as process:
            started = time.monotonic()
            try:
                while process.poll() is None:
                    if cancel():
                        raise Cancelled('Job cancelled')
                    if time.monotonic() - started > timeout:
                        raise TimeoutError('Media command timed out')
                    time.sleep(.05)
            except BaseException:
                if os.name == 'nt':
                    # Kill only this job's process tree, including ML subprocesses.
                    subprocess.run(['taskkill', '/PID', str(process.pid), '/T', '/F'],
                                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, check=False)
                else:
                    try:
                        os.killpg(process.pid, signal.SIGTERM)
                    except ProcessLookupError:
                        pass
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    if os.name != 'nt':
                        try:
                            os.killpg(process.pid, signal.SIGKILL)
                        except ProcessLookupError:
                            pass
                    else:
                        process.kill()
                    process.wait()
                raise
            if process.returncode:
                errors.seek(max(0, errors.tell() - 6000))
                raise RuntimeError(errors.read().decode(errors='replace'))
            output.seek(0)
            return output.read()


def fingerprint(path):
    p = Path(path).resolve(strict=True)
    st = p.stat()
    # Fast identity for large source media; size/mtime + beginning/end content.
    h = hashlib.sha256(f'{p}:{st.st_size}:{st.st_mtime_ns}'.encode())
    with p.open('rb') as f:
        h.update(f.read(1024 * 1024))
        if st.st_size > 1024 * 1024:
            f.seek(max(0, st.st_size - 1024 * 1024))
            h.update(f.read(1024 * 1024))
    return h.hexdigest()


def probe(path, camera, cancel=lambda: False):
    path = Path(path).expanduser().resolve(strict=True)
    if not path.is_file():
        raise ValueError('Media path must be a file')
    data = json.loads(run([binary('ffprobe'), '-v', 'error', '-show_format', '-show_streams', '-of', 'json', path], cancel, 60))
    audio = next((s for s in data['streams'] if s['codec_type'] == 'audio'), None)
    video = next((s for s in data['streams'] if s['codec_type'] == 'video'), None)
    if camera == 'master' and audio is None:
        raise ValueError('Master must contain audio')
    if camera != 'master' and video is None:
        raise ValueError('Camera source must contain video')
    frame_rate = None
    if video:
        try:
            frame_rate = float(Fraction(video.get('avg_frame_rate') or video.get('r_frame_rate', '0/1')))
            if not 0 < frame_rate <= 240:
                frame_rate = None
        except (ValueError, ZeroDivisionError):
            pass
    # Timeline is relative to each file's first decoded audio/video timestamp.
    return Clip(path=str(path), camera=camera, duration=float(data['format']['duration']),
                channels=audio.get('channels', 0) if audio else 0,
                has_video=video is not None, frame_rate=frame_rate, fingerprint=fingerprint(path),
                warnings=[] if audio else ['no_camera_audio_manual_sync_required'])


def audio_copy(clip, directory, rate=16000, channel=None, cancel=lambda: False):
    if fingerprint(clip.path) != clip.fingerprint:
        raise ValueError(f'Source changed: {clip.path}; reimport before processing')
    destination = directory / f'{clip.fingerprint}-{rate}-{channel}.wav'
    if destination.exists():
        return destination
    temporary = destination.with_suffix('.partial.wav')
    args = [binary('ffmpeg'), '-v', 'error', '-nostdin', '-y', '-i', clip.path, '-vn']
    if channel is not None:
        if not 0 <= channel < clip.channels:
            raise ValueError('Invalid audio channel')
        args += ['-af', f'pan=mono|c0=c{channel}']
    args += ['-ac', '1', '-ar', str(rate), '-c:a', 'pcm_s16le', temporary]
    try:
        run(args, cancel)
        temporary.replace(destination)
    finally:
        temporary.unlink(missing_ok=True)
    return destination


def read_wav(path):
    import wave
    with wave.open(str(path), 'rb') as f:
        if f.getsampwidth() != 2 or f.getnchannels() != 1:
            raise ValueError('Expected 16-bit mono processing WAV')
        return np.frombuffer(f.readframes(f.getnframes()), dtype='<i2').astype(np.float32) / 32768, f.getframerate()


def waveform(path, bins=2000):
    samples, sr = read_wav(path)
    stride = max(1, int(np.ceil(len(samples) / bins)))
    return dict(sample_rate=sr, seconds_per_bin=stride / sr,
                peaks=[float(np.max(np.abs(samples[i:i + stride]))) for i in range(0, len(samples), stride)])
