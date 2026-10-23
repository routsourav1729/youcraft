//! Small recovery panel using the same device and stream as the native transport.
use crate::{AudioDevices, FilmcraftApp};
use serde_json::json;
use std::time::{Duration, Instant};

#[derive(Default)]
pub struct State {
    pub open: bool,
    devices: Option<AudioDevices>,
    pub message: String,
    testing: Option<Instant>,
}
impl State {
    pub fn tick(&mut self, app: &mut FilmcraftApp, ctx: &egui::Context) {
        if let Some(start) = self.testing {
            if let Some(a) = app.audio.as_mut() {
                a.recover();
            }
            if app.playback.playing || app.source_playback.clock.playing {
                self.testing = None;
            } else if start.elapsed() >= Duration::from_millis(3200) {
                let frames = app.audio.as_ref().and_then(|a| a.played_frames()).unwrap_or(0);
                let detail = app.audio.as_ref().map(|a| a.diagnostics()).unwrap_or_default();
                if let Some(a) = app.audio.as_mut() {
                    a.stop();
                }
                self.testing = None;
                self.message = if frames > 0 { "Test finished. Did you hear the tone?".into() } else { format!("No audio callbacks received. {detail}") };
            } else {
                ctx.request_repaint_after(Duration::from_millis(100));
            }
        }
    }
    fn configure(&mut self, app: &mut FilmcraftApp) {
        app.stop();
        app.stop_source();
        self.testing = None;
        if let Some(a) = app.audio.as_mut() {
            a.configure(&app.session.prefs.audio_hardware, None);
            self.devices = Some(a.devices());
        }
    }
    fn test(&mut self, app: &mut FilmcraftApp) -> Result<(), String> {
        self.configure(app);
        let a = app.audio.as_mut().ok_or("Audio driver unavailable")?;
        let rate = a.sample_rate().max(1) as f64;
        let mut frame = 0u64;
        a.start(Box::new(move |buf, channels| {
            for out in buf.chunks_mut(channels.max(1)) {
                let t = frame as f64 / rate;
                let fade = (t / 0.02).min((2.0 - t) / 0.02).clamp(0., 1.);
                let x = if t < 2. { (t * 440. * std::f64::consts::TAU).sin() * 0.08 * fade } else { 0. };
                out.fill(x as f32);
                frame += 1;
            }
        }))?;
        app.playback.audio_error.clear();
        self.testing = Some(Instant::now());
        self.message = "Playing a two-second tone…".into();
        Ok(())
    }
    pub fn show(&mut self, app: &mut FilmcraftApp, ctx: &egui::Context) {
        if !self.open {
            return;
        }
        let mut open = true;
        let mut close = false;
        egui::Window::new("Audio output").open(&mut open).default_width(520.).show(ctx, |ui| {
            if self.devices.is_none() {
                self.devices = Some(app.audio.as_ref().map(|a| a.devices()).unwrap_or_default());
            }
            let devices = self.devices.clone().unwrap_or_default();
            let mut selected = app.session.prefs.audio_hardware.default_output.clone();
            let before = selected.clone();
            egui::ComboBox::from_id_salt("podcast-output-device").selected_text(if selected.is_empty() { "System default" } else { &selected }).show_ui(
                ui,
                |ui| {
                    ui.selectable_value(&mut selected, String::new(), "System default");
                    for name in &devices.outputs {
                        ui.selectable_value(&mut selected, name.clone(), name);
                    }
                },
            );
            if selected != before {
                match app.session.execute("prefs.set", json!({"key":"audioHardware.defaultOutput","value":selected})) {
                    Ok(_) => {
                        self.configure(app);
                        self.message = "Output selected. Test sound to verify.".into();
                    }
                    Err(e) => self.message = e.to_string(),
                }
            }
            ui.horizontal_wrapped(|ui| {
                if button(app, ui, "refresh", "Refresh") {
                    self.configure(app);
                }
                if button(app, ui, "default", "System default") {
                    match app.session.execute(
                        "prefs.set",
                        json!({"values":{
                        "audioHardware.deviceClass":"", "audioHardware.defaultOutput":"",
                        "audioHardware.bufferSize":512,"audioHardware.sampleRate":48000,
                        "audioHardware.forceDocumentRate":false,"audioHardware.mapLeft":0,"audioHardware.mapRight":1}}),
                    ) {
                        Ok(_) => {
                            self.configure(app);
                            self.message = "Default output restored. Test sound to verify.".into();
                        }
                        Err(e) => self.message = e.to_string(),
                    }
                }
                if button(app, ui, "test", "Test sound")
                    && let Err(e) = self.test(app)
                {
                    self.message = format!("Test failed: {e}");
                }
                if ui.add_enabled(app.session.active_sequence().is_some(), egui::Button::new("Retry playback")).clicked() {
                    self.configure(app);
                    app.play(1.);
                    self.message = "Retrying playback…".into();
                }
            });
            if !self.message.is_empty() {
                ui.label(&self.message);
            }
            let hw = &app.session.prefs.audio_hardware;
            let device = if hw.default_output.is_empty() { "System default" } else { &hw.default_output };
            let detail = app.audio.as_ref().map(|a| a.diagnostics()).unwrap_or_default();
            let rate = app.audio.as_ref().map(|a| a.sample_rate()).unwrap_or(0);
            let channels = app.audio.as_ref().map(|a| a.channels()).unwrap_or(0);
            let report = format!(
                "YouCraft 0.1.10\nOutput: {device}\nAvailable: {}\nFormat: {rate} Hz, {channels} channels\nDevice: {detail}\nPlayback: {}\nTest: {}",
                devices.outputs.join(", "),
                app.playback.audio_error,
                self.message
            );
            ui.label(format!("{rate} Hz · {channels} channels"));
            if !detail.is_empty() {
                ui.label(&detail);
            }
            if !app.playback.audio_error.is_empty() {
                ui.colored_label(egui::Color32::LIGHT_RED, &app.playback.audio_error);
            }
            if button(app, ui, "copy", "Copy diagnostics") {
                ctx.copy_text(report);
            }
            if button(app, ui, "done", "Done") {
                close = true;
            }
        });
        self.open = open && !close;
    }
}

fn button(app: &mut FilmcraftApp, ui: &mut egui::Ui, id: &str, label: &str) -> bool {
    let r = ui.button(label);
    app.auto.add_response(&format!("podcast.audio.{id}"), &r, label);
    r.clicked()
}
