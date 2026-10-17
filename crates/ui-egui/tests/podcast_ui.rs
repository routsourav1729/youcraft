//! Exercise the real docked workflow with the existing Filmcraft app and egui harness.
use filmcraft_engine::Session;
use filmcraft_ui_egui::FilmcraftApp;
#[test]
fn podcast_workspace_renders_every_step_and_keeps_native_panels() {
    let mut session = Session::default();
    session.execute("file.newSequence", serde_json::json!({"name":"UI fixture", "fps":25})).unwrap();
    let mut app = FilmcraftApp::new(session);
    app.set_workspace("Podcast");
    // Synthetic analysis state exercises the controls that are absent in an empty project.
    app.podcast.project_id = "ui-fixture".into();
    app.podcast.project = serde_json::json!({
        "id":"ui-fixture", "name":"Episode review", "revision":1,
        "settings":{"host_speaker":"SPEAKER_00","roles_locked":true,"speaker_camera":{"SPEAKER_00":"A","SPEAKER_01":"B"}},
        "clips":[{"id":"host","camera":"A","path":"Host / recording-01.mp4","duration":60,"offset":0,"rate":1,"sync_reviewed":true,"sync_score":0.93,"warnings":[]}],
        "activity":[{"start":0,"end":8,"speaker":"SPEAKER_00"},{"start":8,"end":60,"speaker":"SPEAKER_01"}],
        "captions":[{"id":"caption-1","start":0,"end":3,"speaker":"SPEAKER_00","text":"ନମସ୍କାର ଓଡ଼ିଶା","reviewed":false}],
        "reactions":[{"id":"reaction-1","start":20,"end":24,"clip_id":"host","score":0.75,"cues":["reference gaze","smile"],"decision":"pending"}]
    });
    app.podcast.samples = vec![serde_json::json!({"speaker":"SPEAKER_00","start":0,"end":8}), serde_json::json!({"speaker":"SPEAKER_01","start":8,"end":20})];
    let screenshots = std::env::var_os("FILMCRAFT_UI_SNAPSHOT_DIR").map(std::path::PathBuf::from);
    let mut builder = egui_kittest::Harness::builder().with_size(egui::vec2(1600., 1000.)).with_max_steps(10000);
    if screenshots.is_some() {
        builder = builder.wgpu();
    }
    let mut harness = builder.build_eframe(move |_| app);
    for step in 0..7 {
        harness.state_mut().podcast.step = step;
        for _ in 0..5 {
            harness.step();
        }
        assert_eq!(harness.state().ui.workspace, "Podcast");
        let control = ["connection_test", "master", "lock_roles", "sync", "save_review", "analyze_reactions", "export_srt"][step];
        assert!(harness.state().auto.find(&format!("podcast.{control}")).is_some(), "missing step {step} control");
        assert!(harness.state().auto.find("program.scrubBar").is_some(), "native Program monitor missing");
        if let Some(dir) = &screenshots {
            std::fs::create_dir_all(dir).unwrap();
            harness.render().expect("render real Podcast UI").save(dir.join(format!("podcast-{step}.png"))).unwrap();
        }
    }
}

#[test]
fn podcast_control_is_atomic_and_does_not_expose_credentials() {
    use filmcraft_ui_egui::control::{ControlRequest, Outcome, handle};
    let mut app = FilmcraftApp::new(Session::default());
    app.podcast.token = "private-token".into();
    let ctx = egui::Context::default();
    let (req, _) = ControlRequest::new("podcast.configure", serde_json::json!({"master":"audio.mp3","camera_a":["01.mp4","02.mp4"],"compute_device":"cpu"}));
    assert!(matches!(handle(&mut app, &ctx, &req), Outcome::Done(_)));
    assert_eq!(app.podcast.master, "audio.mp3");
    let (req, _) = ControlRequest::new("podcast.configure", serde_json::json!({"master":"changed","compute_device":"invalid"}));
    let _ = handle(&mut app, &ctx, &req);
    assert_eq!(app.podcast.master, "audio.mp3");
    assert_eq!(app.podcast.token, "private-token");
    let (req, _) = ControlRequest::new("podcast.inspect", serde_json::json!({}));
    let Outcome::Done(reply) = handle(&mut app, &ctx, &req) else { panic!("inspect deferred") };
    assert!(!reply.to_string().contains("private-token"));
    let (req, _) = ControlRequest::new("podcast.action", serde_json::json!({"action":"lock_roles","host":"missing"}));
    let _ = handle(&mut app, &ctx, &req);
    assert!(app.podcast.project.is_null());
}

