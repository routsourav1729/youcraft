# Ama Bartalap development validation

These checks support development handoff, not a production or hardware certification.
Windows execution and the Lenovo LOQ RTX 3050 6 GB remain acceptance-test targets.

## Model memory check — 17 October 2026

Real audio inference on an allocated NVIDIA L40S, Torch 2.11.0+cu128. Each model
ran in its own process with a 4.5 GiB PyTorch allocation limit and four CPU threads.

| Provider | Input | Peak allocated | Peak reserved | Elapsed including load |
| --- | --- | --- | --- | --- |
| Nemotron Transformers | 10-minute recording | 0.451 GiB | 0.484 GiB | 139.0 seconds |
| Indic Core | 1-minute Odia recording | 2.340 GiB | 2.402 GiB | 76.6 seconds |

Both completed and returned predictions. Free device memory before each process
was identical at 43.981 GiB. These are PyTorch allocator figures, not total driver,
display or editor VRAM. L40S speed does not predict RTX 3050 speed; these excerpts
do not establish full-episode memory requirements or transcription/diarization accuracy.
The pinned model revisions are in `ama-backend/packaging/models-3050.example.json`.
The small reports in `validation/` contain measurements only, with no recordings or transcript.

Nemotron is the default V1 candidate; pyannote remains an optional adapter.
Gaze/smile/nod inference and IndicXlit model runtimes still need real-model acceptance
tests. Their UI, filtering and adapter code must not be mistaken for quality validation.

## Editing checks

The targeted native tests cover camera assembly, invalid-input rollback, linked
dialogue deletion, undo, transcript correction, and subtitle timing through camera
razors. The backend suite covers synchronization/edit planning, prediction cleanup,
reaction eligibility, cancellation, CPU retry, source relinking and GPU device selection.

A four-second sample from two real cameras rendered through the native FilmCraft
CLI to 1920×1080 H.264 with 48 kHz AAC, including a combined shot. This is a short
render smoke test; it is not a full-episode synchronization or export benchmark.
Original recordings and rendered samples are kept outside the source repository.

The seven-step UI has a native egui harness. Optional software-OpenGL screenshots
can be generated with `FILMCRAFT_UI_SNAPSHOT_DIR`; these do not establish Windows
playback smoothness. The Windows workflow is the source of truth for Windows
compilation and downloadable development artifacts.

## Local-first handoff

The Windows installer starts the Python helper on the same computer. CPU mode
is selectable in Audio; the remote connection is an Advanced development option.
Long instructional labels have been removed from the podcast workflow.
The source archive and prior audio/TTS experiments are development artifacts,
not additional user installations.

## CPU check — 17 October 2026

Both workers completed real one-minute audio inference with CUDA disabled, four
CPU threads, and a 16 GiB Slurm memory allocation. Each model ran in a separate
process. Peak child RSS was 1.114 GiB for Nemotron and 5.413 GiB for Indic Core.
Elapsed times including imports/loading were 289.0 and 164.9 seconds. The shared
server filesystem and server CPU differ from a laptop; these are functionality
and memory checks, not laptop speed predictions or full-episode guarantees.
The editor/Windows overhead was not part of the worker memory measurement.

## Release gates

`cargo xtask ci` completed successfully: formatting, workspace Clippy, native
workspace tests (3,499 passed, 44 ignored), dependency layers, asset attribution,
and WASM compilation (49 crates). The backend suite passed all 36 tests.
A separate Windows MSVC target compiler check passed. The native seven-step UI
harness also rendered all screens using software OpenGL after the label cleanup.
The staged source passed its asset and dependency-layer checks.

Actual Windows installation/playback and Lenovo GPU acceptance remain separate
from these Linux-hosted checks. Consult the Windows Actions run for the installer.

The UTF-8 regression test simulates a non-UTF-8 Windows default for Odia worker
results, model configuration and cached results. The Windows workflow also installs
its generated installer and checks an Odia project through the packaged helper.
The native headless MCP smoke test listed 17 tools, exposed all three podcast
commands, and created/inspected a sequence without an AI provider.

The first Windows run passed native compilation, linked transcript tests and the UI
smoke test. Its backend export test caught a removed FFmpeg option. Export now
uses the current file-option syntax with a targeted fallback for older FFmpeg.
The Python packaging checks run before the expensive native build.
