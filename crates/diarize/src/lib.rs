use std::path::{Path, PathBuf};

use sherpa_rs::diarize::{Diarize, DiarizeConfig, Segment};
use sherpa_rs::read_audio_file;

#[derive(Debug, Clone, Copy)]
pub struct DiarizationSegment {
    pub start_seconds: f64,
    pub end_seconds: f64,
    pub speaker: usize,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("diarization failed: {0}")]
    Inference(String),
    #[error("failed to read audio: {0}")]
    Audio(String),
    #[error("model not found: {0}")]
    ModelNotFound(PathBuf),
}

pub type Result<T> = std::result::Result<T, Error>;

pub struct Diarizer {
    segmentation_model: PathBuf,
    embedding_model: PathBuf,
    config: DiarizeConfig,
}

impl Diarizer {
    pub fn new<P: AsRef<Path>>(segmentation_model: P, embedding_model: P) -> Result<Self> {
        Self::with_config(
            segmentation_model,
            embedding_model,
            DiarizeConfig::default(),
        )
    }

    pub fn with_num_speakers<P: AsRef<Path>>(
        segmentation_model: P,
        embedding_model: P,
        num_speakers: Option<i32>,
    ) -> Result<Self> {
        let config = match num_speakers {
            Some(n) => DiarizeConfig {
                num_clusters: Some(n),
                ..Default::default()
            },
            None => DiarizeConfig::default(),
        };
        Self::with_config(segmentation_model, embedding_model, config)
    }

    fn with_config<P: AsRef<Path>>(
        segmentation_model: P,
        embedding_model: P,
        config: DiarizeConfig,
    ) -> Result<Self> {
        let seg = segmentation_model.as_ref().to_path_buf();
        let emb = embedding_model.as_ref().to_path_buf();

        if !seg.exists() {
            return Err(Error::ModelNotFound(seg));
        }
        if !emb.exists() {
            return Err(Error::ModelNotFound(emb));
        }

        Ok(Self {
            segmentation_model: seg,
            embedding_model: emb,
            config,
        })
    }

    pub fn diarize_file(&self, audio_path: &str) -> Result<Vec<DiarizationSegment>> {
        let (samples, _sample_rate) =
            read_audio_file(audio_path).map_err(|e| Error::Audio(e.to_string()))?;

        self.diarize_samples(&samples)
    }

    pub fn diarize_samples(&self, samples: &[f32]) -> Result<Vec<DiarizationSegment>> {
        let mut sd = Diarize::new(
            &self.segmentation_model,
            &self.embedding_model,
            self.config.clone(),
        )
        .map_err(|e| Error::Inference(e.to_string()))?;

        let raw_segments = sd
            .compute(samples.to_vec(), None)
            .map_err(|e| Error::Inference(e.to_string()))?;

        Ok(raw_segments.into_iter().map(Into::into).collect())
    }
}

impl From<Segment> for DiarizationSegment {
    fn from(seg: Segment) -> Self {
        Self {
            start_seconds: seg.start as f64,
            end_seconds: seg.end as f64,
            speaker: seg.speaker as usize,
        }
    }
}

pub fn assign_speakers_to_words(
    words: &mut [owhisper_interface::batch::Word],
    segments: &[DiarizationSegment],
) {
    if segments.is_empty() {
        return;
    }

    for word in words.iter_mut() {
        let mid = (word.start + word.end) / 2.0;

        let best = segments
            .iter()
            .find(|seg| mid >= seg.start_seconds && mid < seg.end_seconds)
            .or_else(|| {
                segments.iter().min_by(|a, b| {
                    let da = ((a.start_seconds + a.end_seconds) / 2.0 - mid).abs();
                    let db = ((b.start_seconds + b.end_seconds) / 2.0 - mid).abs();
                    da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
                })
            });

        if let Some(seg) = best {
            word.speaker = Some(seg.speaker);
        }
    }
}

pub mod model_paths {
    use std::path::PathBuf;

    pub const SEGMENTATION_URL: &str = "https://github.com/k2-fsa/sherpa-onnx/releases/download/speaker-segmentation-models/sherpa-onnx-pyannote-segmentation-3-0.tar.bz2";
    pub const SEGMENTATION_FILE: &str = "model.int8.onnx";
    pub const SEGMENTATION_DIR: &str = "sherpa-onnx-pyannote-segmentation-3-0";

    pub const EMBEDDING_URL: &str = "https://github.com/k2-fsa/sherpa-onnx/releases/download/speaker-recongition-models/3dspeaker_speech_eres2net_base_sv_zh-cn_3dspeaker_16k.onnx";
    pub const EMBEDDING_FILE: &str = "3dspeaker_embedding.onnx";

