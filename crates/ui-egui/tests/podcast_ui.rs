//! Exercise the real docked workflow with the existing Filmcraft app and egui harness.
use filmcraft_engine::Session;
use filmcraft_ui_egui::FilmcraftApp;
#[test]
fn podcast_workspace_renders_every_step_before_opening_native_editor() {
    let mut session = Session::default();
    session.execute("file.newSequence", serde_json::json!({"name":"UI fixture", "fps":25})).unwrap();
    let mut app = FilmcraftApp::new(session);
    app.set_workspace("Podcast");
    app.podcast.projects_open = false;
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
    let mut builder = egui_kittest::Harness::builder().with_size(egui::vec2(1366., 768.)).with_max_steps(10000);
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
        assert!(harness.state().auto.find("podcast.models").is_some());
        assert!(harness.state().auto.find("podcast.connect_ai").is_some());
        assert!(harness.state().auto.find("program.scrubBar").is_none(), "editor hidden until requested");
        if let Some(dir) = &screenshots {
            std::fs::create_dir_all(dir).unwrap();
            harness.render().expect("render real Podcast UI").save(dir.join(format!("podcast-{step}.png"))).unwrap();
        }
    }
    harness.state_mut().set_workspace("Editing");
    for _ in 0..5 {
        harness.step();
    }
    assert!(harness.state().auto.find("program.scrubBar").is_some(), "native editor remains available");
}

