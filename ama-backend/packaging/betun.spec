# Run with PyInstaller on the target OS. Never include model packs or user data.
from pathlib import Path
root = Path(SPECPATH).parent
analysis = Analysis([str(root / 'backend/betun/__main__.py')],
    pathex=[str(root / 'backend')],
    datas=[(str(root / 'backend/betun/ui'), 'betun/ui'),
           (str(root / 'backend/betun/model_worker.py'), 'betun'),
           (str(root / 'backend/betun/vision_worker.py'), 'betun')],
    hiddenimports=['betun.api', 'uvicorn.logging', 'uvicorn.loops.auto',
                   'uvicorn.protocols.http.auto', 'uvicorn.protocols.websockets.auto',
                   'uvicorn.lifespan.on'],
    excludes=['torch', 'transformers', 'nemo', 'pyannote'],
    noarchive=False)
pyz = PYZ(analysis.pure)
exe = EXE(pyz, analysis.scripts, [], exclude_binaries=True, name='BetunBackend', console=True)
collection = COLLECT(exe, analysis.binaries, analysis.datas, name='BetunBackend')