    pub const TOTAL_SIZE_BYTES: u64 = 1_540_506 + 39_593_761;

    pub fn cache_dir() -> Option<PathBuf> {
        dirs::cache_dir().map(|d| d.join("anarlog").join("diarize-models"))
    }

    pub fn segmentation_model_path() -> Option<PathBuf> {
        cache_dir().map(|d| d.join(SEGMENTATION_DIR).join(SEGMENTATION_FILE))
    }

    pub fn embedding_model_path() -> Option<PathBuf> {
        cache_dir().map(|d| d.join(EMBEDDING_FILE))
    }

    pub fn models_exist() -> bool {
        segmentation_model_path()
            .zip(embedding_model_path())
            .is_some_and(|(s, e)| s.exists() && e.exists())
    }
}

pub mod download {
    use std::path::Path;

    use super::model_paths;
    use crate::Error;

    pub type ProgressFn = Box<dyn Fn(u8) + Send + Sync>;

    pub async fn download_models(progress: Option<ProgressFn>) -> Result<(), Error> {
        let cache =
            model_paths::cache_dir().ok_or_else(|| Error::Audio("cache dir unavailable".into()))?;
        tokio::fs::create_dir_all(&cache)
            .await
            .map_err(|e| Error::Audio(e.to_string()))?;

        download_segmentation(&cache, progress.as_deref()).await?;
        download_embedding(&cache, progress.as_deref()).await?;
        Ok(())
    }

    async fn download_segmentation(
        cache: &Path,
        progress: Option<&(dyn Fn(u8) + Send + Sync)>,
    ) -> Result<(), Error> {
        let dest_dir = cache.join(model_paths::SEGMENTATION_DIR);
        let model_path = dest_dir.join(model_paths::SEGMENTATION_FILE);
        if model_path.exists() {
            return Ok(());
        }

        if let Some(p) = progress {
            p(0);
        }

        let bytes = fetch_bytes(model_paths::SEGMENTATION_URL, |frac| {
            if let Some(p) = progress {
                p((frac * 40.0) as u8);
            }
        })
        .await?;

        let dest_dir_clone = dest_dir.clone();
        tokio::task::spawn_blocking(move || extract_segmentation(&bytes, &dest_dir_clone))
            .await
            .map_err(|e| Error::Audio(e.to_string()))??;

        if let Some(p) = progress {
            p(50);
        }
        Ok(())
    }

    async fn download_embedding(
        cache: &Path,
        progress: Option<&(dyn Fn(u8) + Send + Sync)>,
    ) -> Result<(), Error> {
        let dest = cache.join(model_paths::EMBEDDING_FILE);
        if dest.exists() {
            return Ok(());
        }

        let bytes = fetch_bytes(model_paths::EMBEDDING_URL, |frac| {
            if let Some(p) = progress {
                p(50 + (frac * 50.0) as u8);
            }
        })
        .await?;

        tokio::fs::write(&dest, &bytes)
            .await
            .map_err(|e| Error::Audio(e.to_string()))?;

        if let Some(p) = progress {
            p(100);
        }
        Ok(())
    }

