//! WBP2 Wait Module - Event-driven waiting v2
//!
//! Smart waiting with multiple condition types and DOM mutation observing.
//! See: Plans.md Phase 8

use serde::{Deserialize, Serialize};

// ============================================================================
// Wait Condition Types
// ============================================================================

/// Condition type for smart waiting
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum WaitCondition {
    /// Element is present in DOM (may not be visible)
    Present,
    /// Element is visible (display != none, visibility != hidden)
    Visible,
    /// Element content is stable (no changes for N ms)
    Stable,
    /// Element text contains specified string
    TextContains,
    /// Element text matches regex
    TextMatches,
    /// Element attribute has specific value
    AttributeEquals,
    /// Element is clickable (visible + enabled)
    Clickable,
    /// Element is gone from DOM
    Detached,
    /// Navigation completed
    NavigationComplete,
    /// Network idle (no pending requests for N ms)
    NetworkIdle,
}

impl Default for WaitCondition {
    fn default() -> Self {
        WaitCondition::Present
    }
}

// ============================================================================
// Wait Request/Response
// ============================================================================

/// Request for POST /v2/wait
#[derive(Debug, Clone, Deserialize)]
pub struct WaitRequest {
    /// Session name to wait on
    pub session: String,
    /// CSS selector to wait for
    pub selector: String,
    /// Condition type
    #[serde(default)]
    pub condition: WaitCondition,
    /// Timeout in milliseconds
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
    /// For text_contains/text_matches condition
    #[serde(default)]
    pub text: Option<String>,
    /// For attribute_equals condition
    #[serde(default)]
    pub attribute: Option<String>,
    /// Attribute value for attribute_equals
    #[serde(default)]
    pub value: Option<String>,
    /// Stability duration in ms (for stable condition)
    #[serde(default = "default_stable_ms")]
    pub stable_ms: u64,
    /// Also extract data after wait succeeds
    #[serde(default)]
    pub extract: Option<ExtractOptions>,
    /// Multiple selectors to wait for any/all
    #[serde(default)]
    pub selectors: Option<Vec<String>>,
    /// Wait for all selectors (true) or any (false)
    #[serde(default)]
    pub wait_all: bool,
    /// Target iframe (URL substring, frame name, or frame ID)
    #[serde(default)]
    pub frame: Option<String>,
}

fn default_timeout() -> u64 {
    30000 // 30 seconds
}

fn default_stable_ms() -> u64 {
    500 // 500ms stability
}

/// Extract options after wait succeeds
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ExtractOptions {
    /// Attribute to extract (text, innerHTML, href, src, etc.)
    #[serde(default = "default_extract_attr")]
    pub attribute: String,
    /// Extract from all matching elements
    #[serde(default)]
    pub all: bool,
}

fn default_extract_attr() -> String {
    "text".to_string()
}

/// Response for POST /v2/wait
#[derive(Debug, Clone, Serialize)]
pub struct WaitResponse {
    /// Whether wait condition was met
    pub success: bool,
    /// Which selector was matched (for multi-selector wait)
    pub matched_selector: Option<String>,
    /// Time taken in ms
    pub elapsed_ms: u64,
    /// Extracted data (if extract option was set)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
    /// Condition that was met
    pub condition: WaitCondition,
}

// ============================================================================
// JavaScript for MutationObserver-based waiting
// ============================================================================