#[test]
fn podcast_control_is_atomic_and_does_not_expose_credentials() {
    use filmcraft_ui_egui::control::{ControlRequest, Outcome, handle};
    let mut app = FilmcraftApp::new(Session::default());
    app.podcast.token = "private-token".into();
    app.podcast.setup.hf_token = "private-hf-token".into();
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
    assert!(!reply.to_string().contains("private-hf-token"));
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
    fn frame(harness: &mut egui_kittest::Harness<'_, FilmcraftApp>) {
        let ctx = harness.ctx.clone();
        let mut input = std::mem::take(harness.input_mut());
        eframe::App::raw_input_hook(harness.state_mut(), &ctx, &mut input);
        *harness.input_mut() = input;
        harness.step();
    }
    macro_rules! wait {
        () => {{
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(120);
            loop {
                frame(&mut harness);
                let state = call(harness.state_mut(), &ctx, "podcast.inspect", json!({}));
                if state["busy"] == false {
                    assert_eq!(state["error"], "", "{state}");
                    // Let the spinner disappear before resolving the next button's position.
                    for _ in 0..4 {
                        frame(&mut harness);
                    }
                    break;
                }
                assert!(std::time::Instant::now() < deadline, "job timeout: {state}");
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
        }};
    }
    call(harness.state_mut(), &ctx, "podcast.configure", json!({"name":"MCP workflow smoke","master":format!("{media}/master.wav"),"compute_device":"cpu"}));
    let guided = std::env::var_os("YOUCRAFT_SMOKE_AUTO_IDENTIFY").is_some();
    if guided {
        harness.state_mut().set_workspace("Podcast");
        harness.state_mut().podcast.setup.check_on_start = true;
        frame(&mut harness);
        wait!();
        assert_eq!(harness.state().podcast.step, 0, "missing transcript model opens setup");
        assert!(harness.state().auto.find("podcast.setup_cpu").is_some());
        assert!(harness.state().auto.find("podcast.connect_ai").is_some());
        let (req, _) = ControlRequest::new("ui.click", json!({"id":"podcast.continue_audio"}));
        let _ = handle(harness.state_mut(), &ctx, &req);
        for _ in 0..5 {
            frame(&mut harness);
        }
        assert_eq!(harness.state().podcast.step, 1);
        let picked = format!("{media}/master.wav");
        harness.state_mut().podcast.master.clear();
        harness.state_mut().hooks.pick_files = Some(Box::new(move |_| vec![picked.clone()]));
        for _ in 0..5 {
            frame(&mut harness);
        }
        let (req, _) = ControlRequest::new("ui.click", json!({"id":"podcast.files"}));
        let _ = handle(harness.state_mut(), &ctx, &req);
        for _ in 0..5 {
            frame(&mut harness);
        }
        assert!(harness.state().podcast.master.ends_with("master.wav"));
        let (req, _) = ControlRequest::new("ui.click", json!({"id":"podcast.create"}));
        let _ = handle(harness.state_mut(), &ctx, &req);
        for _ in 0..5 {
            frame(&mut harness);
        }
    } else {
        call(harness.state_mut(), &ctx, "podcast.action", json!({"action":"create","options":{"identify":false}}));
    }
    wait!();
    if guided {
        assert_eq!(harness.state().podcast.step, 2);
        assert_eq!(harness.state().podcast.samples.len(), 2);
        assert_eq!(harness.state().podcast.project["activity"].as_array().unwrap().len(), 2);
    }
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

#[test]
fn imported_episode_keeps_compute_controls_available_for_recovery() {
    use filmcraft_ui_egui::control::{ControlRequest, handle};
    let mut app = FilmcraftApp::new(Session::default());
    app.set_workspace("Podcast");
    app.podcast.projects_open = false;
    app.podcast.step = 1;
    app.podcast.compute_device = "cuda".into();
    app.podcast.project_id = "device-fixture".into();
    app.podcast.project = serde_json::json!({
        "id":"device-fixture", "name":"GPU recovery", "revision":1,
        "settings":{"compute_device":"cuda"},
        "clips":[{"id":"master","camera":"master","path":"episode.wav","duration":60}]
    });
    let screenshots = std::env::var_os("FILMCRAFT_UI_SNAPSHOT_DIR").map(std::path::PathBuf::from);
    let mut builder = egui_kittest::Harness::builder().with_size(egui::vec2(1366., 768.)).with_max_steps(10000);
    if screenshots.is_some() {
        builder = builder.wgpu();
    }
    let mut harness = builder.build_eframe(move |_| app);
    for _ in 0..5 {
        harness.step();
    }
    for device in ["auto", "cpu", "cuda"] {
        assert!(harness.state().auto.find(&format!("podcast.device.{device}")).is_some());
    }
    let ctx = harness.ctx.clone();
    let (req, _) = ControlRequest::new("ui.click", serde_json::json!({"id":"podcast.device.cpu"}));
    let _ = handle(harness.state_mut(), &ctx, &req);
    for _ in 0..5 {
        let mut input = std::mem::take(harness.input_mut());
        eframe::App::raw_input_hook(harness.state_mut(), &ctx, &mut input);
        *harness.input_mut() = input;
        harness.step();
    }
    assert_eq!(harness.state().podcast.compute_device, "cpu");
    assert_eq!(harness.state().podcast.project["revision"], 1);
    assert_eq!(harness.state().podcast.project["clips"][0]["path"], "episode.wav");
    if let Some(dir) = screenshots {
        std::fs::create_dir_all(&dir).unwrap();
        harness.render().expect("render recovery controls").save(dir.join("podcast-device-recovery.png")).unwrap();
    }
}

fn saved_episode_app() -> FilmcraftApp {
    let mut app = FilmcraftApp::new(Session::default());
    app.set_workspace("Podcast");
    app.podcast.projects_open = false;
    app.podcast.project_id = "workflow-fixture".into();
    app.podcast.step = 3;
    app.podcast.project = serde_json::json!({
        "id":"workflow-fixture", "name":"Saved conversation", "revision":7, "duration":60,
        "settings":{"compute_device":"cuda","roles_locked":true,"host_speaker":"host"},
        "clips":[{"id":"master","camera":"master","path":"master.wav","duration":60},
                 {"id":"host-camera","camera":"A","path":"host.mp4","duration":60,"sync_reviewed":true}],
        "activity":[{"id":"turn","start":0,"end":60,"speaker":"host"}],
        "captions":(0..6).map(|i| serde_json::json!({"id":format!("caption-{i}"),"start":i*10,"end":(i+1)*10,"speaker":"host","text":format!("ନମସ୍କାର segment {i}"),"reviewed":true})).collect::<Vec<_>>(),
        "shots":[{"id":"cut","start":0,"end":60,"layout":"A","locked":true}],
        "reactions":[], "cuts":[]
    });
    app
}

fn click_workflow(harness: &mut egui_kittest::Harness<'_, FilmcraftApp>, id: &str) {
    use filmcraft_ui_egui::control::{ControlRequest, handle};
    let ctx = harness.ctx.clone();
    let (req, _) = ControlRequest::new("ui.click", serde_json::json!({"id":id}));
    let _ = handle(harness.state_mut(), &ctx, &req);
    for _ in 0..5 {
        let mut input = std::mem::take(harness.input_mut());
        eframe::App::raw_input_hook(harness.state_mut(), &ctx, &mut input);
        *harness.input_mut() = input;
        harness.step();
    }
}

#[test]
fn navigation_and_continue_preserve_saved_results_without_starting_jobs() {
    let app = saved_episode_app();
    let original = app.podcast.project.clone();
    let mut harness = egui_kittest::Harness::builder().with_size(egui::vec2(1366., 900.)).build_eframe(move |_| app);
    for _ in 0..5 {
        harness.step();
    }
    click_workflow(&mut harness, "podcast.transcribe");
    assert_eq!(harness.state().podcast.step, 4);
    for step in [2, 3, 4, 5, 6, 4] {
        click_workflow(&mut harness, &format!("podcast.step.{step}"));
        assert_eq!(harness.state().podcast.step, step);
        assert_eq!(harness.state().podcast.project, original);
        assert!(harness.state().podcast.job.is_none());
        let inspect = filmcraft_ui_egui::podcast::control(harness.state_mut(), "podcast.inspect", &serde_json::json!({})).unwrap();
        assert_eq!(inspect["busy"], false, "Navigation launched a request");
    }
}

#[test]
fn compact_transcript_edits_one_segment_and_requests_regeneration_explicitly() {
    let mut app = saved_episode_app();
    app.podcast.step = 4;
    let original = app.podcast.project.clone();
    let screenshots = std::env::var_os("FILMCRAFT_UI_SNAPSHOT_DIR").map(std::path::PathBuf::from);
    let mut builder = egui_kittest::Harness::builder().with_size(egui::vec2(1366., 768.)).with_max_steps(10000);
    if screenshots.is_some() {
        builder = builder.wgpu();
    }
    let mut harness = builder.build_eframe(move |_| app);
    for _ in 0..5 {
        harness.step();
    }
    assert!(harness.state().auto.find("podcast.caption.text.0").is_some());
    assert!(harness.state().auto.find("podcast.caption.text.1").is_none());
    click_workflow(&mut harness, "podcast.captions.next");
    assert!(harness.state().auto.find("podcast.caption.text.0").is_none());
    assert!(harness.state().auto.find("podcast.caption.text.1").is_some());
    click_workflow(&mut harness, "podcast.caption.roman.1");
    assert!(harness.state().auto.find("podcast.roman").is_some());
    if let Some(dir) = &screenshots {
        std::fs::create_dir_all(dir).unwrap();
        harness.render().unwrap().save(dir.join("compact-transcript.png")).unwrap();
    }
    filmcraft_ui_egui::podcast::control(harness.state_mut(), "podcast.action", &serde_json::json!({"action":"transcribe"})).unwrap();
    let inspect = filmcraft_ui_egui::podcast::control(harness.state_mut(), "podcast.inspect", &serde_json::json!({})).unwrap();
    assert_eq!(inspect["regeneration_pending"], "transcribe");
    assert_eq!(inspect["busy"], false);
    filmcraft_ui_egui::podcast::control(harness.state_mut(), "podcast.action", &serde_json::json!({"action":"keep_results"})).unwrap();
    assert_eq!(harness.state().podcast.project, original);
}

#[test]
fn recovery_drafts_are_separate_per_project_and_keep_outputs_and_view() {
    let root = std::env::temp_dir().join(format!("youcraft-draft-test-{}", std::process::id()));
    let mut app = saved_episode_app();
    app.podcast.cache = root.to_string_lossy().into();
    app.podcast.step = 6;
    for id in ["11111111111111111111111111111111", "22222222222222222222222222222222"] {
        app.podcast.project_id = id.into();
        app.podcast.project["id"] = serde_json::json!(id);
        app.podcast.checkpoint_on_exit();
    }
    for id in ["11111111111111111111111111111111", "22222222222222222222222222222222"] {
        let saved: serde_json::Value = serde_json::from_slice(&std::fs::read(root.join(format!("unsaved-{id}.json"))).unwrap()).unwrap();
        assert_eq!(saved["project_id"], id);
        assert_eq!(saved["step"], 6);
        assert_eq!(saved["project"]["captions"].as_array().unwrap().len(), 6);
        assert_eq!(saved["project"]["shots"][0]["locked"], true);
        assert!(saved.get("token").is_none());
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn startup_lands_on_clean_projects_screen() {
    let mut app = FilmcraftApp::new(Session::default());
    app.set_workspace("Podcast");
    let screenshots = std::env::var_os("FILMCRAFT_UI_SNAPSHOT_DIR").map(std::path::PathBuf::from);
    let mut builder = egui_kittest::Harness::builder().with_size(egui::vec2(1366., 768.)).with_max_steps(10000);
    if screenshots.is_some() {
        builder = builder.wgpu();
    }
    let mut harness = builder.build_eframe(move |_| app);
    harness.step();
    std::thread::sleep(std::time::Duration::from_millis(700));
    for _ in 0..5 {
        harness.step();
    }
    assert!(harness.state().auto.find("podcast.new_project").is_some());
    assert!(harness.state().auto.find("podcast.connect_ai").is_none());
    if let Some(dir) = screenshots {
        std::fs::create_dir_all(&dir).unwrap();
        harness.render().unwrap().save(dir.join("projects-home.png")).unwrap();
    }
}
