import importlib.util
import json
from pathlib import Path
import pytest

script = Path(__file__).resolve().parents[1] / 'scripts/configure_local_models.py'
# Source-distribution CI keeps scripts beside the backend.
spec = importlib.util.spec_from_file_location('configure_models', script)
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)

def test_model_config_uses_installed_paths_and_memory_limits(tmp_path):
    manifest = {}
    for name in ('nemotron', 'indic-core'):
        path = tmp_path / name
        path.mkdir()
        manifest[name] = {'path':str(path), 'revision':'pinned-test'}
    (tmp_path/'manifest.json').write_text(json.dumps(manifest))
    config = module.configuration(tmp_path, 'auto', 'C:/Users/Test/speech/Scripts/python.exe')
    assert set(config) == {'diarize', 'transcribe'}
    for entry in config.values():
        assert entry['max_vram_gb'] == 4.5
        assert entry['cpu_threads'] == 4
        assert entry['allow_cpu_fallback']
        assert entry['device'] == 'auto'
        assert Path(entry['model_path']).is_dir()
    (tmp_path/'nemotron').rmdir()
    with pytest.raises(ValueError, match='Missing model'):
        module.configuration(tmp_path, 'cpu', 'python.exe')


def test_setup_preserves_optional_packs_and_previous_config_on_failure(tmp_path, monkeypatch):
    manifest = {}
    for name in ('nemotron', 'indic-core'):
        path = tmp_path / name
        path.mkdir()
        manifest[name] = {'path':str(path), 'revision':'pinned-test'}
    (tmp_path/'manifest.json').write_text(json.dumps(manifest))
    output = tmp_path/'models.json'
    output.write_text(json.dumps({'reactions':{'provider':'existing-vision'}}))
    monkeypatch.setattr('sys.argv', ['setup', '--directory', str(tmp_path), '--output', str(output)])
    module.main()
    saved = output.read_bytes()
    assert json.loads(saved)['reactions']['provider'] == 'existing-vision'
    (tmp_path/'nemotron').rmdir()
    with pytest.raises(ValueError):
        module.main()
    assert output.read_bytes() == saved
