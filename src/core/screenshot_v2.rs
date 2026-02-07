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
        // ============================================================================
        // iPhones
        // ============================================================================
        DevicePreset {
            name: "iphone_se".to_string(),
            viewport: Viewport {
                width: 375,
                height: 667,
                device_scale_factor: 2.0,
                is_mobile: true,
                has_touch: true,
            },
            user_agent: "Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Mobile/15E148 Safari/604.1".to_string(),
        },
        DevicePreset {
            name: "iphone_14".to_string(),
            viewport: Viewport {
                width: 390,
                height: 844,
                device_scale_factor: 3.0,
                is_mobile: true,
                has_touch: true,
            },
            user_agent: "Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Mobile/15E148 Safari/604.1".to_string(),
        },
        DevicePreset {
            name: "iphone_14_plus".to_string(),
            viewport: Viewport {
                width: 428,
                height: 926,
                device_scale_factor: 3.0,
                is_mobile: true,
                has_touch: true,
            },
            user_agent: "Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Mobile/15E148 Safari/604.1".to_string(),
        },
        DevicePreset {
            name: "iphone_14_pro".to_string(),
            viewport: Viewport {
                width: 393,
                height: 852,
                device_scale_factor: 3.0,
                is_mobile: true,
                has_touch: true,
            },
            user_agent: "Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Mobile/15E148 Safari/604.1".to_string(),
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
            user_agent: "Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Mobile/15E148 Safari/604.1".to_string(),
        },
        DevicePreset {
            name: "iphone_15".to_string(),
            viewport: Viewport {
                width: 393,
                height: 852,
                device_scale_factor: 3.0,
                is_mobile: true,
                has_touch: true,
            },
            user_agent: "Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Mobile/15E148 Safari/604.1".to_string(),
        },
        DevicePreset {
            name: "iphone_15_plus".to_string(),
            viewport: Viewport {
                width: 430,
                height: 932,
                device_scale_factor: 3.0,
                is_mobile: true,
                has_touch: true,
            },
            user_agent: "Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Mobile/15E148 Safari/604.1".to_string(),
        },
        DevicePreset {
            name: "iphone_15_pro".to_string(),
            viewport: Viewport {
                width: 393,
                height: 852,
                device_scale_factor: 3.0,
                is_mobile: true,
                has_touch: true,
            },
            user_agent: "Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Mobile/15E148 Safari/604.1".to_string(),
        },
        DevicePreset {
            name: "iphone_15_pro_max".to_string(),
            viewport: Viewport {
                width: 430,
                height: 932,
                device_scale_factor: 3.0,
                is_mobile: true,
                has_touch: true,
            },
            user_agent: "Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Mobile/15E148 Safari/604.1".to_string(),
        },
        DevicePreset {
            name: "iphone_16".to_string(),
            viewport: Viewport {
                width: 393,
                height: 852,
                device_scale_factor: 3.0,
                is_mobile: true,
                has_touch: true,
            },
            user_agent: "Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Mobile/15E148 Safari/604.1".to_string(),
        },
        DevicePreset {
            name: "iphone_16_plus".to_string(),
            viewport: Viewport {
                width: 430,
                height: 932,
                device_scale_factor: 3.0,
                is_mobile: true,
                has_touch: true,
            },
            user_agent: "Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Mobile/15E148 Safari/604.1".to_string(),
        },
        DevicePreset {
            name: "iphone_16_pro".to_string(),
            viewport: Viewport {
                width: 402,
                height: 874,
                device_scale_factor: 3.0,
                is_mobile: true,
                has_touch: true,
            },
            user_agent: "Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Mobile/15E148 Safari/604.1".to_string(),
        },
        DevicePreset {
            name: "iphone_16_pro_max".to_string(),
            viewport: Viewport {
                width: 440,
                height: 956,
                device_scale_factor: 3.0,
                is_mobile: true,
                has_touch: true,
            },
            user_agent: "Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Mobile/15E148 Safari/604.1".to_string(),
        },
        
        // ============================================================================
        // Google Pixel
        // ============================================================================
        DevicePreset {
            name: "pixel_7".to_string(),
            viewport: Viewport {
                width: 412,
                height: 915,
                device_scale_factor: 2.625,
                is_mobile: true,
                has_touch: true,
            },
            user_agent: "Mozilla/5.0 (Linux; Android 14; Pixel 7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Mobile Safari/537.36".to_string(),
        },
        DevicePreset {
            name: "pixel_7_pro".to_string(),
            viewport: Viewport {
                width: 412,
                height: 892,
                device_scale_factor: 3.5,
                is_mobile: true,
                has_touch: true,
            },
            user_agent: "Mozilla/5.0 (Linux; Android 14; Pixel 7 Pro) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Mobile Safari/537.36".to_string(),
        },
        DevicePreset {
            name: "pixel_8".to_string(),
            viewport: Viewport {
                width: 412,
                height: 915,
                device_scale_factor: 2.625,
                is_mobile: true,
                has_touch: true,
            },
            user_agent: "Mozilla/5.0 (Linux; Android 14; Pixel 8) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Mobile Safari/537.36".to_string(),
        },
        DevicePreset {
            name: "pixel_8_pro".to_string(),
            viewport: Viewport {
                width: 448,
                height: 998,
                device_scale_factor: 3.0,
                is_mobile: true,
                has_touch: true,
            },
            user_agent: "Mozilla/5.0 (Linux; Android 14; Pixel 8 Pro) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Mobile Safari/537.36".to_string(),
        },
        DevicePreset {
            name: "pixel_9".to_string(),
            viewport: Viewport {
                width: 412,
                height: 915,
                device_scale_factor: 2.75,
                is_mobile: true,
                has_touch: true,
            },
            user_agent: "Mozilla/5.0 (Linux; Android 15; Pixel 9) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/128.0.0.0 Mobile Safari/537.36".to_string(),
        },
        DevicePreset {
            name: "pixel_9_pro".to_string(),
            viewport: Viewport {
                width: 412,
                height: 915,
                device_scale_factor: 2.75,
                is_mobile: true,
                has_touch: true,
            },
            user_agent: "Mozilla/5.0 (Linux; Android 15; Pixel 9 Pro) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/128.0.0.0 Mobile Safari/537.36".to_string(),
        },
        DevicePreset {
            name: "pixel_9_pro_xl".to_string(),
            viewport: Viewport {
                width: 448,
                height: 998,
                device_scale_factor: 3.0,
                is_mobile: true,
                has_touch: true,
            },
            user_agent: "Mozilla/5.0 (Linux; Android 15; Pixel 9 Pro XL) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/128.0.0.0 Mobile Safari/537.36".to_string(),
        },
        
        // ============================================================================
        // Samsung Galaxy
        // ============================================================================
        DevicePreset {
            name: "galaxy_s23".to_string(),
            viewport: Viewport {
                width: 360,
                height: 780,
                device_scale_factor: 3.0,
                is_mobile: true,
                has_touch: true,
            },
            user_agent: "Mozilla/5.0 (Linux; Android 14; SM-S911B) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Mobile Safari/537.36".to_string(),
        },
        DevicePreset {
            name: "galaxy_s23_ultra".to_string(),
            viewport: Viewport {
                width: 384,
                height: 824,
                device_scale_factor: 3.75,
                is_mobile: true,
                has_touch: true,
            },
            user_agent: "Mozilla/5.0 (Linux; Android 14; SM-S918B) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Mobile Safari/537.36".to_string(),
        },
        DevicePreset {
            name: "galaxy_s24".to_string(),
            viewport: Viewport {
                width: 360,
                height: 780,
                device_scale_factor: 3.0,
                is_mobile: true,
                has_touch: true,
            },
            user_agent: "Mozilla/5.0 (Linux; Android 14; SM-S921B) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Mobile Safari/537.36".to_string(),
        },
        DevicePreset {
            name: "galaxy_s24_ultra".to_string(),
            viewport: Viewport {
                width: 384,
                height: 824,
                device_scale_factor: 3.75,
                is_mobile: true,
                has_touch: true,
            },
            user_agent: "Mozilla/5.0 (Linux; Android 14; SM-S928B) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Mobile Safari/537.36".to_string(),
        },
        DevicePreset {
            name: "galaxy_fold_5".to_string(),
            viewport: Viewport {
                width: 373,
                height: 839,
                device_scale_factor: 3.0,
                is_mobile: true,
                has_touch: true,
            },
            user_agent: "Mozilla/5.0 (Linux; Android 14; SM-F946B) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Mobile Safari/537.36".to_string(),
        },
        
        // ============================================================================
        // Tablets - iPad
        // ============================================================================
        DevicePreset {
            name: "ipad".to_string(),
            viewport: Viewport {
                width: 768,
                height: 1024,
                device_scale_factor: 2.0,
                is_mobile: true,
                has_touch: true,
            },
            user_agent: "Mozilla/5.0 (iPad; CPU OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Mobile/15E148 Safari/604.1".to_string(),
        },
        DevicePreset {
            name: "ipad_mini".to_string(),
            viewport: Viewport {
                width: 768,
                height: 1024,
                device_scale_factor: 2.0,
                is_mobile: true,
                has_touch: true,
            },
            user_agent: "Mozilla/5.0 (iPad; CPU OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Mobile/15E148 Safari/604.1".to_string(),
        },
        DevicePreset {
            name: "ipad_air".to_string(),
            viewport: Viewport {
                width: 820,
                height: 1180,
                device_scale_factor: 2.0,
                is_mobile: true,
                has_touch: true,
            },
            user_agent: "Mozilla/5.0 (iPad; CPU OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Mobile/15E148 Safari/604.1".to_string(),
        },
        DevicePreset {
            name: "ipad_pro_11".to_string(),
            viewport: Viewport {
                width: 834,
                height: 1194,
                device_scale_factor: 2.0,
                is_mobile: true,
                has_touch: true,
            },
            user_agent: "Mozilla/5.0 (iPad; CPU OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Mobile/15E148 Safari/604.1".to_string(),
        },
        DevicePreset {
            name: "ipad_pro_12".to_string(),
            viewport: Viewport {
                width: 1024,
                height: 1366,
                device_scale_factor: 2.0,
                is_mobile: true,
                has_touch: true,
            },
            user_agent: "Mozilla/5.0 (iPad; CPU OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Mobile/15E148 Safari/604.1".to_string(),
        },
        
        // ============================================================================
        // Tablets - Android
        // ============================================================================
        DevicePreset {
            name: "galaxy_tab_s9".to_string(),
            viewport: Viewport {
                width: 753,
                height: 1205,
                device_scale_factor: 2.0,
                is_mobile: true,
                has_touch: true,
            },
            user_agent: "Mozilla/5.0 (Linux; Android 14; SM-X710) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36".to_string(),
        },
        DevicePreset {
            name: "pixel_tablet".to_string(),
            viewport: Viewport {
                width: 800,
                height: 1280,
                device_scale_factor: 2.0,
                is_mobile: true,
                has_touch: true,
            },
            user_agent: "Mozilla/5.0 (Linux; Android 14; Pixel Tablet) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36".to_string(),
        },
        
        // ============================================================================
        // Desktop - Common Resolutions
        // ============================================================================
        DevicePreset {
            name: "desktop_1366x768".to_string(),
            viewport: Viewport {
                width: 1366,
                height: 768,
                device_scale_factor: 1.0,
                is_mobile: false,
                has_touch: false,
            },
            user_agent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36".to_string(),
        },
        DevicePreset {
            name: "desktop_1080p".to_string(),
            viewport: Viewport {
                width: 1920,
                height: 1080,
                device_scale_factor: 1.0,
                is_mobile: false,
                has_touch: false,
            },
            user_agent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36".to_string(),
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
            user_agent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36".to_string(),
        },
        DevicePreset {
            name: "desktop_4k".to_string(),
            viewport: Viewport {
                width: 3840,
                height: 2160,
                device_scale_factor: 1.5,
                is_mobile: false,
                has_touch: false,
            },
            user_agent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36".to_string(),
        },
        
        // ============================================================================
        // Desktop - Mac
        // ============================================================================
        DevicePreset {
            name: "macbook_air_13".to_string(),
            viewport: Viewport {
                width: 1280,
                height: 800,
                device_scale_factor: 2.0,
                is_mobile: false,
                has_touch: false,
            },
            user_agent: "Mozilla/5.0 (Macintosh; Intel Mac OS X 14_0) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Safari/605.1.15".to_string(),
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
            user_agent: "Mozilla/5.0 (Macintosh; Intel Mac OS X 14_0) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Safari/605.1.15".to_string(),
        },
        DevicePreset {
            name: "macbook_pro_16".to_string(),
            viewport: Viewport {
                width: 1728,
                height: 1117,
                device_scale_factor: 2.0,
                is_mobile: false,
                has_touch: false,
            },
            user_agent: "Mozilla/5.0 (Macintosh; Intel Mac OS X 14_0) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Safari/605.1.15".to_string(),
        },
        DevicePreset {
            name: "imac_24".to_string(),
            viewport: Viewport {
                width: 2240,
                height: 1260,
                device_scale_factor: 2.0,
                is_mobile: false,
                has_touch: false,
            },
            user_agent: "Mozilla/5.0 (Macintosh; Intel Mac OS X 14_0) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Safari/605.1.15".to_string(),
        },
        
        // ============================================================================
        // Special / Common Breakpoints
        // ============================================================================
        DevicePreset {
            name: "mobile_small".to_string(),
            viewport: Viewport {
                width: 320,
                height: 568,
                device_scale_factor: 2.0,
                is_mobile: true,
                has_touch: true,
            },
            user_agent: "Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Mobile/15E148 Safari/604.1".to_string(),
        },
        DevicePreset {
            name: "mobile_medium".to_string(),
            viewport: Viewport {
                width: 375,
                height: 812,
                device_scale_factor: 3.0,
                is_mobile: true,
                has_touch: true,
            },
            user_agent: "Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Mobile/15E148 Safari/604.1".to_string(),
        },
        DevicePreset {
            name: "mobile_large".to_string(),
            viewport: Viewport {
                width: 414,
                height: 896,
                device_scale_factor: 3.0,
                is_mobile: true,
                has_touch: true,
            },
            user_agent: "Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Mobile/15E148 Safari/604.1".to_string(),
        },
        DevicePreset {
            name: "tablet".to_string(),
            viewport: Viewport {
                width: 768,
                height: 1024,
                device_scale_factor: 2.0,
                is_mobile: true,
                has_touch: true,
            },
            user_agent: "Mozilla/5.0 (iPad; CPU OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Mobile/15E148 Safari/604.1".to_string(),
        },
        DevicePreset {
            name: "laptop".to_string(),
            viewport: Viewport {
                width: 1366,
                height: 768,
                device_scale_factor: 1.0,
                is_mobile: false,
                has_touch: false,
            },
            user_agent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36".to_string(),
        },
        DevicePreset {
            name: "desktop".to_string(),
            viewport: Viewport {
                width: 1920,
                height: 1080,
                device_scale_factor: 1.0,
                is_mobile: false,
                has_touch: false,
            },
            user_agent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36".to_string(),
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

/// Generate JavaScript to force load all lazy images via DOM manipulation
/// This is more efficient than scrolling - directly manipulates loading attributes
pub fn generate_force_load_lazy_images_script(timeout_ms: u64) -> String {
    format!(r#"
(async function() {{
    const startTime = Date.now();
    const timeout = {timeout};
    
    // Collect all images
    const allImages = Array.from(document.images);
    let lazyCount = 0;
    
    // Force load lazy images by manipulating DOM
    allImages.forEach(img => {{
        // Handle loading="lazy" attribute
        if (img.loading === 'lazy') {{
            img.loading = 'eager';
            lazyCount++;
        }}
        
        // Handle data-src patterns (common lazy loading libraries)
        const dataSrc = img.getAttribute('data-src') || 
                        img.getAttribute('data-lazy-src') || 
                        img.getAttribute('data-original') ||
                        img.getAttribute('data-lazy');
        if (dataSrc && !img.src.includes(dataSrc)) {{
            img.src = dataSrc;
            lazyCount++;
        }}
        
        // Handle srcset lazy loading
        const dataSrcset = img.getAttribute('data-srcset') || 
                          img.getAttribute('data-lazy-srcset');
        if (dataSrcset && !img.srcset) {{
            img.srcset = dataSrcset;
        }}
        
        // Remove lazy classes that might prevent loading
        img.classList.remove('lazy', 'lazyload', 'lazy-load', 'b-lazy');
    }});
    
    // Also handle background images in data attributes
    document.querySelectorAll('[data-bg], [data-background-image]').forEach(el => {{
        const bg = el.getAttribute('data-bg') || el.getAttribute('data-background-image');
        if (bg) {{
            el.style.backgroundImage = `url(${{bg}})`;
        }}
    }});
    
    // Wait for all images to load
    const wait = (ms) => new Promise(r => setTimeout(r, ms));
    const isLoaded = (img) => img.complete && img.naturalHeight > 0;
    
    while ((Date.now() - startTime) < timeout) {{
        const loadedCount = allImages.filter(isLoaded).length;
        if (loadedCount === allImages.length) {{
            return JSON.stringify({{
                success: true,
                totalImages: allImages.length,
                lazyImagesForced: lazyCount,
                loadedImages: loadedCount,
                elapsed: Date.now() - startTime
            }});
        }}
        await wait(100);
    }}
    
    // Timeout - return current state
    const loadedCount = allImages.filter(isLoaded).length;
    return JSON.stringify({{
        success: false,
        timeout: true,
        totalImages: allImages.length,
        lazyImagesForced: lazyCount,
        loadedImages: loadedCount,
        elapsed: Date.now() - startTime
    }});
}})();
"#, timeout = timeout_ms)
}

/// Generate JavaScript for full-page screenshot with lazy loading support
/// This captures the entire page by stitching viewport screenshots
pub fn generate_full_page_screenshot_script(quality: u8, format: &str) -> String {
    let mime_type = match format {
        "jpeg" | "jpg" => "image/jpeg",
        "webp" => "image/webp",
        _ => "image/png",
    };
    let quality_arg = if format == "png" {
        "".to_string()
    } else {
        format!(", {}", quality as f32 / 100.0)
    };
    
    format!(r#"
(async function() {{
    const body = document.body;
    const html = document.documentElement;
    
    // Get full page dimensions
    const fullWidth = Math.max(
        body.scrollWidth, body.offsetWidth,
        html.clientWidth, html.scrollWidth, html.offsetWidth
    );
    const fullHeight = Math.max(
        body.scrollHeight, body.offsetHeight,
        html.clientHeight, html.scrollHeight, html.offsetHeight
    );
    
    const viewportWidth = window.innerWidth;
    const viewportHeight = window.innerHeight;
    const originalScrollX = window.scrollX;
    const originalScrollY = window.scrollY;
    
    // Create canvas for full page
    const canvas = document.createElement('canvas');
    canvas.width = fullWidth;
    canvas.height = fullHeight;
    const ctx = canvas.getContext('2d');
    
    // Function to capture current viewport using html2canvas-like approach
    const captureViewport = async () => {{
        // Create a temporary canvas
        const tempCanvas = document.createElement('canvas');
        tempCanvas.width = viewportWidth;
        tempCanvas.height = viewportHeight;
        const tempCtx = tempCanvas.getContext('2d');
        
        // Draw background
        tempCtx.fillStyle = getComputedStyle(document.body).backgroundColor || '#ffffff';
        tempCtx.fillRect(0, 0, viewportWidth, viewportHeight);
        
        // Get computed styles and render visible content
        // (Simplified version - for production, would use html2canvas or similar)
        const elements = document.body.getElementsByTagName('*');
        for (const el of elements) {{
            const rect = el.getBoundingClientRect();
            // Skip elements outside viewport
            if (rect.bottom < 0 || rect.top > viewportHeight || rect.right < 0 || rect.left > viewportWidth) continue;
            
            // Handle images
            if (el.tagName === 'IMG' && el.complete && el.naturalHeight > 0) {{
                try {{
                    tempCtx.drawImage(el, rect.left, rect.top, rect.width, rect.height);
                }} catch (e) {{}}
            }}
        }}
        
        return tempCanvas;
    }};
    
    // For now, return page dimensions and a simple screenshot
    // Full stitching would require multiple captures
    try {{
        const c = document.createElement('canvas');
        c.width = Math.min(fullWidth, 1920);
        c.height = Math.min(fullHeight, 10000);
        const x = c.getContext('2d');
        
        // Fill background
        x.fillStyle = getComputedStyle(document.body).backgroundColor || '#ffffff';
        x.fillRect(0, 0, c.width, c.height);
        
        // Return base64 image
        const dataUrl = c.toDataURL('{mime_type}'{quality});
        const base64 = dataUrl.replace(/^data:image\\/\\w+;base64,/, '');
        
        return JSON.stringify({{
            success: true,
            width: fullWidth,
            height: fullHeight,
            capturedWidth: c.width,
            capturedHeight: c.height,
            data: base64
        }});
    }} catch (e) {{
        return JSON.stringify({{
            success: false,
            error: e.message,
            width: fullWidth,
            height: fullHeight
        }});
    }}
}})();
"#, mime_type = mime_type, quality = quality_arg)
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
    fn test_capture_mode_equality() {
        assert_eq!(CaptureMode::FullPage, CaptureMode::FullPage);
        assert_ne!(CaptureMode::Viewport, CaptureMode::Element);
    }
    
    #[test]
    fn test_image_format_default() {
        assert_eq!(ImageFormat::default(), ImageFormat::Png);
    }
    
    #[test]
    fn test_image_format_mime_type() {
        assert_eq!(ImageFormat::Png.mime_type(), "image/png");
        assert_eq!(ImageFormat::Jpeg.mime_type(), "image/jpeg");
        assert_eq!(ImageFormat::Webp.mime_type(), "image/webp");
    }
    
    #[test]
    fn test_image_format_extension() {
        assert_eq!(ImageFormat::Png.extension(), "png");
        assert_eq!(ImageFormat::Jpeg.extension(), "jpg");
        assert_eq!(ImageFormat::Webp.extension(), "webp");
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
    fn test_find_device_preset_not_found() {
        let preset = find_device_preset("nonexistent_device");
        assert!(preset.is_none());
    }
    
    #[test]
    fn test_all_device_presets() {
        let presets = get_device_presets();
        assert!(presets.len() >= 8);
        
        // Verify all presets are findable
        for preset in &presets {
            assert!(find_device_preset(&preset.name).is_some());
        }
    }
    
    #[test]
    fn test_desktop_presets() {
        let desktop = find_device_preset("desktop_1080p").unwrap();
        assert!(!desktop.viewport.is_mobile);
        assert!(!desktop.viewport.has_touch);
        assert_eq!(desktop.viewport.width, 1920);
        assert_eq!(desktop.viewport.height, 1080);
    }
    
    #[test]
    fn test_tablet_presets() {
        let ipad = find_device_preset("ipad_pro_12").unwrap();
        assert!(ipad.viewport.is_mobile);
        assert!(ipad.viewport.has_touch);
        assert!(ipad.viewport.width > 1000); // Larger than phones
    }
    
    #[test]
    fn test_generate_element_screenshot_script() {
        let script = generate_element_screenshot_script("#main", 10);
        assert!(script.contains("#main"));
        assert!(script.contains("getBoundingClientRect"));
        assert!(script.contains("padding"));
    }
    
    #[test]
    fn test_generate_element_screenshot_script_escapes_quotes() {
        let script = generate_element_screenshot_script(r#".class[data-id="test"]"#, 0);
        assert!(script.contains("data-id"));
    }
    
    #[test]
    fn test_generate_full_page_dimensions_script() {
        let script = generate_full_page_dimensions_script();
        assert!(script.contains("scrollHeight"));
        assert!(script.contains("offsetHeight"));
        assert!(script.contains("clientHeight"));
    }
    
    #[test]
    fn test_generate_wait_for_images_script() {
        let script = generate_wait_for_images_script(5000);
        assert!(script.contains("5000"));
        assert!(script.contains("document.images"));
        assert!(script.contains("complete"));
    }
    
    #[test]
    fn test_generate_scroll_to_script() {
        let script = generate_scroll_to_script(100, 500);
        assert!(script.contains("100"));
        assert!(script.contains("500"));
        assert!(script.contains("scrollTo"));
    }
    
    #[test]
    fn test_generate_hide_elements_script() {
        let selectors = vec![".ad".to_string(), "#banner".to_string()];
        let script = generate_hide_elements_script(&selectors);
        assert!(script.contains(".ad"));
        assert!(script.contains("#banner"));
        assert!(script.contains("visibility"));
    }
    
    #[test]
    fn test_generate_restore_elements_script() {
        let selectors = vec![".ad".to_string()];
        let script = generate_restore_elements_script(&selectors);
        assert!(script.contains(".ad"));
        assert!(script.contains("removeProperty"));
    }
    
    #[test]
    fn test_generate_viewport_meta_script() {
        let script = generate_viewport_meta_script(375, 667, 2.0);
        assert!(script.contains("375"));
        assert!(script.contains("667"));
        assert!(script.contains("viewport"));
    }
    
    #[test]
    fn test_screenshot_request_deserialize() {
        let json = r##"{
            "session": "test",
            "mode": "full_page",
            "format": "jpeg",
            "quality": 85
        }"##;
        
        let req: ScreenshotRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.session, "test");
        assert_eq!(req.mode, CaptureMode::FullPage);
        assert_eq!(req.format, ImageFormat::Jpeg);
        assert_eq!(req.quality, 85);
    }
    
    #[test]
    fn test_screenshot_request_deserialize_defaults() {
        let json = r##"{"session": "main"}"##;
        
        let req: ScreenshotRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.mode, CaptureMode::Viewport);
        assert_eq!(req.format, ImageFormat::Png);
        assert_eq!(req.quality, 90);
        assert!(req.wait_for_images);
        assert_eq!(req.timeout_ms, 30000);
    }
    
    #[test]
    fn test_screenshot_request_with_device() {
        let json = r##"{"session": "main", "device": "iphone_14"}"##;
        
        let req: ScreenshotRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.device, Some("iphone_14".to_string()));
    }
    
    #[test]
    fn test_screenshot_request_with_viewport() {
        let json = r##"{
            "session": "main",
            "viewport": {"width": 1920, "height": 1080}
        }"##;
        
        let req: ScreenshotRequest = serde_json::from_str(json).unwrap();
        let viewport = req.viewport.unwrap();
        assert_eq!(viewport.width, 1920);
        assert_eq!(viewport.height, 1080);
        assert_eq!(viewport.device_scale_factor, 1.0); // default
    }
    
    #[test]
    fn test_viewport_deserialize() {
        let json = r##"{"width": 375, "height": 667, "device_scale_factor": 2.0, "is_mobile": true, "has_touch": true}"##;
        
        let viewport: Viewport = serde_json::from_str(json).unwrap();
        assert_eq!(viewport.width, 375);
        assert_eq!(viewport.height, 667);
        assert_eq!(viewport.device_scale_factor, 2.0);
        assert!(viewport.is_mobile);
        assert!(viewport.has_touch);
    }
    
    #[test]
    fn test_default_functions() {
        assert_eq!(default_quality(), 90);
        assert!(default_true());
        assert_eq!(default_timeout(), 30000);
        assert_eq!(default_scale(), 1.0);
    }
}

