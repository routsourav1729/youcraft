"""Review-based diagnostics; these are not a published benchmark or formal DER."""
import unicodedata
from collections import Counter


def distance(a, b):
    previous = list(range(len(b) + 1))
    for i, x in enumerate(a, 1):
        current = [i]
        for j, y in enumerate(b, 1):
            current.append(min(current[-1] + 1, previous[j] + 1, previous[j - 1] + (x != y)))
        previous = current
    return previous[-1]


def normalized(text):
    return ' '.join(unicodedata.normalize('NFC', text).split())


def compare(baseline, reference):
    predicted = {c.id: c for c in baseline.captions}
    references = [c for c in reference.captions if c.reviewed and c.id in predicted]
    word_errors = char_errors = words = chars = 0
    for caption in references:
        a, b = normalized(predicted[caption.id].text), normalized(caption.text)
        word_errors += distance(a.split(), b.split())
        char_errors += distance(list(a), list(b))
        words += len(b.split())
        chars += len(b)
    points = sorted({x for turn in [*baseline.activity, *reference.activity] for x in (turn.start, turn.end)})
    missed = extra = confused = overlap_predicted = overlap_reference = overlap_correct = 0.
    for start, end in zip(points, points[1:]):
        a = {t.speaker for t in baseline.activity if t.start < end and t.end > start}
        b = {t.speaker for t in reference.activity if t.start < end and t.end > start}
        duration = end - start
        if not a and b: missed += duration
        if a and not b: extra += duration
        if a and b and a != b: confused += duration
        if len(a) > 1: overlap_predicted += duration
        if len(b) > 1: overlap_reference += duration
        if len(a) > 1 and len(b) > 1: overlap_correct += duration
    return dict(reviewed_caption_count=len(references), word_error_rate=word_errors / words if words else None,
                character_error_rate=char_errors / chars if chars else None,
                speech_missed_seconds=missed, speech_extra_seconds=extra,
                differing_speaker_activity_seconds=confused,
                overlap_precision=overlap_correct / overlap_predicted if overlap_predicted else None,
                overlap_recall=overlap_correct / overlap_reference if overlap_reference else None,
                caveat='Diagnostics against your corrected reference; not formal DER. Ensure speaker labels match. Odia WER is sensitive to spacing/spelling; CER helps interpret it.')
