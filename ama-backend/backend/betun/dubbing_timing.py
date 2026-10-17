"""Fit complete speech onto a source clock without time-stretch or truncation."""
import numpy as np


def fit_status(speech_seconds, slot_seconds, minimum_ratio=.9, sample_rate=48000):
    if not np.isfinite(speech_seconds) or speech_seconds <= 0 or slot_seconds <= 0:
        return 'invalid_audio'
    if speech_seconds > slot_seconds + .5 / sample_rate:
        return 'overrun_rewrite_required'
    if speech_seconds < minimum_ratio * slot_seconds:
        return 'too_short_rewrite_required'
    return 'fits_with_natural_pause_padding'


def trim_edge_silence(samples, sample_rate=48000, threshold=.001, pad_seconds=.08):
    """Trim only quiet outer edges; no internal silence or spoken samples are cut."""
    samples=np.asarray(samples,dtype=np.float32).reshape(-1)
    if not len(samples) or not np.isfinite(samples).all():raise ValueError('Invalid generated audio')
    frame=max(1,round(.02*sample_rate))
    active=[i for i in range(0,len(samples),frame) if np.sqrt(np.mean(samples[i:i+frame]**2))>=threshold]
    if not active:raise ValueError('Generated audio contains no usable speech')
    pad=round(pad_seconds*sample_rate)
    start=max(0,active[0]-pad);end=min(len(samples),active[-1]+frame+pad)
    return samples[start:end]


def place_complete_speech(destination,speech,start,slot_end,sample_rate=48000):
    start_frame=round(start*sample_rate);end_frame=round(slot_end*sample_rate)
    if start_frame<0 or end_frame>len(destination):raise ValueError('Slot outside output timeline')
    if len(speech)>end_frame-start_frame:raise ValueError('Refusing to truncate or stretch an overrun')
    destination[start_frame:start_frame+len(speech)]+=speech