/// Optional integration with the real local helper, exercised by the development smoke run.
#[test]
fn podcast_live_helper_flow() {
    use filmcraft_ui_egui::control::{ControlRequest, Outcome, handle};
    use serde_json::{Value, json};
    let Ok(endpoint) = std::env::var("YOUCRAFT_SMOKE_ENDPOINT") else { return };
    let media = std::env::var("YOUCRAFT_SMOKE_MEDIA").expect("test media directory");
    let mut app = FilmcraftApp::new(Session::default());
    app.podcast.endpoint = endpoint;
    app.podcast.token = std::env::var("YOUCRAFT_SMOKE_TOKEN").expect("test helper token");
    // Stay in another workspace to prove job polling does not depend on panel visibility.
    app.set_workspace("Editing");
    let ctx = egui::Context::default();
    let mut harness = egui_kittest::Harness::builder().with_size(egui::vec2(1200., 800.)).build_eframe(move |_| app);
    fn call(app: &mut FilmcraftApp, ctx: &egui::Context, method: &str, params: Value) -> Value {
        let (req, _) = ControlRequest::new(method, params);
        let Outcome::Done(reply) = handle(app, ctx, &req) else { panic!("deferred podcast call") };
        assert_eq!(reply["ok"], true, "{reply}");
        reply["result"].clone()
    }
    macro_rules! wait {
        () => {{
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(120);
            loop {
                harness.step();
                let state = call(harness.state_mut(), &ctx, "podcast.inspect", json!({}));
                if state["busy"] == false {
                    assert_eq!(state["error"], "", "{state}");
                    break;
                }
                assert!(std::time::Instant::now() < deadline, "job timeout: {state}");
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
        }};
    }
    call(harness.state_mut(), &ctx, "podcast.configure", json!({"name":"MCP workflow smoke","master":format!("{media}/master.wav"),"compute_device":"cpu"}));
    call(harness.state_mut(), &ctx, "podcast.action", json!({"action":"create"}));
    wait!();
    assert_eq!(harness.state().podcast.compute_device, "cpu");
    call(harness.state_mut(), &ctx, "podcast.configure", json!({"camera_a":[format!("{media}/host.mp4")],"camera_b":[format!("{media}/guest.mp4")]}));
    call(harness.state_mut(), &ctx, "podcast.action", json!({"action":"attach_cameras"}));
    wait!();
    assert!(harness.state().podcast.camera_a.is_empty());
    // Synthetic diarization avoids an ML download in this integration test.
    call(harness.state_mut(), &ctx, "podcast.review", json!({"activity":[{"start":0.,"end":2.,"speaker":"host"},{"start":2.,"end":4.,"speaker":"guest"}]}));
    wait!();
    call(harness.state_mut(), &ctx, "podcast.action", json!({"action":"lock_roles","host":"host"}));
    wait!();
    let mut clips = harness.state().podcast.project["clips"].clone();
    for clip in clips.as_array_mut().unwrap() {
        clip["sync_reviewed"] = json!(true);
    }
    call(harness.state_mut(), &ctx, "podcast.review", json!({"clips":clips}));
    wait!();
    call(harness.state_mut(), &ctx, "podcast.action", json!({"action":"shots"}));
    wait!();
    call(harness.state_mut(), &ctx, "podcast.action", json!({"action":"open_edit"}));
    wait!();
    assert!(harness.state().podcast.assembled);
    call(harness.state_mut(), &ctx, "podcast.action", json!({"action":"save"}));
    wait!();
    assert!(harness.state().session.active_sequence().is_some());
}