    async fn fetch_bytes<F>(url: &str, mut on_progress: F) -> Result<Vec<u8>, Error>
    where
        F: FnMut(f64),
    {
        let client = reqwest::Client::builder()
            .user_agent("anarlog")
            .build()
            .map_err(|e| Error::Audio(e.to_string()))?;

        let response = client
            .get(url)
            .send()
            .await
            .map_err(|e| Error::Audio(e.to_string()))?;

        let total = response.content_length().unwrap_or(0) as f64;
        use futures_util::StreamExt;
        let mut stream = response.bytes_stream();

        let mut buf = Vec::new();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| Error::Audio(e.to_string()))?;
            buf.extend_from_slice(&chunk);
            if total > 0.0 {
                on_progress(buf.len() as f64 / total);
            }
        }
        Ok(buf)
    }

    fn extract_segmentation(bytes: &[u8], dest_dir: &Path) -> Result<(), Error> {
        std::fs::create_dir_all(dest_dir).map_err(|e| Error::Audio(e.to_string()))?;

        let cursor = std::io::Cursor::new(bytes);
        let bz = bzip2::read::BzDecoder::new(cursor);
        let mut archive = tar::Archive::new(bz);

        for entry in archive.entries().map_err(|e| Error::Audio(e.to_string()))? {
            let mut entry = entry.map_err(|e| Error::Audio(e.to_string()))?;
            let path = entry.path().map_err(|e| Error::Audio(e.to_string()))?;
            if path.file_name().and_then(|n| n.to_str()) == Some(model_paths::SEGMENTATION_FILE) {
                let dest = dest_dir.join(model_paths::SEGMENTATION_FILE);
                entry
                    .unpack(&dest)
                    .map_err(|e| Error::Audio(e.to_string()))?;
                return Ok(());
            }
        }
        Err(Error::Audio("model.int8.onnx not found in archive".into()))
    }

    pub fn delete_models() -> Result<(), Error> {
        if let Some(dir) = model_paths::cache_dir() {
            if dir.exists() {
                std::fs::remove_dir_all(&dir).map_err(|e| Error::Audio(e.to_string()))?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use owhisper_interface::batch::Word;

    fn word(text: &str, start: f64, end: f64) -> Word {
        Word {
            word: text.to_string(),
            start,
            end,
            confidence: 1.0,
            channel: 0,
            speaker: None,
            punctuated_word: Some(text.to_string()),
        }
    }

    fn seg(start: f64, end: f64, speaker: usize) -> DiarizationSegment {
        DiarizationSegment {
            start_seconds: start,
            end_seconds: end,
            speaker,
        }
    }

    #[test]
    fn assigns_speakers_by_timestamp_overlap() {
        let mut words = vec![
            word("hello", 0.0, 0.5),
            word("world", 0.5, 1.0),
            word("foo", 3.0, 3.5),
            word("bar", 3.5, 4.0),
        ];
        let segments = vec![seg(0.0, 2.0, 0), seg(2.5, 4.5, 1)];

        assign_speakers_to_words(&mut words, &segments);

        assert_eq!(words[0].speaker, Some(0));
        assert_eq!(words[1].speaker, Some(0));
        assert_eq!(words[2].speaker, Some(1));
        assert_eq!(words[3].speaker, Some(1));
    }

    #[test]
    fn leaves_words_unassigned_when_no_segments() {
        let mut words = vec![word("test", 0.0, 1.0)];
        assign_speakers_to_words(&mut words, &[]);
        assert_eq!(words[0].speaker, None);
    }

    #[test]
    fn handles_word_between_segments_with_nearest_fallback() {
        let mut words = vec![word("gap", 2.1, 2.4)];
        let segments = vec![seg(0.0, 2.0, 0), seg(2.5, 4.0, 1)];

        assign_speakers_to_words(&mut words, &segments);

        assert!(words[0].speaker.is_some());
    }

    #[test]
    fn multiple_speakers_assigned_to_same_channel() {
        let mut words = vec![
            word("a", 0.0, 0.5),
            word("b", 1.0, 1.5),
            word("c", 2.0, 2.5),
            word("d", 3.0, 3.5),
        ];
        let segments = vec![
            seg(0.0, 1.0, 0),
            seg(1.0, 2.0, 1),
            seg(2.0, 3.0, 0),
            seg(3.0, 4.0, 2),
        ];

        assign_speakers_to_words(&mut words, &segments);

        assert_eq!(words[0].speaker, Some(0));
        assert_eq!(words[1].speaker, Some(1));
        assert_eq!(words[2].speaker, Some(0));
        assert_eq!(words[3].speaker, Some(2));
    }

    #[test]
    fn integration_diarize_four_speaker_audio() {
        let audio_path = "/tmp/test-diarize.wav";
        if !std::path::Path::new(audio_path).exists() {
            eprintln!("skipping integration test: {audio_path} not found");
            return;
        }

        let seg_path = model_paths::segmentation_model_path().unwrap();
        let emb_path = model_paths::embedding_model_path().unwrap();

        if !seg_path.exists() || !emb_path.exists() {
            eprintln!("skipping integration test: models not downloaded");
            return;
        }

        let diarizer = Diarizer::with_num_speakers(&seg_path, &emb_path, Some(4))
            .expect("failed to create diarizer");

        let segments = diarizer
            .diarize_file(audio_path)
            .expect("diarization failed");

        assert!(!segments.is_empty(), "should find at least one segment");

        let speakers: std::collections::HashSet<_> = segments.iter().map(|s| s.speaker).collect();
        assert!(
            speakers.len() >= 2,
            "should detect multiple speakers, got {speakers:?}"
        );

        for seg in &segments {
            println!(
                "  {:.3}s -- {:.3}s  speaker_{}",
                seg.start_seconds, seg.end_seconds, seg.speaker
            );
        }
        println!(
            "  detected {} speakers across {} segments",
            speakers.len(),
            segments.len()
        );
    }
}
