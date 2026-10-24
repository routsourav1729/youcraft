# Guided podcast workflow

Keep Rust/egui, native video playback and the shared engine/control/MCP commands.
UI UX Pro Max can inform spacing, hierarchy and states; it does not supply native
egui widgets. shadcn/ui is a web component system, so adding it would require a
frontend migration. That migration is not necessary for these workflow repairs.

## Page responsibilities

| Page | Primary work | Secondary controls | Exclude from this step |
| --- | --- | --- | --- |
| Projects | New project, Open project, Open saved edit | Default folder, import/export metadata, rename | Model details, native timeline commands |
| Audio | Choose master audio, identify speakers | Device selection and processing status | Camera edits, exports, native Lift/Extract |
| Speakers | Audition speakers, assign host/guest, lock roles | Correct activity intervals | Reaction thresholds and export controls |
| Cameras | Attach host/guest recordings, sync, review each recording | Manual offset/rate adjustments, source replacement | Native sequence Insert/Overwrite |
| Transcript | Watch both cameras, correct text, approve segments | Search, speaker filter, Odia spelling | Reaction-edited output, native Lift/Extract |
| Reactions | Watch proposed output, Keep/Remove, manual host shots | Right-side timing/sensitivity; full-width reference picker | Instruction paragraphs, unrelated native editing commands |
| Handoff | Resolve outstanding reviews, open saved native edit | Subtitle/video export | Silent regeneration of native timeline edits |

## Implemented in 0.1.14

- Shared global navigation, compact Settings menu and visible installed version.
- Playback-only transport in guided monitors; complete transport in the editor.
- Reaction review/inspector layout, proposal output toggle, clickable markers and
  player-adjacent decisions. Split-screen buffers live with selection controls.
- Readable project folders and selectable root for future projects. Existing
  projects retain their locations. JSON metadata stays alongside project-generated
  files; original media is referenced rather than copied without a user decision.
- First native handoff saves in the project folder; home offers Open edit for a
  recorded native project. Native edits must save successfully before switching.
- Metadata exchange is available in Project files and through the control API.

## Added in 0.1.15

Audio now separates recording and processing; Speakers uses paired role cards;
Cameras uses host/guest columns with per-recording sync and framing controls.
The grid stacks at narrow widths. Camera preview height is independent from the
larger Transcript/Reactions view. Input-dependent actions are disabled until
usable. Native editor branding consistently identifies YouCraft.

## Remaining refinement

Transcript correction tools and full editor discoverability still need further
refinement. Automatic framing is a stable camera crop for seated recordings, not
continuous subject tracking or a guarantee that hands remain visible. Always
allow Fit full image and manual correction. Do not call this a complete design
system or a frontend migration.

## Verification rules

- Check the actual native build at laptop width and desktop width; check narrow
  docked panels after handoff as well as full-page guided screens.
- Avoid overlapping controls and horizontally hidden primary actions. Use
  independent scrolling for review content and inspector controls.
- Preview and accepted edit are separate states. Pending shots never become
  accepted merely because the user plays them. Screen-time totals identify which
  state they represent. Split-screen contributes to both people's visible time.
- Every action needs an automation ID; model/file work stays off the paint thread.
  Refresh suggestions reuses cached analysis and must not reload models.
- Error messages name the failed action and a recovery action. Empty results
  distinguish no matches from an already reached visibility target.

References: https://github.com/nextlevelbuilder/ui-ux-pro-max-skill and
https://ui.shadcn.com/docs/installation . These are design/tool references, not
runtime dependencies or instructions to install global npm packages.
