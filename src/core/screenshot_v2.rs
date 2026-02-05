//! WBP2 Screenshot v2 Module
//!
//! Advanced screenshot capabilities: element capture, full-page, device emulation.
//! See: Plans.md Phase 9

use serde::{Deserialize, Serialize};

// ============================================================================
// Screenshot Request/Response Types
// ============================================================================

/// Request for POST /v2/screenshot
#[derive(Debug, Clone, Deserialize)]
pub struct ScreenshotRequest {
    /// Session name
    pub session: String,
    
    /// Capture mode
    #[serde(default)]
    pub mode: CaptureMode,
    
    /// Element selector (for element mode)
    #[serde(default)]
    pub selector: Option<String>,
    
    /// Image format
    #[serde(default)]
    pub format: ImageFormat,
    
    /// Quality (1-100, for JPEG/WebP)
    #[serde(default = "default_quality")]
    pub quality: u8,
    
    /// Wait for images to load
    #[serde(default = "default_true")]
    pub wait_for_images: bool,
    
    /// Device emulation preset
    #[serde(default)]
    pub device: Option<String>,
    
    /// Custom viewport size
    #[serde(default)]
    pub viewport: Option<Viewport>,
    
    /// Padding around element (for element mode)
    #[serde(default)]
    pub padding: Option<u32>,
    
    /// Hide elements matching these selectors
    #[serde(default)]
    pub hide_selectors: Option<Vec<String>>,
    
    /// Timeout in ms
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
}

fn default_quality() -> u8 {
    90
}

fn default_true() -> bool {
    true
}

fn default_timeout() -> u64 {
    30000
}

/// Capture mode
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CaptureMode {
    /// Capture visible viewport only
    #[default]
    Viewport,
    /// Capture entire page (scrolling)
    FullPage,
    /// Capture specific element
    Element,
}

/// Image format
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ImageFormat {
    #[default]
    Png,
    Jpeg,
    Webp,
}

impl ImageFormat {
    pub fn mime_type(&self) -> &'static str {
        match self {
            ImageFormat::Png => "image/png",
            ImageFormat::Jpeg => "image/jpeg",
            ImageFormat::Webp => "image/webp",
        }
    }
    
    pub fn extension(&self) -> &'static str {
        match self {
            ImageFormat::Png => "png",
            ImageFormat::Jpeg => "jpg",
            ImageFormat::Webp => "webp",
        }
    }
}

/// Viewport configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Viewport {
    pub width: u32,
    pub height: u32,
    #[serde(default = "default_scale")]
    pub device_scale_factor: f32,
    #[serde(default)]
    pub is_mobile: bool,
    #[serde(default)]
    pub has_touch: bool,
}

fn default_scale() -> f32 {
    1.0
}

/// Response for POST /v2/screenshot
#[derive(Debug, Clone, Serialize)]
pub struct ScreenshotResponse {
    pub success: bool,
    /// Base64-encoded image data
    pub data: String,
    /// MIME type
    pub mime_type: String,
    /// Image dimensions
    pub width: u32,
    pub height: u32,
    /// Time taken in ms
    pub elapsed_ms: u64,
}

// ============================================================================
// Device Presets
// ============================================================================

/// Common device presets for emulation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DevicePreset {
    pub name: String,
    pub viewport: Viewport,
    pub user_agent: String,
}

