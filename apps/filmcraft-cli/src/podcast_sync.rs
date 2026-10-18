//! Small process boundary around Filmcraft's existing audio synchronizer.
//! PCM is decoded by the media helper; no alternative correlation implementation.
use serde_json::{Value, json};
use std::io::{Read, Seek, SeekFrom};

fn samples(request: &Value, key: &str) -> Result<Vec<f32>, String> {
    let part = &request[key];
    let path = part["path"].as_str().ok_or("missing PCM path")?;
    let start = part["start"].as_u64().ok_or("missing sample start")?;
    let count = part["count"].as_u64().ok_or("missing sample count")?;
    if count == 0 || count > 16_000_000 || start > u64::MAX / 4 {
        return Err("invalid PCM window size".into());
    }
    let mut file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    file.seek(SeekFrom::Start(start * 4)).map_err(|e| e.to_string())?;
    let mut bytes = vec![0; count as usize * 4];
    file.read_exact(&mut bytes).map_err(|e| e.to_string())?;
    let data: Vec<_> = bytes.chunks_exact(4).map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])).collect();
    if data.iter().any(|v| !v.is_finite()) {
        return Err("non-finite PCM samples".into());
    }
    Ok(data)
}

pub fn run(path: &str) -> Result<Value, String> {
    let request: Value = serde_json::from_slice(&std::fs::read(path).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    let sr = request["sample_rate"].as_u64().filter(|r| (1000..=48000).contains(r)).ok_or("invalid sample rate")? as u32;
    let a = samples(&request, "master")?;
    let b = samples(&request, "camera")?;
    let Some(result) = filmcraft_audio_dsp::sync::find_offset(&a, &b, sr, &Default::default()) else {
        return Ok(json!({"lag_seconds":0.0,"confidence":0.0,"distinct":0.0,"reliable":false,"engine":"filmcraft-gcc-phat"}));
    };
    Ok(json!({"lag_seconds":result.lag_fine / f64::from(sr),"confidence":result.confidence,
        "distinct":result.distinct,"reliable":result.reliable(),"engine":"filmcraft-gcc-phat"}))
}
