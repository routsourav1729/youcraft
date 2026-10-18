"""Optional ML worker. Run in a separate provider-specific Python environment, not the app venv.
Protocol: python model_worker.py REQUEST_JSON RESULT_JSON.
Speech/transliteration weights use configured local directories/files. Prepare the
optional upstream vision runtime and its detector cache before offline inference.
"""
import json
import os
from pathlib import Path
import sys


def main(request_path, result_path):
    os.environ.setdefault('OMP_NUM_THREADS', '4')
    os.environ.setdefault('MKL_NUM_THREADS', '4')
    os.environ.setdefault('HF_HUB_OFFLINE', '1')
    os.environ.setdefault('TRANSFORMERS_OFFLINE', '1')
    request = json.loads(Path(request_path).read_text(encoding='utf-8'))
    config, payload, task = request['config'], request['payload'], request['task']
    def progress(stage, **details):
        if request.get('progress_path'):
            destination = Path(request['progress_path'])
            temporary = destination.with_suffix('.tmp')
            try:
                temporary.write_text(json.dumps(dict(stage=stage, **details)), encoding='utf-8')
                temporary.replace(destination)
            except OSError:
                pass  # Progress must not abort inference if a Windows reader holds the file.
    progress('Loading runtime')
    provider = config['provider']
    model = Path(config.get('model_path', '.')).resolve()
    if not model.exists():
        raise ValueError('Configured model files are missing')
    device = config.get('device', 'cpu')
    if task in ('diarize', 'transcribe') or (task == 'reactions' and provider == 'mediapipe-l2cs'):
        import torch
        torch.set_num_threads(int(config.get('cpu_threads', 4)))
        if device == 'auto':
            device = 'cuda' if torch.cuda.is_available() else 'cpu'
        if device.startswith('cuda'):
            selected = torch.device(device).index
            device = f'cuda:{torch.cuda.current_device() if selected is None else selected}'
        if device.startswith('cuda') and config.get('max_vram_gb'):
            total = torch.cuda.get_device_properties(device).total_memory
            fraction = min(.9, float(config['max_vram_gb']) * 1024**3 / total)
            if not 0 < fraction <= 1:
                raise ValueError('Invalid VRAM budget')
            torch.cuda.set_per_process_memory_fraction(fraction, device)
        config = dict(config, device=device)

    progress('Loading model')
    if task == 'diarize' and provider == 'pyannote':
        import torch
        from pyannote.audio import Pipeline
        pipeline = Pipeline.from_pretrained(str(model))
        pipeline.to(torch.device(device))
        progress('Identifying speakers')
        output = pipeline(payload['audio'], num_speakers=2)
        annotation = getattr(output, 'speaker_diarization', output)
        rows = [dict(start=float(t.start), end=float(t.end), speaker=str(s)) for t, _, s in annotation.itertracks(yield_label=True)]
        result = dict(activity=rows)
    elif task == 'diarize' and provider == 'nemotron-transformers':
        import torch
        import soundfile as sf
        from transformers import AutoModelForAudioFrameClassification, AutoProcessor
        audio, rate = sf.read(payload['audio'], dtype='float32')
        if rate != 16000 or audio.ndim != 1:
            raise ValueError('Nemotron requires the prepared 16 kHz mono copy')
        processor = AutoProcessor.from_pretrained(str(model), local_files_only=True)
        diarizer = AutoModelForAudioFrameClassification.from_pretrained(str(model), local_files_only=True).to(device).eval()
        progress('Preparing speech features')
        inputs = processor(audio, sampling_rate=rate).to(device, dtype=diarizer.dtype)
        progress('Identifying speakers')
        with torch.inference_mode():
            logits = diarizer(**inputs).logits
        segments = processor.extract_speaker_dict(logits, inputs.attention_mask)[0]
        result = dict(activity=[dict(start=float(t['Start']), end=float(t['End']), speaker=str(t['Speaker'])) for t in segments])
    elif task == 'diarize' and provider == 'nemotron':
        from nemo.collections.asr.models import SortformerEncLabelModel
        diarizer = SortformerEncLabelModel.restore_from(str(model), map_location=device)
        diarizer.eval()
        diarizer.sortformer_modules.chunk_len = 340
        diarizer.sortformer_modules.chunk_right_context = 40
        diarizer.sortformer_modules.fifo_len = 40
        diarizer.sortformer_modules.spkcache_update_period = 300
        diarizer.sortformer_modules.spkcache_len = 264
        diarizer._check_streaming_parameters()
        output = diarizer.diarize(audio=[payload['audio']], batch_size=1)
        rows = []
        for line in output[0]:
            start, end, speaker = line.split()
            rows.append(dict(start=float(start), end=float(end), speaker=speaker))
        result = dict(activity=rows)
    elif task == 'transcribe' and provider == 'indic-core':
        # Model bundle contains its own Python implementation. Only install trusted revisions.
        sys.path.insert(0, str(model))
        from indic_transcribe import IndicTranscribe
        from long_form import transcribe_long
        asr = IndicTranscribe.from_pretrained(str(model), device=device)
        captions = []
        chunks = payload['chunks']
        for index, chunk in enumerate(chunks):
            progress('Transcribing', completed=index, total=len(chunks), fraction=index / max(1, len(chunks)))
            captions.append(dict(id=chunk['id'], text=transcribe_long(asr, chunk['audio'], lang='or')))
        progress('Transcription complete', completed=len(chunks), total=len(chunks), fraction=1.)
        result = dict(captions=captions)
    elif task == 'reactions' and provider in ('mediapipe-l2cs', 'mediapipe-reference'):
        from vision_worker import analyze
        result = analyze(payload, config, progress)
    elif task == 'transliterate' and provider == 'indicxlit':
        # Initialize the upstream engine from an explicit local bundle; no implicit downloads.
        os.environ['CUDA_VISIBLE_DEVICES'] = ''
        from ai4bharat.transliteration.transformer.en2indic import XlitEngineTransformer_En2Indic
        from ai4bharat.transliteration.transformer.base_engine import BaseEngineTransformer
        if not (model / 'transformer' / 'indicxlit.pt').is_file():
            raise ValueError('IndicXlit model_path must contain transformer/indicxlit.pt and corpus-bin')
        engine = XlitEngineTransformer_En2Indic.__new__(XlitEngineTransformer_En2Indic)
        engine._all_supported_langs = set((model.parent / 'lang_list.txt').read_text(encoding='utf-8').split()) - {'en'}
        engine._tgt_langs = {'or'}
        BaseEngineTransformer.__init__(engine, str(model), beam_width=5, rescore=False)
        candidates = engine.translit_word(payload['text'], topk=5)['or']
        result = dict(text=candidates[0] if candidates else payload['text'], candidates=candidates)
    elif task == 'transliterate' and provider == 'aksharamukha':
        from aksharamukha import transliterate
        result = dict(text=transliterate.process('ITRANS', 'Oriya', payload['text']))
    else:
        raise ValueError(f'Unsupported built-in provider: {task}/{provider}; configure an external command for extensions')
    Path(result_path).write_text(json.dumps(result, ensure_ascii=False), encoding='utf-8')


if __name__ == '__main__':
    main(*sys.argv[1:])