/// Get built-in device presets
pub fn get_device_presets() -> Vec<DevicePreset> {
    vec![
        // Mobile devices
        DevicePreset {
            name: "iphone_14".to_string(),
            viewport: Viewport {
                width: 390,
                height: 844,
                device_scale_factor: 3.0,
                is_mobile: true,
                has_touch: true,
            },
            user_agent: "Mozilla/5.0 (iPhone; CPU iPhone OS 16_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/16.0 Mobile/15E148 Safari/604.1".to_string(),
        },
        DevicePreset {
            name: "iphone_14_pro_max".to_string(),
            viewport: Viewport {
                width: 430,
                height: 932,
                device_scale_factor: 3.0,
                is_mobile: true,
                has_touch: true,
            },
            user_agent: "Mozilla/5.0 (iPhone; CPU iPhone OS 16_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/16.0 Mobile/15E148 Safari/604.1".to_string(),
        },
        DevicePreset {
            name: "pixel_7".to_string(),
            viewport: Viewport {
                width: 412,
                height: 915,
                device_scale_factor: 2.625,
                is_mobile: true,
                has_touch: true,
            },
            user_agent: "Mozilla/5.0 (Linux; Android 13; Pixel 7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/116.0.0.0 Mobile Safari/537.36".to_string(),
        },
        DevicePreset {
            name: "galaxy_s23".to_string(),
            viewport: Viewport {
                width: 360,
                height: 780,
                device_scale_factor: 3.0,
                is_mobile: true,
                has_touch: true,
            },
            user_agent: "Mozilla/5.0 (Linux; Android 13; SM-S911B) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/116.0.0.0 Mobile Safari/537.36".to_string(),
        },
        // Tablets
        DevicePreset {
            name: "ipad_pro_12".to_string(),
            viewport: Viewport {
                width: 1024,
                height: 1366,
                device_scale_factor: 2.0,
                is_mobile: true,
                has_touch: true,
            },
            user_agent: "Mozilla/5.0 (iPad; CPU OS 16_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/16.0 Mobile/15E148 Safari/604.1".to_string(),
        },
        // Desktop
        DevicePreset {
            name: "desktop_1080p".to_string(),
            viewport: Viewport {
                width: 1920,
                height: 1080,
                device_scale_factor: 1.0,
                is_mobile: false,
                has_touch: false,
            },
            user_agent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/116.0.0.0 Safari/537.36".to_string(),
        },
        DevicePreset {
            name: "desktop_1440p".to_string(),
            viewport: Viewport {
                width: 2560,
                height: 1440,
                device_scale_factor: 1.0,
                is_mobile: false,
                has_touch: false,
            },
            user_agent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/116.0.0.0 Safari/537.36".to_string(),
        },
        DevicePreset {
            name: "macbook_pro_14".to_string(),
            viewport: Viewport {
                width: 1512,
                height: 982,
                device_scale_factor: 2.0,
                is_mobile: false,
                has_touch: false,
            },
            user_agent: "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/16.0 Safari/605.1.15".to_string(),
        },
    ]
}

/// Find a device preset by name
pub fn find_device_preset(name: &str) -> Option<DevicePreset> {
    get_device_presets()
        .into_iter()
        .find(|d| d.name.eq_ignore_ascii_case(name))
}

// ============================================================================
// Screenshot Scripts
// ============================================================================

