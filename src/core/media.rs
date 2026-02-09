//! WBP2 Media Collection Module
//!
//! Image collection, video download, subtitle extraction, and media analysis.
//! See: Plans.md Phase 12

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

// ============================================================================
// Image Collection (12.1)
// ============================================================================

/// Request for POST /v2/media/images
#[derive(Debug, Clone, Deserialize)]
pub struct ImageCollectRequest {
    /// Session name
    pub session: String,
    
    /// CSS selector for image container (optional)
    #[serde(default)]
    pub container_selector: Option<String>,
    
    /// Minimum width filter
    #[serde(default)]
    pub min_width: Option<u32>,
    
    /// Minimum height filter
    #[serde(default)]
    pub min_height: Option<u32>,
    
    /// URL pattern filter (regex)
    #[serde(default)]
    pub url_pattern: Option<String>,
    
    /// Output format
    #[serde(default)]
    pub output: ImageOutputFormat,
    
    /// Maximum images to collect
    #[serde(default = "default_max_images")]
    pub max_images: u32,
    
    /// Download concurrency
    #[serde(default = "default_concurrency")]
    pub concurrency: u32,
}

fn default_max_images() -> u32 {
    100
}

fn default_concurrency() -> u32 {
    5
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ImageOutputFormat {
    /// Return URLs only
    #[default]
    Urls,
    /// Return base64 data inline
    Base64,
    /// Save to files and return reference
    Files,
    /// Create ZIP archive
    Zip,
}

/// Response for POST /v2/media/images
#[derive(Debug, Clone, Serialize)]
pub struct ImageCollectResponse {
    pub success: bool,
    /// Reference ID for file downloads
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
    /// Image URLs found
    pub images: Vec<ImageInfo>,
    /// Total count
    pub count: usize,
    /// Elapsed time
    pub elapsed_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImageInfo {
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base64: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
}

/// Generate JavaScript for image extraction
pub fn generate_image_extract_script(request: &ImageCollectRequest) -> String {
    let container = request.container_selector.as_deref().unwrap_or("body");
    let min_width = request.min_width.unwrap_or(0);
    let min_height = request.min_height.unwrap_or(0);
    let pattern = request.url_pattern.as_deref().unwrap_or(".*");
    let max_images = request.max_images;
    
    format!(r#"
(function() {{
    const container = document.querySelector("{}");
    if (!container) return JSON.stringify({{ error: "Container not found" }});
    
    const images = Array.from(container.querySelectorAll("img"));
    const pattern = new RegExp("{}");
    
    const results = images
        .filter(img => {{
            const w = img.naturalWidth || img.width;
            const h = img.naturalHeight || img.height;
            return w >= {} && h >= {} && pattern.test(img.src);
        }})
        .slice(0, {})
        .map(img => ({{
            url: img.src,
            width: img.naturalWidth || img.width,
            height: img.naturalHeight || img.height,
            alt: img.alt || null
        }}));
    
    return JSON.stringify({{ images: results, count: results.length }});
}})();
"#, 
        container.replace('"', "\\\""),
        pattern.replace('"', "\\\""),
        min_width, min_height, max_images)
}

// ============================================================================
// YouTube/Video (12.2, 12.3)
// ============================================================================

/// Request for POST /v2/media/youtube/subtitles
#[derive(Debug, Clone, Deserialize)]
pub struct SubtitleRequest {
    /// YouTube URL or video ID
    pub url: String,
    
    /// Preferred language(s)
    #[serde(default)]
    pub languages: Vec<String>,
    
    /// Include auto-generated subtitles
    #[serde(default = "default_true")]
    pub auto_generated: bool,
    
    /// Output format
    #[serde(default)]
    pub format: SubtitleFormat,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum SubtitleFormat {
    /// Plain text
    #[default]
    Text,
    /// SRT format
    Srt,
    /// VTT format
    Vtt,
    /// JSON with timestamps
    Json,
}

/// Response for POST /v2/media/youtube/subtitles
#[derive(Debug, Clone, Serialize)]
pub struct SubtitleResponse {
    pub success: bool,
    pub video_id: String,
    pub title: Option<String>,
    pub language: String,
    pub format: String,
    /// Subtitle content (inline for small, reference for large)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    /// Reference for large content
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
}

/// Request for POST /v2/media/youtube/download
#[derive(Debug, Clone, Deserialize)]
pub struct VideoDownloadRequest {
    /// YouTube URL or video ID
    pub url: String,
    
    /// Quality preference
    #[serde(default)]
    pub quality: VideoQuality,
    
    /// Audio only
    #[serde(default)]
    pub audio_only: bool,
    
    /// Format preference
    #[serde(default)]
    pub format: Option<String>,
    
    /// Embed metadata
    #[serde(default = "default_true")]
    pub embed_metadata: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum VideoQuality {
    /// Best available
    #[default]
    Best,
    /// 1080p or lower
    Hd,
    /// 720p or lower
    Sd,
    /// Lowest quality
    Low,
    /// Specific height
    #[serde(rename = "specific")]
    Specific(u32),
}

impl VideoQuality {
    pub fn to_ytdlp_format(&self) -> String {
        match self {
            VideoQuality::Best => "bestvideo+bestaudio/best".to_string(),
            VideoQuality::Hd => "bestvideo[height<=1080]+bestaudio/best[height<=1080]".to_string(),
            VideoQuality::Sd => "bestvideo[height<=720]+bestaudio/best[height<=720]".to_string(),
            VideoQuality::Low => "worstvideo+worstaudio/worst".to_string(),
            VideoQuality::Specific(h) => format!("bestvideo[height<={}]+bestaudio/best[height<={}]", h, h),
        }
    }
}

/// Response for video download
#[derive(Debug, Clone, Serialize)]
pub struct VideoDownloadResponse {
    pub success: bool,
    pub video_id: String,
    pub title: Option<String>,
    /// Reference ID for file access
    pub reference: String,
    /// Download status
    pub status: DownloadStatus,
    /// Progress 0-100 (if in progress)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress: Option<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DownloadStatus {
    Queued,
    InProgress,
    Completed,
    Failed,
}

// ============================================================================
// Video Analysis / FFmpeg (12.4)
// ============================================================================

/// Request for POST /v2/media/analyze
#[derive(Debug, Clone, Deserialize)]
pub struct VideoAnalyzeRequest {
    /// Video URL or file reference
    pub source: String,
    
    /// Analysis to perform
    pub analysis: Vec<AnalysisType>,
    
    /// Output options
    #[serde(default)]
    pub output: AnalysisOutput,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisType {
    /// Detect scene changes
    SceneChange,
    /// Extract I-frames/keyframes
    Keyframes,
    /// Extract frames at interval
    IntervalFrames { interval_seconds: f32 },
    /// Separate audio track
    AudioSeparate,
    /// Get video metadata
    Metadata,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AnalysisOutput {
    /// Output directory (default: temp)
    #[serde(default)]
    pub directory: Option<PathBuf>,
    /// Image format for frames
    #[serde(default = "default_frame_format")]
    pub frame_format: String,
    /// Audio format
    #[serde(default = "default_audio_format")]
    pub audio_format: String,
}

fn default_frame_format() -> String {
    "jpg".to_string()
}

fn default_audio_format() -> String {
    "mp3".to_string()
}

/// Response for video analysis
#[derive(Debug, Clone, Serialize)]
pub struct VideoAnalyzeResponse {
    pub success: bool,
    /// Reference ID for output files
    pub reference: String,
    /// Analysis results
    pub results: HashMap<String, AnalysisResult>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AnalysisResult {
    SceneChange {
        scenes: Vec<SceneInfo>,
        count: usize,
    },
    Keyframes {
        frames: Vec<FrameInfo>,
        count: usize,
    },
    IntervalFrames {
        frames: Vec<FrameInfo>,
        interval: f32,
        count: usize,
    },
    AudioSeparate {
        filename: String,
        duration_seconds: f32,
    },
    Metadata {
        duration_seconds: f32,
        width: u32,
        height: u32,
        fps: f32,
        codec: String,
    },
}

#[derive(Debug, Clone, Serialize)]
pub struct SceneInfo {
    pub timestamp: f32,
    pub frame_filename: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct FrameInfo {
    pub timestamp: f32,
    pub filename: String,
}

// ============================================================================
// Media File Reference System (12.5)
// ============================================================================

/// Media reference for file access
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaReference {
    pub id: String,
    pub created_at: String,
    pub expires_at: String,
    pub files: Vec<MediaFile>,
    pub total_size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaFile {
    pub filename: String,
    pub size: u64,
    pub mime_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, serde_json::Value>>,
}

/// Media cache manager
#[derive(Debug, Clone, Default)]
pub struct MediaCache {
    /// URL -> Reference mapping
    pub url_map: HashMap<String, String>,
    /// Reference -> Files mapping
    pub references: HashMap<String, MediaReference>,
}

impl MediaCache {
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Create a new reference
    pub fn create_reference(&mut self) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono_now_iso8601();
        
        self.references.insert(id.clone(), MediaReference {
            id: id.clone(),
            created_at: now.clone(),
            expires_at: now, // TODO: Add proper expiration
            files: Vec::new(),
            total_size: 0,
        });
        
        id
    }
    
    /// Get reference by ID
    pub fn get_reference(&self, id: &str) -> Option<&MediaReference> {
        self.references.get(id)
    }
    
    /// Check if URL is cached
    pub fn get_cached(&self, url: &str) -> Option<&str> {
        self.url_map.get(url).map(|s| s.as_str())
    }
    
    /// Add URL to cache
    pub fn cache_url(&mut self, url: &str, reference: &str) {
        self.url_map.insert(url.to_string(), reference.to_string());
    }
    
    /// Cleanup expired references
    pub fn cleanup_expired(&mut self) -> Vec<String> {
        // TODO: Implement expiration check
        Vec::new()
    }
}

fn chrono_now_iso8601() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}", now.as_secs())
}

// ============================================================================
// FFmpeg Command Generation
// ============================================================================

/// Generate FFmpeg command for scene detection
pub fn generate_scene_detect_cmd(input: &str, threshold: f32) -> Vec<String> {
    vec![
        "-i".to_string(), input.to_string(),
        "-vf".to_string(), format!("select='gt(scene,{})',showinfo", threshold),
        "-vsync".to_string(), "vfr".to_string(),
        "-f".to_string(), "null".to_string(),
        "-".to_string(),
    ]
}

/// Generate FFmpeg command for keyframe extraction
pub fn generate_keyframe_extract_cmd(input: &str, output_pattern: &str) -> Vec<String> {
    vec![
        "-i".to_string(), input.to_string(),
        "-vf".to_string(), "select='eq(pict_type,I)'".to_string(),
        "-vsync".to_string(), "vfr".to_string(),
        "-q:v".to_string(), "2".to_string(),
        output_pattern.to_string(),
    ]
}

/// Generate FFmpeg command for interval frame extraction
pub fn generate_interval_extract_cmd(input: &str, interval: f32, output_pattern: &str) -> Vec<String> {
    vec![
        "-i".to_string(), input.to_string(),
        "-vf".to_string(), format!("fps=1/{}", interval),
        "-q:v".to_string(), "2".to_string(),
        output_pattern.to_string(),
    ]
}

/// Generate FFmpeg command for audio extraction
pub fn generate_audio_extract_cmd(input: &str, output: &str, format: &str) -> Vec<String> {
    match format {
        "mp3" => vec![
            "-i".to_string(), input.to_string(),
            "-vn".to_string(),
            "-acodec".to_string(), "libmp3lame".to_string(),
            "-ab".to_string(), "192k".to_string(),
            output.to_string(),
        ],
        "wav" => vec![
            "-i".to_string(), input.to_string(),
            "-vn".to_string(),
            "-acodec".to_string(), "pcm_s16le".to_string(),
            output.to_string(),
        ],
        _ => vec![
            "-i".to_string(), input.to_string(),
            "-vn".to_string(),
            "-c:a".to_string(), "copy".to_string(),
            output.to_string(),
        ],
    }
}

/// Generate yt-dlp command for subtitle extraction
pub fn generate_ytdlp_subtitle_cmd(url: &str, lang: &str, auto: bool) -> Vec<String> {
    let mut cmd = vec![
        url.to_string(),
        "--write-sub".to_string(),
        "--sub-lang".to_string(), lang.to_string(),
        "--skip-download".to_string(),
    ];
    
    if auto {
        cmd.push("--write-auto-sub".to_string());
    }
    
    cmd
}

/// Generate yt-dlp command for video download
pub fn generate_ytdlp_download_cmd(url: &str, quality: &VideoQuality, output: &str) -> Vec<String> {
    vec![
        url.to_string(),
        "-f".to_string(), quality.to_ytdlp_format(),
        "-o".to_string(), output.to_string(),
        "--embed-metadata".to_string(),
        "--progress".to_string(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_image_output_format() {
        assert_eq!(ImageOutputFormat::default(), ImageOutputFormat::Urls);
        assert_ne!(ImageOutputFormat::Urls, ImageOutputFormat::Base64);
    }
    
    #[test]
    fn test_video_quality_format() {
        assert_eq!(
            VideoQuality::Best.to_ytdlp_format(),
            "bestvideo+bestaudio/best"
        );
        assert!(VideoQuality::Hd.to_ytdlp_format().contains("1080"));
        assert!(VideoQuality::Sd.to_ytdlp_format().contains("720"));
        assert!(VideoQuality::Low.to_ytdlp_format().contains("worst"));
        assert!(VideoQuality::Specific(480).to_ytdlp_format().contains("480"));
    }
    
    #[test]
    fn test_generate_scene_detect_cmd() {
        let cmd = generate_scene_detect_cmd("input.mp4", 0.3);
        assert!(cmd.contains(&"-i".to_string()));
        assert!(cmd.iter().any(|s| s.contains("scene")));
        assert!(cmd.iter().any(|s| s.contains("0.3")));
    }
    
    #[test]
    fn test_generate_keyframe_extract_cmd() {
        let cmd = generate_keyframe_extract_cmd("input.mp4", "frame_%04d.jpg");
        assert!(cmd.contains(&"-i".to_string()));
        assert!(cmd.iter().any(|s| s.contains("pict_type")));
        assert!(cmd.contains(&"frame_%04d.jpg".to_string()));
    }
    
    #[test]
    fn test_generate_interval_extract_cmd() {
        let cmd = generate_interval_extract_cmd("input.mp4", 5.0, "frame_%04d.jpg");
        assert!(cmd.contains(&"-i".to_string()));
        assert!(cmd.iter().any(|s| s.contains("fps=1/5")));
    }
    
    #[test]
    fn test_generate_audio_extract_cmd_mp3() {
        let cmd = generate_audio_extract_cmd("input.mp4", "output.mp3", "mp3");
        assert!(cmd.contains(&"-i".to_string()));
        assert!(cmd.contains(&"-vn".to_string()));
        assert!(cmd.contains(&"libmp3lame".to_string()));
        assert!(cmd.contains(&"192k".to_string()));
    }
    
    #[test]
    fn test_generate_audio_extract_cmd_wav() {
        let cmd = generate_audio_extract_cmd("input.mp4", "output.wav", "wav");
        assert!(cmd.contains(&"pcm_s16le".to_string()));
    }
    
    #[test]
    fn test_generate_audio_extract_cmd_other() {
        let cmd = generate_audio_extract_cmd("input.mp4", "output.aac", "aac");
        assert!(cmd.contains(&"copy".to_string()));
    }
    
    #[test]
    fn test_generate_ytdlp_subtitle_cmd() {
        let cmd = generate_ytdlp_subtitle_cmd("https://youtube.com/watch?v=abc", "en", true);
        assert!(cmd.contains(&"--write-sub".to_string()));
        assert!(cmd.contains(&"--write-auto-sub".to_string()));
        assert!(cmd.contains(&"--sub-lang".to_string()));
    }
    
    #[test]
    fn test_generate_ytdlp_subtitle_cmd_no_auto() {
        let cmd = generate_ytdlp_subtitle_cmd("https://youtube.com/watch?v=abc", "ja", false);
        assert!(!cmd.contains(&"--write-auto-sub".to_string()));
    }
    
    #[test]
    fn test_generate_ytdlp_download_cmd() {
        let cmd = generate_ytdlp_download_cmd(
            "https://youtube.com/watch?v=abc",
            &VideoQuality::Hd,
            "output.mp4"
        );
        assert!(cmd.iter().any(|s| s.contains("1080")));
        assert!(cmd.contains(&"--embed-metadata".to_string()));
    }
    
    #[test]
    fn test_media_cache() {
        let mut cache = MediaCache::new();
        let ref_id = cache.create_reference();
        
        assert!(cache.get_reference(&ref_id).is_some());
        
        cache.cache_url("https://example.com/img.jpg", &ref_id);
        assert_eq!(cache.get_cached("https://example.com/img.jpg"), Some(ref_id.as_str()));
    }
    
    #[test]
    fn test_media_cache_not_found() {
        let cache = MediaCache::new();
        assert!(cache.get_reference("nonexistent").is_none());
        assert!(cache.get_cached("https://not-cached.com/img.jpg").is_none());
    }
    
    #[test]
    fn test_media_cache_cleanup() {
        let mut cache = MediaCache::new();
        let expired = cache.cleanup_expired();
        assert!(expired.is_empty()); // Currently not implemented
    }
    
    #[test]
    fn test_generate_image_extract_script() {
        let request = ImageCollectRequest {
            session: "test".to_string(),
            container_selector: Some("#gallery".to_string()),
            min_width: Some(100),
            min_height: Some(100),
            url_pattern: None,
            output: ImageOutputFormat::Urls,
            max_images: 50,
            concurrency: 5,
        };
        
        let script = generate_image_extract_script(&request);
        assert!(script.contains("#gallery"));
        assert!(script.contains("querySelectorAll"));
        assert!(script.contains("100")); // min dimensions
    }
    
    #[test]
    fn test_generate_image_extract_script_defaults() {
        let request = ImageCollectRequest {
            session: "test".to_string(),
            container_selector: None,
            min_width: None,
            min_height: None,
            url_pattern: None,
            output: ImageOutputFormat::Urls,
            max_images: 100,
            concurrency: 5,
        };
        
        let script = generate_image_extract_script(&request);
        assert!(script.contains("body")); // default container
    }
    
    #[test]
    fn test_subtitle_format_default() {
        assert_eq!(SubtitleFormat::default(), SubtitleFormat::Text);
    }
    
    #[test]
    fn test_download_status_equality() {
        assert_eq!(DownloadStatus::Queued, DownloadStatus::Queued);
        assert_ne!(DownloadStatus::Queued, DownloadStatus::Completed);
    }
    
    #[test]
    fn test_analysis_type_equality() {
        assert_eq!(AnalysisType::SceneChange, AnalysisType::SceneChange);
        assert_eq!(AnalysisType::Keyframes, AnalysisType::Keyframes);
    }
    
    #[test]
    fn test_default_functions() {
        assert_eq!(default_max_images(), 100);
        assert_eq!(default_concurrency(), 5);
        assert!(default_true());
        assert_eq!(default_frame_format(), "jpg");
        assert_eq!(default_audio_format(), "mp3");
    }
    
    #[test]
    fn test_image_collect_request_deserialize() {
        let json = r##"{"session": "main"}"##;
        let req: ImageCollectRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.session, "main");
        assert_eq!(req.max_images, 100); // default
        assert_eq!(req.concurrency, 5); // default
    }
    
    #[test]
    fn test_subtitle_request_deserialize() {
        let json = r##"{"url": "https://youtube.com/watch?v=abc"}"##;
        let req: SubtitleRequest = serde_json::from_str(json).unwrap();
        assert!(req.auto_generated); // default true
        assert_eq!(req.format, SubtitleFormat::Text);
    }
    
    #[test]
    fn test_video_download_request_deserialize() {
        let json = r##"{"url": "https://youtube.com/watch?v=abc"}"##;
        let req: VideoDownloadRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.quality, VideoQuality::Best);
        assert!(!req.audio_only);
        assert!(req.embed_metadata); // default true
    }
    
    #[test]
    fn test_analysis_output_default() {
        let output = AnalysisOutput::default();
        assert!(output.directory.is_none());
        // Default derive uses String::default() (empty string)
        // serde defaults only apply during deserialization
        assert_eq!(output.frame_format, "");
        assert_eq!(output.audio_format, "");
    }
    
    #[test]
    fn test_video_quality_equality() {
        assert_eq!(VideoQuality::Best, VideoQuality::Best);
        assert_ne!(VideoQuality::Best, VideoQuality::Low);
        assert_eq!(VideoQuality::Specific(720), VideoQuality::Specific(720));
    }
}