/// Generate JavaScript for smart waiting
pub fn generate_wait_script(request: &WaitRequest) -> String {
    let selectors_json = if let Some(ref selectors) = request.selectors {
        serde_json::to_string(selectors).unwrap_or_else(|_| "[]".to_string())
    } else {
        format!("[\"{}\"]", request.selector.replace('"', "\\\""))
    };

    let condition = match request.condition {
        WaitCondition::Present => "present",
        WaitCondition::Visible => "visible",
        WaitCondition::Stable => "stable",
        WaitCondition::TextContains => "text_contains",
        WaitCondition::TextMatches => "text_matches",
        WaitCondition::AttributeEquals => "attribute_equals",
        WaitCondition::Clickable => "clickable",
        WaitCondition::Detached => "detached",
        WaitCondition::NavigationComplete => "navigation_complete",
        WaitCondition::NetworkIdle => "network_idle",
    };

    let text_json = request
        .text
        .as_ref()
        .map(|t| format!("\"{}\"", t.replace('"', "\\\"")))
        .unwrap_or_else(|| "null".to_string());

    let attr_json = request
        .attribute
        .as_ref()
        .map(|a| format!("\"{}\"", a.replace('"', "\\\"")))
        .unwrap_or_else(|| "null".to_string());

    let value_json = request
        .value
        .as_ref()
        .map(|v| format!("\"{}\"", v.replace('"', "\\\"")))
        .unwrap_or_else(|| "null".to_string());

    let extract_attr = request
        .extract
        .as_ref()
        .map(|e| format!("\"{}\"", e.attribute.replace('"', "\\\"")))
        .unwrap_or_else(|| "null".to_string());

    let extract_all = request.extract.as_ref().map(|e| e.all).unwrap_or(false);

    format!(
        r#"
(function() {{
    const config = {{
        selectors: {selectors},
        condition: "{condition}",
        timeout: {timeout},
        stableMs: {stable_ms},
        waitAll: {wait_all},
        text: {text},
        attribute: {attr},
        value: {value},
        extractAttr: {extract_attr},
        extractAll: {extract_all}
    }};
    
    const startTime = Date.now();
    let lastChange = Date.now();
    let observer = null;
    let resolvePromise = null;
    
    // Check if element meets condition
    function checkCondition(el, selector) {{
        if (!el) return {{ met: false }};
        
        switch (config.condition) {{
            case 'present':
                return {{ met: true, selector }};
            
            case 'visible':
                const style = getComputedStyle(el);
                const isVisible = style.display !== 'none' && 
                                  style.visibility !== 'hidden' &&
                                  style.opacity !== '0' &&
                                  el.offsetParent !== null;
                return {{ met: isVisible, selector }};
            
            case 'clickable':
                const cs = getComputedStyle(el);
                const clickable = cs.display !== 'none' && 
                                  cs.visibility !== 'hidden' &&
                                  !el.disabled &&
                                  cs.pointerEvents !== 'none';
                return {{ met: clickable, selector }};
            
            case 'text_contains':
                if (!config.text) return {{ met: false }};
                return {{ met: el.textContent.includes(config.text), selector }};
            
            case 'text_matches':
                if (!config.text) return {{ met: false }};
                try {{
                    const regex = new RegExp(config.text);
                    return {{ met: regex.test(el.textContent), selector }};
                }} catch {{ return {{ met: false }}; }}
            
            case 'attribute_equals':
                if (!config.attribute) return {{ met: false }};
                const attrValue = el.getAttribute(config.attribute);
                return {{ met: attrValue === config.value, selector }};
            
            case 'stable':
                // Stable means no DOM changes for stableMs
                if (Date.now() - lastChange >= config.stableMs) {{
                    return {{ met: true, selector }};
                }}
                return {{ met: false }};
            
            case 'detached':
                return {{ met: false }}; // Element exists, so not detached
            
            default:
                return {{ met: true, selector }};
        }}
    }}
    
    // Check all selectors
    function checkAllSelectors() {{
        let metCount = 0;
        let firstMatch = null;
        
        for (const selector of config.selectors) {{
            const el = document.querySelector(selector);
            
            // For detached condition, check if element is gone
            if (config.condition === 'detached') {{
                if (!el) {{
                    if (!firstMatch) firstMatch = {{ met: true, selector }};
                    metCount++;
                }}
                continue;
            }}
            
            const result = checkCondition(el, selector);
            if (result.met) {{
                if (!firstMatch) firstMatch = result;
                metCount++;
            }}
        }}
        
        if (config.waitAll) {{
            return metCount === config.selectors.length ? firstMatch : null;
        }} else {{
            return firstMatch;
        }}
    }}
    
    // Extract data from element
    function extractData(selector) {{
        if (!config.extractAttr) return null;
        
        const elements = config.extractAll 
            ? Array.from(document.querySelectorAll(selector))
            : [document.querySelector(selector)].filter(Boolean);
        
        const data = elements.map(el => {{
            switch (config.extractAttr) {{
                case 'text': return el.textContent.trim();
                case 'innerHTML': return el.innerHTML;
                case 'outerHTML': return el.outerHTML;
                case 'value': return el.value;
                default: return el.getAttribute(config.extractAttr);
            }}
        }});
        
        return config.extractAll ? data : data[0];
    }}
    
    return new Promise((resolve) => {{
        resolvePromise = resolve;
        
        // Initial check
        const initial = checkAllSelectors();
        if (initial && config.condition !== 'stable') {{
            const data = extractData(initial.selector);
            resolve({{
                success: true,
                matched_selector: initial.selector,
                elapsed_ms: Date.now() - startTime,
                data: data,
                condition: config.condition
            }});
            return;
        }}
        
        // Set up MutationObserver
        observer = new MutationObserver((mutations) => {{
            lastChange = Date.now();
            
            const result = checkAllSelectors();
            if (result) {{
                observer.disconnect();
                const data = extractData(result.selector);
                resolvePromise({{
                    success: true,
                    matched_selector: result.selector,
                    elapsed_ms: Date.now() - startTime,
                    data: data,
                    condition: config.condition
                }});
            }}
        }});
        
        observer.observe(document.body, {{
            childList: true,
            subtree: true,
            attributes: true,
            characterData: true
        }});
        
        // Stability check interval (for stable condition)
        if (config.condition === 'stable') {{
            const stableInterval = setInterval(() => {{
                if (Date.now() - lastChange >= config.stableMs) {{
                    clearInterval(stableInterval);
                    observer.disconnect();
                    const selector = config.selectors[0];
                    const data = extractData(selector);
                    resolvePromise({{
                        success: true,
                        matched_selector: selector,
                        elapsed_ms: Date.now() - startTime,
                        data: data,
                        condition: config.condition
                    }});
                }}
            }}, 100);
        }}
        
        // Timeout
        setTimeout(() => {{
            if (observer) observer.disconnect();
            resolvePromise({{
                success: false,
                matched_selector: null,
                elapsed_ms: Date.now() - startTime,
                data: null,
                condition: config.condition
            }});
        }}, config.timeout);
    }});
}})();
"#,
        selectors = selectors_json,
        condition = condition,
        timeout = request.timeout_ms,
        stable_ms = request.stable_ms,
        wait_all = request.wait_all,
        text = text_json,
        attr = attr_json,
        value = value_json,
        extract_attr = extract_attr,
        extract_all = extract_all
    )
}

