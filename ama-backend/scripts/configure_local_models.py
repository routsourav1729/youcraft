"""Create portable CPU configuration after explicitly installing model packs."""
import json
import sys
from pathlib import Path
root=Path(__file__).resolve().parents[1]
manifest=json.loads((root/'models/manifest.json').read_text())
config={task:dict(provider=provider,python=sys.executable,device='cpu',
                  model_path=manifest[name]['path'],revision=manifest[name]['revision'])
        for task,provider,name in [('diarize','nemotron-transformers','nemotron'),('transcribe','indic-core','indic-core')]}
(root/'models/local-config.json').write_text(json.dumps(config,indent=2))
print('Local CPU model configuration saved. Model files remain outside the executable.')
