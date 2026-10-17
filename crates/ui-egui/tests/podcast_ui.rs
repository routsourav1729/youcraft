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