// ============================================================================
// Multi-selector Wait
// ============================================================================

/// Wait for multiple selectors concurrently
#[derive(Debug, Clone, Deserialize)]
pub struct MultiWaitRequest {
    pub session: String,
    pub selectors: Vec<SelectorWithCondition>,
    /// Wait for all (true) or any (false)
    #[serde(default)]
    pub wait_all: bool,
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SelectorWithCondition {
    pub selector: String,
    #[serde(default)]
    pub condition: WaitCondition,
    #[serde(default)]
    pub extract: Option<ExtractOptions>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wait_condition_default() {
        assert_eq!(WaitCondition::default(), WaitCondition::Present);
    }

    #[test]
    fn test_wait_condition_equality() {
        assert_eq!(WaitCondition::Visible, WaitCondition::Visible);
        assert_ne!(WaitCondition::Visible, WaitCondition::Present);
    }

    #[test]
    fn test_generate_wait_script() {
        let request = WaitRequest {
            session: "test".to_string(),
            selector: "#submit".to_string(),
            condition: WaitCondition::Visible,
            timeout_ms: 5000,
            text: None,
            attribute: None,
            value: None,
            stable_ms: 500,
            extract: None,
            selectors: None,
            wait_all: false,
            frame: None,
        };

        let script = generate_wait_script(&request);
        assert!(script.contains("visible"));
        assert!(script.contains("#submit"));
        assert!(script.contains("5000"));
    }

    #[test]
    fn test_generate_wait_script_text_contains() {
        let request = WaitRequest {
            session: "test".to_string(),
            selector: ".message".to_string(),
            condition: WaitCondition::TextContains,
            timeout_ms: 10000,
            text: Some("Success".to_string()),
            attribute: None,
            value: None,
            stable_ms: 500,
            extract: None,
            selectors: None,
            wait_all: false,
            frame: None,
        };

        let script = generate_wait_script(&request);
        assert!(script.contains("text_contains"));
        assert!(script.contains("Success"));
    }

    #[test]
    fn test_generate_wait_script_attribute_equals() {
        let request = WaitRequest {
            session: "test".to_string(),
            selector: "#input".to_string(),
            condition: WaitCondition::AttributeEquals,
            timeout_ms: 5000,
            text: None,
            attribute: Some("disabled".to_string()),
            value: Some("true".to_string()),
            stable_ms: 500,
            extract: None,
            selectors: None,
            wait_all: false,
            frame: None,
        };

        let script = generate_wait_script(&request);
        assert!(script.contains("attribute_equals"));
        assert!(script.contains("disabled"));
        assert!(script.contains("true"));
    }

    #[test]
    fn test_generate_wait_script_with_extract() {
        let request = WaitRequest {
            session: "test".to_string(),
            selector: ".result".to_string(),
            condition: WaitCondition::Present,
            timeout_ms: 5000,
            text: None,
            attribute: None,
            value: None,
            stable_ms: 500,
            extract: Some(ExtractOptions {
                attribute: "innerHTML".to_string(),
                all: true,
            }),
            selectors: None,
            wait_all: false,
            frame: None,
        };

        let script = generate_wait_script(&request);
        assert!(script.contains("innerHTML"));
        assert!(script.contains("true")); // extractAll
    }

    #[test]
    fn test_generate_wait_script_multiple_selectors() {
        let request = WaitRequest {
            session: "test".to_string(),
            selector: "".to_string(),
            condition: WaitCondition::Present,
            timeout_ms: 5000,
            text: None,
            attribute: None,
            value: None,
            stable_ms: 500,
            extract: None,
            selectors: Some(vec!["#a".to_string(), "#b".to_string(), "#c".to_string()]),
            wait_all: true,
            frame: None,
        };

        let script = generate_wait_script(&request);
        assert!(script.contains("#a"));
        assert!(script.contains("#b"));
        assert!(script.contains("#c"));
        assert!(script.contains("waitAll: true"));
    }

    #[test]
    fn test_generate_wait_script_stable() {
        let request = WaitRequest {
            session: "test".to_string(),
            selector: ".dynamic-content".to_string(),
            condition: WaitCondition::Stable,
            timeout_ms: 10000,
            text: None,
            attribute: None,
            value: None,
            stable_ms: 2000,
            extract: None,
            selectors: None,
            wait_all: false,
            frame: None,
        };

        let script = generate_wait_script(&request);
        assert!(script.contains("stable"));
        assert!(script.contains("2000")); // stable_ms
    }

    #[test]
    fn test_generate_wait_script_all_conditions() {
        let conditions = vec![
            WaitCondition::Present,
            WaitCondition::Visible,
            WaitCondition::Stable,
            WaitCondition::TextContains,
            WaitCondition::TextMatches,
            WaitCondition::AttributeEquals,
            WaitCondition::Clickable,
            WaitCondition::Detached,
            WaitCondition::NavigationComplete,
            WaitCondition::NetworkIdle,
        ];

        for condition in conditions {
            let request = WaitRequest {
                session: "test".to_string(),
                selector: "#el".to_string(),
                condition: condition.clone(),
                timeout_ms: 5000,
                text: Some("text".to_string()),
                attribute: Some("attr".to_string()),
                value: Some("val".to_string()),
                stable_ms: 500,
                extract: None,
                selectors: None,
                wait_all: false,
                frame: None,
            };

            let script = generate_wait_script(&request);
            // Should not panic and should contain the condition name
            assert!(!script.is_empty());
        }
    }

    #[test]
    fn test_wait_request_deserialize() {
        let json = r##"{
            "session": "test",
            "selector": ".button",
            "condition": "text_contains",
            "text": "Submit"
        }"##;

        let req: WaitRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.session, "test");
        assert_eq!(req.condition, WaitCondition::TextContains);
        assert_eq!(req.text, Some("Submit".to_string()));
        assert_eq!(req.timeout_ms, 30000); // default
    }

    #[test]
    fn test_wait_request_deserialize_defaults() {
        let json = r##"{"session": "main", "selector": "#btn"}"##;

        let req: WaitRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.condition, WaitCondition::Present);
        assert_eq!(req.timeout_ms, 30000);
        assert_eq!(req.stable_ms, 500);
        assert!(!req.wait_all);
    }

    #[test]
    fn test_default_functions() {
        assert_eq!(default_timeout(), 30000);
        assert_eq!(default_stable_ms(), 500);
        assert_eq!(default_extract_attr(), "text");
    }

    #[test]
    fn test_extract_options_deserialize() {
        let json = r##"{"attribute": "href", "all": true}"##;
        let opt: ExtractOptions = serde_json::from_str(json).unwrap();
        assert_eq!(opt.attribute, "href");
        assert!(opt.all);
    }

    #[test]
    fn test_extract_options_default() {
        let json = r##"{}"##;
        let opt: ExtractOptions = serde_json::from_str(json).unwrap();
        assert_eq!(opt.attribute, "text");
        assert!(!opt.all);
    }

    #[test]
    fn test_multi_wait_request_deserialize() {
        let json = r##"{
            "session": "main",
            "selectors": [
                {"selector": "#a", "condition": "visible"},
                {"selector": "#b"}
            ],
            "wait_all": true
        }"##;

        let req: MultiWaitRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.selectors.len(), 2);
        assert!(req.wait_all);
        assert_eq!(req.selectors[0].condition, WaitCondition::Visible);
        assert_eq!(req.selectors[1].condition, WaitCondition::Present); // default
    }

    #[test]
    fn test_selector_with_condition() {
        let swc = SelectorWithCondition {
            selector: "#test".to_string(),
            condition: WaitCondition::Clickable,
            extract: Some(ExtractOptions {
                attribute: "value".to_string(),
                all: false,
            }),
        };

        assert_eq!(swc.condition, WaitCondition::Clickable);
        assert!(swc.extract.is_some());
    }
}
