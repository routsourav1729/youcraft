# YouCraft

A Windows podcast editor built on [FilmCraft](https://github.com/storytold/filmcraft),
with a native Rust interface and a Python helper running on the same computer.
The final app is local: no remote server is required. Choose CPU for testing
without an NVIDIA GPU, or Auto/GPU on the Lenovo LOQ. This is an
independent customized build based on FilmCraft by the ArtCraft team.

**Development build. Windows execution, the RTX 3050 6 GB target, and the optional
vision/transliteration model runtimes still need acceptance testing.**

## The workflow

Master audio → identify speakers → audition solo samples → lock host/guest →
attach ordered camera shards → review audio synchronization → correct Odia
transcript → review camera and host-reaction suggestions → edit in FilmCraft →
export video and edited Odia SRT.

| Reused from FilmCraft | Added for YouCraft |
| --- | --- |
| Native timeline, source/program monitors, linked edits and undo | Docked Podcast workspace and guided analysis stages |
| Effects, color tools, audio DSP and mixing | Master-audio sync and ordered camera shards |
| Source transcript projection through timeline edits | Indic Core text correction, Romanized Odia spelling adapter |
| Project format, media decoding and export | Speaker camera assembly and reaction review |
| Local command/control channel and MCP bridge | Server proxies and native snapshot rendering against originals |

The edit stays in FilmCraft. Removing linked conversation clips also removes the
corresponding sequence transcript; edited SRT is derived from the current sequence.
Word positions inherited from segment-only ASR are **estimated**, not forced aligned.
Camera-only edits preserve dialogue when the user unlinks the camera.

Analysis runs sequentially in disposable processes. Closing a worker releases its
model allocations before the next model starts. Model packs are separate downloads;
configuration does not establish installation, quality or GPU fit.

The [development validation record](docs/ama-validation.md) includes real Nemotron
and Indic Core runs under a 4.5 GiB allocation cap on an L40S. Both completed;
RTX 3050 speed, Windows CUDA and full-episode memory still need laptop testing.

For optional remote development testing, use an SSH tunnel to the service on localhost:8765. Browse
server recordings and cache small proxies on Windows. Original files stay on the
server. Final server export uses this fork's matching CLI and the edited project.

## Local setup and AI control

Run **YouCraft model setup** from Start to install speech models for CPU or NVIDIA.
Weights and Torch are separate from the installer. See [model setup](ama-backend/docs/models.md).
Run **YouCraft connect AI** to register your existing Codex or Claude Code client,
then launch **YouCraft with AI**. The same MCP bridge now controls podcast imports,
speaker roles, synchronization, correction and assembly as well as native editing.
See [AI workflow](packaging/ama/AI-WORKFLOW.md). No embedded chat panel is included.

## Build on Windows

Run the **YouCraft Windows V1** workflow in `.github/workflows/ama-windows.yml`.
It builds the Rust editor and CLI, runs regression tests, and packages one
Windows installer (`YouCraft-Setup-x64.exe`). When `ama-backend/` is included, it packages the Python service too.
The source staging script in the parent Betun workspace creates that combined tree.

Manual build: install the Rust 1.95 toolchain and Visual Studio C++ build tools,
then run `cargo build --locked --release -p filmcraft -p filmcraft-cli`.
The executable is `target/release/filmcraft.exe` (packaged as `YouCraft.exe`).
See `packaging/ama/READ-ME.txt` for launch, server and MCP instructions.

Existing advanced FilmCraft tools remain available through its workspaces.
Upstream documentation is retained under `docs/`; upstream capabilities and known
limitations are described in [FilmCraft's README](https://github.com/storytold/filmcraft#readme)
and [gaps](docs/gaps.md). This fork does not claim those capabilities have all been
validated on the target laptop.

## Later versions

Reviewed translation, dubbing/TTS and optional lip-sync; SoundCraft integration
where useful; PhotoCraft thumbnail handoff; YouTube publishing and analytics.
The inherited local MCP bridge can already expose editing commands to an MCP client.
Using that bridge does not make ChatGPT or Claude inference offline.
See [assistant integration](docs/ama-assistant.md) for the existing bridge and planned Ask panel.

## License and credits

Based on FilmCraft by the ArtCraft team and its contributors. The original
MIT OR Apache-2.0 licenses, NOTICE, contributor credits and asset attributions are
retained. ArtCraft trademark artwork is not used in the customized app or its
source release bundle. YouCraft uses original text branding for now.
