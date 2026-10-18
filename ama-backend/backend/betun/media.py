import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import signal
import sys
import threading
from fractions import Fraction
import time
import numpy as np
from .schema import Clip


class Cancelled(Exception):
    pass


_spawn_lock = threading.Lock()


def external_process(args, **kwargs):
    """Do not make external Python/FFmpeg load the frozen helper's private DLLs.

    See PyInstaller common-issues-and-pitfalls, launching external programs on Windows.
    Restore the helper's DLL search directory immediately after creating the child.
    """
    bundle = getattr(sys, '_MEIPASS', None)
    if os.name != 'nt' or not getattr(sys, 'frozen', False) or not bundle:
        return subprocess.Popen(args, **kwargs)
    import ctypes
    kernel = ctypes.WinDLL('kernel32', use_last_error=True)
    set_directory = kernel.SetDllDirectoryW
    set_directory.argtypes = [ctypes.c_wchar_p]
    set_directory.restype = ctypes.c_int
    environment = dict(os.environ)
    directory = Path(bundle).resolve()
    environment['PATH'] = os.pathsep.join(p for p in environment.get('PATH', '').split(os.pathsep)
        if p and not Path(p).resolve().is_relative_to(directory))
    with _spawn_lock:
        if not set_directory(None):
            raise ctypes.WinError(ctypes.get_last_error())
        try:
            return subprocess.Popen(args, env=environment, **kwargs)
        finally:
            if not set_directory(str(directory)):
                raise ctypes.WinError(ctypes.get_last_error())


def binary(name):
    configured = os.environ.get('BETUN_' + name.upper())
    path = configured
    if not path and os.environ.get('LOCALAPPDATA'):
        settings = Path(os.environ['LOCALAPPDATA']) / 'YouCraft/media-tools.json'
        if settings.is_file():
            candidate = json.loads(settings.read_text(encoding='utf-8')).get(name)
            if candidate and Path(candidate).is_file():
                path = candidate
    path = path or shutil.which(name)
    if not path or not Path(path).is_file():
        raise RuntimeError(f"{name} unavailable; install it or set BETUN_{name.upper()} to its executable")
    return str(path)


def run(args, cancel=lambda: False, timeout=3600, on_poll=None):
    # File-backed output prevents pipe deadlocks and unbounded RAM for noisy tools.
    import tempfile
    with tempfile.TemporaryFile() as output, tempfile.TemporaryFile() as errors:
        with external_process([str(x) for x in args], stdin=subprocess.DEVNULL,
                              stdout=output, stderr=errors, start_new_session=os.name != 'nt',
                              creationflags=(subprocess.CREATE_NEW_PROCESS_GROUP | subprocess.CREATE_NO_WINDOW) if os.name == 'nt' else 0) as process:
            started = time.monotonic()
            try:
                while process.poll() is None:
                    if on_poll:
                        on_poll()
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


def playback_audio(clip, directory, cancel=lambda: False):
    """One canonical PCM copy for the native player, preserving stereo dialogue."""
    with _playback_lock:
        if fingerprint(clip.path) != clip.fingerprint:
            raise ValueError('Master audio changed; relink the recording')
        directory = Path(directory)
        directory.mkdir(parents=True, exist_ok=True)
        target = directory / f'{clip.fingerprint}-playback-48000-stereo.wav'
        if target.is_file():
            return target
        temporary = target.with_suffix('.partial.wav')
        try:
            run([binary('ffmpeg'), '-v', 'error', '-nostdin', '-y', '-i', clip.path,
                 '-map', '0:a:0', '-vn', '-ac', '2', '-ar', '48000', '-c:a', 'pcm_s16le', temporary], cancel)
            temporary.replace(target)
        finally:
            temporary.unlink(missing_ok=True)
        return target


_playback_lock = threading.Lock()
