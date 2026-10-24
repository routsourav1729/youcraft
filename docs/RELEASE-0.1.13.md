YouCraft 0.1.13 fixes the first part of reaction calibration and review playback.

- Opening a saved episode on Reactions loads its edited review automatically.
- New reaction analysis stores actual reference-look distance. The overlay uses
  the same units as the eye/gaze tolerance, with head pitch shown separately.
  Older analysis without this measurement is labelled rather than inventing it.
- Smile is labelled as a 0–1 expression signal, not a confidence percentage.
  Mark at least three neutral and three modest-smile frames from the same camera
  angle to calculate a threshold, or adjust it directly.
- Apply suggestions reuses cached candidate signals when changing smile threshold,
  interval or spacing. A 0.22 signal is no longer permanently excluded by the old
  fixed 0.35 cutoff. Older caches use available sampled smile measurements;
  new analysis records the peak from the denser detection stream.
- Kept/manual shots and rejected ranges survive reselection and new analysis.
  Analysis no longer silently resets the user's overlap buffer to two seconds.

This release does not claim to solve nod detection, every short-overlap case,
proposed screen-time accounting or adaptive GPU batching. Those remain follow-up
work. No speech/vision model reinstall is required for these application changes.
