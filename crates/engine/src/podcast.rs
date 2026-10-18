//! Ama Bartalap: import an analysis plan as one undoable Filmcraft edit.
//! All subsequent editing and transcript projection use the existing engine.
use crate::commands::{CommandSpec, always, bad, has_seq};
use crate::{EngineError, Result, Session};
use filmcraft_project::{ClipId, ItemId, Transcript};
use filmcraft_time::Tick;
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::BTreeMap;

#[derive(Deserialize)]
struct Asset {
    id: String,
    path: String,
}
#[derive(Deserialize)]
struct Picture {
    asset: String,
    source_in: i64,
    speed: f64,
    side: String,
}
#[derive(Deserialize)]
struct Span {
    start: i64,
    end: i64,
    video: Vec<Picture>,
}
fn default_fps() -> f64 {
    25.0
}

#[derive(Deserialize)]
struct Plan {
    #[serde(default)]
    review: bool,
    #[serde(default = "default_fps")]
    fps: f64,
    version: u32,
    name: String,
    master: String,
    assets: Vec<Asset>,
    clips: Vec<Span>,
    transcript: Transcript,
}

fn assemble(s: &mut Session, p: &Value) -> Result<Value> {
    let plan: Plan = serde_json::from_value(p.clone()).map_err(|e| bad("podcast.assemble", e.to_string()))?;
    if !plan.fps.is_finite()
        || !(1.0..=240.0).contains(&plan.fps)
        || plan.version != 1
        || plan.assets.is_empty()
        || plan.assets.len() > 2001
        || plan.clips.is_empty()
        || plan.clips.len() > 100_000
    {
        return Err(bad("podcast.assemble", "invalid plan size/version"));
    }
    let mut end = 0;
    for span in &plan.clips {
        if span.start != end
            || span.end <= span.start
            || span.end > 254_016_000_000_i64 * 86400
            || (!plan.review && span.video.is_empty())
            || span.video.len() > if plan.review { 4 } else { 2 }
        {
            return Err(bad("podcast.assemble", "cuts must continuously cover the recording with one or two cameras"));
        }
        for v in &span.video {
            if v.source_in < 0
                || !v.speed.is_finite()
                || !(0.98..=1.03).contains(&v.speed)
                || !(["A", "B", "full"].contains(&v.side.as_str())
                    || (plan.review && ["review-bottom-A", "review-bottom-B", "review-top-A", "review-top-B", "review-top-full"].contains(&v.side.as_str())))
            {
                return Err(bad("podcast.assemble", "invalid camera timing/layout"));
            }
        }
        end = span.end;
    }
    if plan.transcript.speakers.len() > 32
        || plan.transcript.words.len() > 1_000_000
        || plan.transcript.words.iter().any(|w| w.start.0 < 0 || w.end.0 > end || w.speaker.is_some_and(|i| i >= 32))
    {
        return Err(bad("podcast.assemble", "invalid transcript bounds"));
    }
    plan.transcript.check().map_err(EngineError::Other)?;
    s.edit_group("Create podcast edit", |s| {
        let mut ids = BTreeMap::new();
        for asset in &plan.assets {
            if ids.contains_key(&asset.id) {
                return Err(bad("podcast.assemble", "duplicate asset"));
            }
            let result = s.execute("file.import", json!({"paths":[asset.path]}))?;
            let id = result
                .get("items")
                .and_then(Value::as_array)
                .and_then(|a| a.first())
                .and_then(Value::as_u64)
                .ok_or_else(|| bad("podcast.assemble", format!("Import failed: {result}")))?;
            ids.insert(asset.id.clone(), id);
        }
        let master = *ids.get(&plan.master).ok_or_else(|| bad("podcast.assemble", "master missing"))?;
        s.execute("file.newSequence", json!({"name":plan.name,"width":1920,"height":1080,"fps":plan.fps,"video":if plan.review {4}else{2},"audio":1}))?;
        if !plan.review {
            s.execute("transcript.set", json!({"item":master,"transcript":plan.transcript}))?;
        }
        for span in &plan.clips {
            let rate = s.sequence_rate();
            let start = rate.snap_nearest(Tick(span.start));
            let end = rate.snap_nearest(Tick(span.end));
            if end <= start {
                continue;
            }
            let duration = end - start;
            let audio = s.execute(
                "timeline.place",
                json!({"item":master,"track":"A1","time":start.0,"sourceIn":start.0,"duration":duration.0,"video":false,"audio":true}),
            )?;
            let mut linked = clip_ids(&audio)?;
            for (index, picture) in span.video.iter().enumerate() {
                let item = *ids.get(&picture.asset).ok_or_else(|| bad("podcast.assemble", "unknown camera asset"))?;
                let placed = s.execute(
                    "timeline.place",
                    json!({"item":item,"track":format!("V{}",index+1),"time":start.0,
                    "sourceIn":picture.source_in,"duration":duration.0,"video":true,"audio":false}),
                )?;
                let clips = clip_ids(&placed)?;
                s.edit_sequence("Camera clock correction", |q, _, _| {
                    for id in &clips {
                        if let Some((_, clip)) = q.find_item_mut(*id) {
                            clip.speed = picture.speed;
                            clip.scale_to_frame = true;
                        }
                    }
                    Ok(())
                })?;
                if picture.side.starts_with("review-") {
                    let full = picture.side == "review-top-full";
                    let x = if full {
                        960.0
                    } else if picture.side.ends_with('A') {
                        640.0
                    } else {
                        1280.0
                    };
                    let y = if picture.side.starts_with("review-bottom-") { 900.0 } else { 360.0 };
                    for id in &clips {
                        s.execute("effects.setParam", json!({"clip":id.0,"effect":"motion","param":"scale","value":if full {66.666667}else{33.333333}}))?;
                        s.execute("effects.setParam", json!({"clip":id.0,"effect":"motion","param":"position","value":[x,y]}))?;
                    }
                } else if picture.side != "full" {
                    for id in &clips {
                        s.execute("effects.setParam", json!({"clip":id.0,"effect":"motion","param":"scale","value":50.0}))?;
                        s.execute(
                            "effects.setParam",
                            json!({"clip":id.0,"effect":"motion","param":"position","value":[if picture.side=="A" {480.0}else{1440.0},540.0]}),
                        )?;
                    }
                }
                linked.extend(clips);
            }
            s.edit_sequence("Link podcast picture and dialogue", |q, ctx, _| {
                let group = ctx.alloc();
                for id in &linked {
                    if let Some((_, clip)) = q.find_item_mut(*id) {
                        clip.link = Some(group);
                    }
                }
                Ok(())
            })?;
        }
        s.state.selection.clear();
        s.set_playhead(Tick::ZERO);
        Ok(json!({"sequence":s.state.active_sequence.map(|i|i.0),"master":master,"cuts":plan.clips.len()}))
    })
}
fn clip_ids(result: &Value) -> Result<Vec<ClipId>> {
    result
        .get("clips")
        .and_then(Value::as_array)
        .ok_or_else(|| bad("podcast.assemble", "missing placed clips"))?
        .iter()
        .map(|v| v.as_u64().map(ClipId).ok_or_else(|| bad("podcast.assemble", "invalid clip id")))
        .collect()
}
fn correct(s: &mut Session, p: &Value) -> Result<Value> {
    let item = p.get("item").and_then(Value::as_u64).map(ItemId).ok_or_else(|| bad("podcast.correct", "item required"))?;
    let index = p.get("index").and_then(Value::as_u64).ok_or_else(|| bad("podcast.correct", "index required"))? as usize;
    let text = p
        .get("text")
        .and_then(Value::as_str)
        .filter(|t| !t.trim().is_empty() && t.len() < 10000)
        .ok_or_else(|| bad("podcast.correct", "nonempty text required"))?
        .to_string();
    s.edit("Correct Odia transcript", |project, _| {
        let tr = project.transcripts.get_mut(&item).ok_or_else(|| bad("podcast.correct", "no transcript"))?;
        let word = std::sync::Arc::make_mut(tr).words.get_mut(index).ok_or_else(|| bad("podcast.correct", "no such transcript unit"))?;
        word.text = text;
        word.confidence = 1.0;
        Ok(())
    })?;
    Ok(Value::Null)
}
fn srt(s: &mut Session, p: &Value) -> Result<Value> {
    let path = p.get("path").and_then(Value::as_str).ok_or_else(|| bad("podcast.exportSrt", "path required"))?;
    let words = crate::transcript::sequence_words(s);
    let rules = filmcraft_edit::transcript::CaptionRules::default();
    let blocks = filmcraft_edit::transcript::caption_blocks(&words, &rules, s.sequence_rate());
    let stamp = |t: Tick| {
        let ms = t.to_units_floor(1000).max(0);
        format!("{:02}:{:02}:{:02},{:03}", ms / 3_600_000, ms / 60_000 % 60, ms / 1000 % 60, ms % 1000)
    };
    let mut text = String::new();
    for (i, b) in blocks.iter().enumerate() {
        text.push_str(&format!("{}\n{} --> {}\n{}\n\n", i + 1, stamp(b.start), stamp(b.end), b.text));
    }
    s.services.write_file(path, text.as_bytes()).map_err(|e| EngineError::Other(e.to_string()))?;
    Ok(json!({"captions":blocks.len(),"path":path}))
}
pub fn commands() -> Vec<CommandSpec> {
    vec![
        CommandSpec {
            id: "podcast.assemble",
            label: "Create Podcast Edit",
            menu: &[],
            shortcut: None,
            params: "{version:1,name,master,assets,clips,transcript}",
            enabled: always,
            run: assemble,
            journal: true,
        },
        CommandSpec {
            id: "podcast.correct",
            label: "Correct Odia Transcript",
            menu: &[],
            shortcut: None,
            params: "{item:id,index:n,text:str}",
            enabled: always,
            run: correct,
            journal: true,
        },
        CommandSpec {
            id: "podcast.exportSrt",
            label: "Export Edited Odia SRT",
            menu: &["File", "Export"],
            shortcut: None,
            params: "{path:str}",
            enabled: has_seq,
            run: srt,
            journal: false,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    fn fixture() -> (std::path::PathBuf, Value) {
        let root = std::env::temp_dir().join(format!("ama-podcast-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir_all(&root).unwrap();
        let wav = root.join("master.wav");
        let size = 4 * 48000 * 2u32;
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(36 + size).to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16u32.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&48000u32.to_le_bytes());
        bytes.extend_from_slice(&96000u32.to_le_bytes());
        bytes.extend_from_slice(&2u16.to_le_bytes());
        bytes.extend_from_slice(&16u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&size.to_le_bytes());
        bytes.resize(44 + size as usize, 0);
        std::fs::write(&wav, bytes).unwrap();
        let png = root.join("camera.png");
        image::RgbImage::from_pixel(16, 16, image::Rgb([40, 80, 120])).save(&png).unwrap();
        let sec = 254_016_000_000_i64;
        let plan = json!({"version":1,"name":"Odia podcast","master":"master","assets":[{"id":"master","path":wav},{"id":"camera","path":png}],
            "clips":[{"start":0,"end":2*sec,"video":[{"asset":"camera","source_in":0,"speed":1.0,"side":"full"}]},
                     {"start":2*sec,"end":4*sec,"video":[{"asset":"camera","source_in":2*sec,"speed":1.0,"side":"full"}]}],
            "transcript":{"language":"or","source":"fixture","speakers":[{"name":"Host"}],"words":[
                {"text":"ନମସ୍କାର","start":sec/2,"end":sec,"speaker":0},{"text":"ଓଡ଼ିଶା","start":5*sec/2,"end":3*sec,"speaker":0}]}});
        (root, plan)
    }
    #[test]
    fn podcast_assemble_links_text_and_exports_after_ripple_delete() {
        let (root, plan) = fixture();
        let mut s = Session::default();
        let result = s.execute("podcast.assemble", plan).unwrap();
        assert_eq!(crate::transcript::sequence_words(&s).len(), 2);
        let q = s.active_sequence().unwrap();
        q.check().unwrap();
        let video = q.video_tracks[0].items[0].id;
        assert_eq!(q.video_tracks[0].items[0].link, q.audio_tracks[0].items[0].link);
        s.execute("timeline.select", json!({"clips":[video.0]})).unwrap();
        s.execute("edit.rippleDelete", json!({})).unwrap();
        let words = crate::transcript::sequence_words(&s);
        assert_eq!(words.len(), 1);
        assert_eq!(words[0].text, "ଓଡ଼ିଶା");
        assert_eq!(words[0].start, Tick::from_seconds_f64(0.5));
        let out = root.join("episode.or.srt");
        s.execute("podcast.exportSrt", json!({"path":out})).unwrap();
        let text = std::fs::read_to_string(out).unwrap();
        assert!(text.contains("ଓଡ଼ିଶା"));
        assert!(!text.contains("ନମସ୍କାର"));
        s.execute("edit.undo", json!({})).unwrap();
        assert_eq!(crate::transcript::sequence_words(&s).len(), 2);
        s.execute("podcast.correct", json!({"item":result["master"],"index":0,"text":"ସ୍ୱାଗତ"})).unwrap();
        assert_eq!(crate::transcript::sequence_words(&s)[0].text, "ସ୍ୱାଗତ");
        s.execute("edit.undo", json!({})).unwrap();
        s.execute("edit.undo", json!({})).unwrap();
        assert!(s.project.transcripts.is_empty());
        let _ = std::fs::remove_dir_all(root);
    }
    #[test]
    fn podcast_failed_import_rolls_back_and_rejects_hostile_times() {
        let (root, mut plan) = fixture();
        let mut s = Session::default();
        let before = s.project.clone();
        plan["clips"][0]["video"][0]["asset"] = json!("missing");
        assert!(s.execute("podcast.assemble", plan.clone()).is_err());
        assert_eq!(*s.project, *before);
        plan["clips"][0]["end"] = json!(i64::MAX);
        assert!(s.execute("podcast.assemble", plan).is_err());
        let _ = std::fs::remove_dir_all(root);
    }
}
