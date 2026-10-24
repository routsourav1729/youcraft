YouCraft 0.1.13 improves podcast reaction calibration and editing.

- Confirm 2–3 listening frames per host recording (up to eight). Matching accepts
  the confirmed examples rather than requiring one exact pose. Each example must
  contain one clear face. The blink gate adapts to those confirmed examples so
  elevated side-view/glasses signals do not reject all listening frames. Eye-distance units now match the tolerance control;
  head pitch is separate, and old caches explicitly request re-analysis.
- Smile intensity is a 0–1 expression signal, not a probability. Capture neutral
  and modest-smile examples to calculate a threshold, or adjust it directly.
  Apply suggestions reuses cached signals and shows matching spans to preview.
- Optional host visibility target replaces the one-per-interval restriction.
  Current and proposed host/guest minutes and percentages are shown separately;
  split screen counts for both. Quality, available footage and spacing can leave
  the result below target. Kept shots are never removed to enforce a percentage.
- Nod proposals require a brief pitch excursion and return, stable yaw and
  listening before/after. Brief gaze excursions are allowed during the movement.
  These are heuristic suggestions to review, not guaranteed intent detection.
- Separate overlap buffers before and after speech, with an adjustable minimum
  overlap duration. A one-click preset includes every detected interruption with
  one second on each side. Locked cuts are preserved; inserts crossing expanded
  overlap are trimmed instead of covering split screen.
- Manual insert length, mark-start/end, trimming, delete, preview, Keep/Reject,
  previous/next suggestions and one-level Undo stay in the reaction workspace.
  Kept/manual edits and rejected/deleted ranges survive suggestion recalculation.
- Transcript and Reactions share the same resizable preview height. Switching
  back to Transcript restores both cameras instead of leaving the edited output.
- Copy diagnostics produces a bounded local model-run report: provider, device,
  duration, failure category and available GPU memory when reported by the worker.
  It contains no media, transcript, paths or credentials and uploads nothing.

No model reinstall is required. Existing vision analysis needs one new analysis
for actual eye-distance measurements and the revised nod detector. New look
examples require analysis; changing smile threshold/spacing/target reuses cache.
GPU adaptive batching and GPU benchmark validation remain follow-up work; this
release does not claim RTX 3050 performance has been measured. Broader visual
redesign follows the corrected workflow.