/// Generate JavaScript for element screenshot
pub fn generate_element_screenshot_script(selector: &str, padding: u32) -> String {
    format!(r#"
(function() {{
    const el = document.querySelector("{}");
    if (!el) {{
        return JSON.stringify({{ error: "Element not found" }});
    }}
    
    const rect = el.getBoundingClientRect();
    const padding = {};
    
    return JSON.stringify({{
        x: Math.max(0, rect.x - padding),
        y: Math.max(0, rect.y - padding + window.scrollY),
        width: rect.width + padding * 2,
        height: rect.height + padding * 2,
        scrollY: window.scrollY
    }});
}})();
"#, selector.replace('"', "\\\""), padding)
}

/// Generate JavaScript for full-page dimensions
pub fn generate_full_page_dimensions_script() -> String {
    r#"
(function() {
    const body = document.body;
    const html = document.documentElement;
    
    const height = Math.max(
        body.scrollHeight, body.offsetHeight,
        html.clientHeight, html.scrollHeight, html.offsetHeight
    );
    
    const width = Math.max(
        body.scrollWidth, body.offsetWidth,
        html.clientWidth, html.scrollWidth, html.offsetWidth
    );
    
    return JSON.stringify({
        width: width,
        height: height,
        viewportWidth: window.innerWidth,
        viewportHeight: window.innerHeight
    });
})();
"#.to_string()
}

/// Generate JavaScript to wait for images
pub fn generate_wait_for_images_script(timeout_ms: u64) -> String {
    format!(r#"
(function() {{
    return new Promise((resolve) => {{
        const images = Array.from(document.images);
        const startTime = Date.now();
        const timeout = {};
        
        function checkImages() {{
            const allLoaded = images.every(img => img.complete);
            
            if (allLoaded) {{
                resolve(JSON.stringify({{ loaded: images.length, elapsed: Date.now() - startTime }}));
            }} else if (Date.now() - startTime > timeout) {{
                const loaded = images.filter(img => img.complete).length;
                resolve(JSON.stringify({{ loaded: loaded, total: images.length, timeout: true }}));
            }} else {{
                setTimeout(checkImages, 100);
            }}
        }}
        
        if (images.length === 0) {{
            resolve(JSON.stringify({{ loaded: 0, elapsed: 0 }}));
        }} else {{
            checkImages();
        }}
    }});
}})();
"#, timeout_ms)
}

/// Generate JavaScript to scroll to position
pub fn generate_scroll_to_script(x: i32, y: i32) -> String {
    format!(r#"
window.scrollTo({}, {});
JSON.stringify({{ scrollX: window.scrollX, scrollY: window.scrollY }});
"#, x, y)
}

/// Generate JavaScript to hide elements
pub fn generate_hide_elements_script(selectors: &[String]) -> String {
    let selectors_json = serde_json::to_string(selectors).unwrap_or_else(|_| "[]".to_string());
    format!(r#"
(function() {{
    const selectors = {};
    const hidden = [];
    
    for (const selector of selectors) {{
        const elements = document.querySelectorAll(selector);
        for (const el of elements) {{
            el.style.setProperty('visibility', 'hidden', 'important');
            hidden.push(selector);
        }}
    }}
    
    return JSON.stringify({{ hidden: hidden.length }});
}})();
"#, selectors_json)
}

/// Generate JavaScript to restore hidden elements
pub fn generate_restore_elements_script(selectors: &[String]) -> String {
    let selectors_json = serde_json::to_string(selectors).unwrap_or_else(|_| "[]".to_string());
    format!(r#"
(function() {{
    const selectors = {};
    let restored = 0;
    
    for (const selector of selectors) {{
        const elements = document.querySelectorAll(selector);
        for (const el of elements) {{
            el.style.removeProperty('visibility');
            restored++;
        }}
    }}
    
    return JSON.stringify({{ restored: restored }});
}})();
"#, selectors_json)
}

/// Generate JavaScript to set viewport meta tag (for mobile emulation)
pub fn generate_viewport_meta_script(width: u32, height: u32, _scale: f32) -> String {
    format!(r#"
(function() {{
    let viewport = document.querySelector('meta[name="viewport"]');
    if (!viewport) {{
        viewport = document.createElement('meta');
        viewport.name = 'viewport';
        document.head.appendChild(viewport);
    }}
    viewport.content = 'width={}, height={}, initial-scale=1';
    return JSON.stringify({{ set: true }});
}})();
"#, width, height)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_capture_mode_default() {
        assert_eq!(CaptureMode::default(), CaptureMode::Viewport);
    }
    
    #[test]
    fn test_image_format_mime_type() {
        assert_eq!(ImageFormat::Png.mime_type(), "image/png");
        assert_eq!(ImageFormat::Jpeg.mime_type(), "image/jpeg");
        assert_eq!(ImageFormat::Webp.mime_type(), "image/webp");
    }
    
    #[test]
    fn test_find_device_preset() {
        let preset = find_device_preset("iphone_14");
        assert!(preset.is_some());
        let preset = preset.unwrap();
        assert_eq!(preset.viewport.width, 390);
        assert!(preset.viewport.is_mobile);
    }
    
    #[test]
    fn test_device_preset_case_insensitive() {
        let preset = find_device_preset("IPHONE_14");
        assert!(preset.is_some());
    }
    
    #[test]
    fn test_generate_element_screenshot_script() {
        let script = generate_element_screenshot_script("#main", 10);
        assert!(script.contains("#main"));
        assert!(script.contains("getBoundingClientRect"));
    }
    
    #[test]
    fn test_screenshot_request_deserialize() {
        let json = r#"{
            "session": "test",
            "mode": "full_page",
            "format": "jpeg",
            "quality": 85
        }"#;
        
        let req: ScreenshotRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.session, "test");
        assert_eq!(req.mode, CaptureMode::FullPage);
        assert_eq!(req.format, ImageFormat::Jpeg);
        assert_eq!(req.quality, 85);
    }
}
