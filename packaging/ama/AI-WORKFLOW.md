# YouCraft local editing

Open YouCraft and choose **Connect AI**. **Claude Desktop** opens the included
`YouCraft.mcpb` extension. Approve its installation in Claude. If Windows does not
open it, use Claude Settings > Extensions > Advanced settings > Install Extension,
and select `YouCraft.mcpb` in the YouCraft installation folder. The app's
**Connection help > Extension folder** opens that folder.

In a Claude chat, use **+ > Connectors > YouCraft** to enable the tools. Restart
Claude completely if the new extension is not listed. YouCraft is a private local
extension; searching the public connector directory will not find it.

**Check tools** verifies MCP initialization, tool discovery and `podcast.inspect`
against the running YouCraft app. It does not claim that Claude enabled the tools
in your current chat. **Copy first prompt** supplies the exact tool call.

Claude Code and Codex buttons register the command with those installed clients.
Sign in through the client. AI setup works independently while model setup runs.
Keep YouCraft open while using its tools. No server URL is needed for this local
extension.

MCP lets the AI call YouCraft's editing tools. It is not an AI model. The client
handles login and usage; YouCraft does not collect those account credentials.
AI reasoning is online. Media processing stays on your computer; requested tool
results, transcript text and screenshots are visible to the AI service.

Browser chat cannot use this stdio connection directly. A browser connector or
secure tunnel is separate work and is not included in this preview. Codex supports
sign-in with your ChatGPT account. There is no embedded subscription-login chat.

## Tools for the AI

Use existing MCP `ui_control` with `method` and `params`. All podcast requests
operate on the same native workspace. Do not use OS file dialogs for imports.
Call `podcast.inspect` first. It returns current project, job, error, samples and
supported actions, without the helper authentication token.

1. `podcast.configure` accepts `name`, `master` (absolute local filename),
   `camera_a`, `camera_b` (ordered filename arrays), `compute_device`
   (`auto`, `cpu`, `cuda`) and `project_id`. To load an existing project, set
   project_id then call refresh before another operation.
2. `podcast.action` with `{"action":"create"}` creates and imports master audio.
   Speaker identification follows automatically; poll inspect until `busy` is false; always check `error`.
   A queued response is not job completion. Stop on an error, explain it.
3. Action `samples` gets long speaker examples. Action `sample` with
   `options:{start,end}` plays a sample in the existing Source monitor.
   Ask the user which speaker is host. Do not infer identity from faces/voice.
4. Action `lock_roles` with `host` set to the chosen speaker ID locks host→A,
   other speakers→B. Configure the camera arrays accordingly, preserving order.
   Action `attach_cameras`, then `sync`. Review low-confidence offsets with user.
5. Action `transcribe`. `podcast.review` replaces only the supplied project fields
   (`settings`, `clips`, `activity`, `captions`, `reactions`, `shots`). Fetch current
   values first, retain IDs and unrelated rows. The backend validates and saves.
   Use this to correct speaker rows, words, sync review and keep/reject choices.
   Settings is a complete field replacement: retain unrelated settings.
6. Action `shots` creates the basic edit. Optional `reactions` takes
   `options:{references:{clip_id:source_seconds},tolerance:10,interval:60,spacing:15}`.
   Requires installed vision models. Never pretend a missing pack ran successfully.
   Action `reference_frame` uses `options:{clip_id,time}`; `reaction_preview` uses
   `options:{reaction_id}`. User keeps/rejects suggestions through review.
7. Action `open_edit` assembles local originals into the native timeline. Use the
   existing `command_list` / `command_run`, `project_inspect`, `ui_elements` and
   screenshot tools to discover current editing commands and required arguments.
   Save a .fcproj before edits. Native linked cuts move the transcript as well.
   `podcast.correct` corrects native transcript; `podcast.exportSrt` exports the
   current edited captions. Inspect command schemas instead of guessing them.
   Do not reassemble an already manually edited sequence without user instruction.
8. Existing native export and podcast export UI remain available. Use the editor
   sequence as source of truth after assembly, not stale backend shot timings.

While a job runs, use inspect or action cancel. UI remains usable and undo is
available for native editing commands. Backend project review uses revision checks;
inspect/refetch after a conflict. A failed import retains the selected camera files.

Official client setup references:
- https://learn.chatgpt.com/docs/extend/mcp?surface=cli
- https://code.claude.com/docs/en/mcp
