# YouCraft 0.1.17

- Speaker auditions stay in the Speakers workspace instead of opening and
  focusing the native Source panel. Each card has Play/Stop and playback progress.
- Leaving Speakers stops an audition. Samples completing after navigation do not
  start playback in another stage. Playback failures appear in the guided UI.
- Voice samples use 48 kHz stereo PCM for the native audio path.
- Transcript stays unavailable until host assignment and camera synchronization
  are confirmed, preventing an early transition into a camera-less preview.

Validation: backend workflow tests, incremental Windows build, real speaker
samples through the native UI, and user confirmation that speech is audible.
