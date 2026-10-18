from betun import providers


def test_odia_typing_without_model_configuration(tmp_path, monkeypatch):
    monkeypatch.setattr(providers, 'configuration', lambda: {})
    result = providers.invoke('transliterate', {'text':'namaskaara'}, tmp_path)
    assert result['text'] == 'ନମସ୍କାର'
    assert result['method'] == 'phonetic-itrans'


def test_odia_phrase_keeps_punctuation(tmp_path, monkeypatch):
    monkeypatch.setattr(providers, 'configuration', lambda: {})
    result = providers.invoke('transliterate', {'text':'mu bhala achhi!'}, tmp_path)
    assert result['text'] == 'ମୁ ଭଲ ଅଛି!'
