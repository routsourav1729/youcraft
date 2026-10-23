# YouCraft CPAL compatibility patch

CPAL 0.16.0, Apache-2.0, vendored from the crates.io package already used by Filmcraft. Only `src/host/wasapi/stream.rs` is modified:

- Prime a cold shared render buffer with silence before `IAudioClient::Start`, preserving queued data on resume.
- Pass `GetCurrentThread()` to `SetThreadPriority`. The original passed a numeric thread ID as a handle, so promotion could fail silently.

References: https://learn.microsoft.com/en-us/windows/win32/api/audioclient/nf-audioclient-iaudioclient-start and https://github.com/RustAudio/cpal/blob/master/src/host/wasapi/stream.rs . No new audio engine or codec. Target laptop audibility still needs verification.
