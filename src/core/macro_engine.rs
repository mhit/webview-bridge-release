//! WBP2 Macro Engine Module
//!
//! JavaScript macro execution and preset macros for SPA automation.
//! See: Plans.md Phase 11

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// Macro Types
// ============================================================================

/// Macro definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MacroDefinition {
    /// Unique macro name
    pub name: String,
    /// Description
    pub description: String,
    /// JavaScript code to execute
    pub script: String,
    /// Required parameters
    #[serde(default)]
    pub required_params: Vec<String>,
    /// Optional parameters with defaults
    #[serde(default)]
    pub optional_params: HashMap<String, serde_json::Value>,
    /// Timeout in ms (default: 30000)
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
    /// Whether this is a built-in macro
    #[serde(default)]
    pub builtin: bool,
}

fn default_timeout() -> u64 {
    30000
}

/// Request for POST /v2/macro
#[derive(Debug, Clone, Deserialize)]
pub struct MacroExecuteRequest {
    /// Session name
    pub session: String,
    /// Macro name
    pub name: String,
    /// Parameters
    #[serde(default)]
    pub params: HashMap<String, serde_json::Value>,
    /// Override timeout
    #[serde(default)]
    pub timeout_ms: Option<u64>,
}

/// Response for POST /v2/macro
#[derive(Debug, Clone, Serialize)]
pub struct MacroExecuteResponse {
    pub success: bool,
    pub macro_name: String,
    /// Result data
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
    /// Time taken in ms
    pub elapsed_ms: u64,
    /// Error message if failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Request for POST /v2/macro/register
#[derive(Debug, Clone, Deserialize)]
pub struct MacroRegisterRequest {
    /// Macro definition
    pub macro_def: MacroDefinition,
    /// Overwrite if exists
    #[serde(default)]
    pub overwrite: bool,
}

// ============================================================================
// SPA Detection
// ============================================================================

/// Detected SPA framework
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SpaFramework {
    React,
    Vue,
    Angular,
    Svelte,
    NextJs,
    Nuxt,
    Unknown,
    None,
}

/// SPA detection result
#[derive(Debug, Clone, Serialize)]
pub struct SpaDetectionResult {
    pub framework: SpaFramework,
    pub version: Option<String>,
    pub hints: Vec<String>,
}

/// Generate JavaScript for SPA framework detection
pub fn generate_spa_detection_script() -> String {
    r#"
(function() {
    const result = {
        framework: "none",
        version: null,
        hints: []
    };
    
    // Check for React
    if (window.__REACT_DEVTOOLS_GLOBAL_HOOK__ || 
        document.querySelector('[data-reactroot]') ||
        document.querySelector('[data-reactid]')) {
        result.framework = "react";
        result.hints.push("React DevTools hook or data attributes detected");
        
        // Check for Next.js
        if (window.__NEXT_DATA__ || document.querySelector('#__next')) {
            result.framework = "nextjs";
            result.hints.push("Next.js detected");
            if (window.__NEXT_DATA__) {
                result.version = window.__NEXT_DATA__.buildId;
            }
        }
    }
    
    // Check for Vue
    if (window.__VUE__ || 
        document.querySelector('[data-v-]') ||
        window.Vue) {
        if (result.framework === "none") {
            result.framework = "vue";
            result.hints.push("Vue detected");
        }
        
        // Check for Nuxt
        if (window.__NUXT__ || document.querySelector('#__nuxt')) {
            result.framework = "nuxt";
            result.hints.push("Nuxt detected");
        }
    }
    
    // Check for Angular
    if (window.ng || 
        document.querySelector('[ng-version]') ||
        document.querySelector('app-root')) {
        if (result.framework === "none") {
            result.framework = "angular";
            const versionEl = document.querySelector('[ng-version]');
            if (versionEl) {
                result.version = versionEl.getAttribute('ng-version');
            }
            result.hints.push("Angular detected");
        }
    }
    
    // Check for Svelte
    if (document.querySelector('[class*="svelte-"]')) {
        if (result.framework === "none") {
            result.framework = "svelte";
            result.hints.push("Svelte class names detected");
        }
    }
    
    if (result.framework === "none" && result.hints.length === 0) {
        result.framework = "unknown";
        result.hints.push("No known SPA framework detected");
    }
    
    return JSON.stringify(result);
})();
"#.to_string()
}

// ============================================================================
// Helper Script Generation
// ============================================================================

/// Generate waitFor helper function script
pub fn generate_wait_for_helper() -> String {
    r#"
window.__wbp2_waitFor = function(selector, options = {}) {
    const timeout = options.timeout || 30000;
    const interval = options.interval || 100;
    const visible = options.visible !== false;
    
    return new Promise((resolve, reject) => {
        const startTime = Date.now();
        
        function check() {
            const el = document.querySelector(selector);
            
            if (el) {
                if (!visible || (el.offsetWidth > 0 && el.offsetHeight > 0)) {
                    resolve(el);
                    return;
                }
            }
            
            if (Date.now() - startTime > timeout) {
                reject(new Error(`Timeout waiting for: ${selector}`));
                return;
            }
            
            setTimeout(check, interval);
        }
        
        check();
    });
};
"#.to_string()
}

/// Generate waitForNavigation helper function script
pub fn generate_wait_for_navigation_helper() -> String {
    r#"
window.__wbp2_waitForNavigation = function(options = {}) {
    const timeout = options.timeout || 30000;
    
    return new Promise((resolve, reject) => {
        const startUrl = window.location.href;
        const startTime = Date.now();
        
        function check() {
            if (window.location.href !== startUrl) {
                // Wait a bit for page to stabilize
                setTimeout(() => {
                    resolve({
                        from: startUrl,
                        to: window.location.href
                    });
                }, 100);
                return;
            }
            
            if (Date.now() - startTime > timeout) {
                reject(new Error('Navigation timeout'));
                return;
            }
            
            setTimeout(check, 100);
        }
        
        check();
    });
};
"#.to_string()
}

/// Generate waitForNetworkIdle helper function script
pub fn generate_wait_for_network_idle_helper() -> String {
    r#"
window.__wbp2_waitForNetworkIdle = function(options = {}) {
    const timeout = options.timeout || 30000;
    const idleTime = options.idleTime || 500;
    
    return new Promise((resolve, reject) => {
        let pending = 0;
        let lastActivity = Date.now();
        const startTime = Date.now();
        
        // Track fetch/XHR
        const originalFetch = window.fetch;
        const originalXHROpen = XMLHttpRequest.prototype.open;
        const originalXHRSend = XMLHttpRequest.prototype.send;
        
        window.fetch = function(...args) {
            pending++;
            lastActivity = Date.now();
            return originalFetch.apply(this, args).finally(() => {
                pending--;
                lastActivity = Date.now();
            });
        };
        
        XMLHttpRequest.prototype.open = function(...args) {
            this.__wbp2_tracked = true;
            return originalXHROpen.apply(this, args);
        };
        
        XMLHttpRequest.prototype.send = function(...args) {
            if (this.__wbp2_tracked) {
                pending++;
                lastActivity = Date.now();
                this.addEventListener('loadend', () => {
                    pending--;
                    lastActivity = Date.now();
                });
            }
            return originalXHRSend.apply(this, args);
        };
        
        function cleanup() {
            window.fetch = originalFetch;
            XMLHttpRequest.prototype.open = originalXHROpen;
            XMLHttpRequest.prototype.send = originalXHRSend;
        }
        
        function check() {
            if (pending === 0 && Date.now() - lastActivity > idleTime) {
                cleanup();
                resolve({ idleTime: Date.now() - startTime });
                return;
            }
            
            if (Date.now() - startTime > timeout) {
                cleanup();
                reject(new Error('Network idle timeout'));
                return;
            }
            
            setTimeout(check, 100);
        }
        
        check();
    });
};
"#.to_string()
}

/// Generate waitForDomStable helper function script
pub fn generate_wait_for_dom_stable_helper() -> String {
    r#"
window.__wbp2_waitForDomStable = function(options = {}) {
    const timeout = options.timeout || 30000;
    const stableTime = options.stableTime || 500;
    const target = options.target || document.body;
    
    return new Promise((resolve, reject) => {
        const startTime = Date.now();
        let lastMutation = Date.now();
        
        const observer = new MutationObserver(() => {
            lastMutation = Date.now();
        });
        
        observer.observe(target, {
            childList: true,
            subtree: true,
            attributes: true,
            characterData: true
        });
        
        function check() {
            if (Date.now() - lastMutation > stableTime) {
                observer.disconnect();
                resolve({ stableAfter: Date.now() - startTime });
                return;
            }
            
            if (Date.now() - startTime > timeout) {
                observer.disconnect();
                reject(new Error('DOM stability timeout'));
                return;
            }
            
            setTimeout(check, 100);
        }
        
        check();
    });
};
"#.to_string()
}

/// Generate all helper functions
pub fn generate_all_helpers() -> String {
    format!(
        "{}{}{}{}",
        generate_wait_for_helper(),
        generate_wait_for_navigation_helper(),
        generate_wait_for_network_idle_helper(),
        generate_wait_for_dom_stable_helper()
    )
}

// ============================================================================
// Preset Macros
// ============================================================================

/// Get built-in preset macros
pub fn get_preset_macros() -> Vec<MacroDefinition> {
    vec![
        // Extract list macro
        MacroDefinition {
            name: "extract_list".to_string(),
            description: "Extract list of items matching selector with optional transform".to_string(),
            script: r#"
(async function(params) {
    await __wbp2_waitFor(params.container || 'body');
    
    const items = Array.from(document.querySelectorAll(params.selector));
    let results = items.map((el, i) => {
        const item = { _index: i };
        
        if (params.fields) {
            for (const [key, fieldSelector] of Object.entries(params.fields)) {
                const fieldEl = el.querySelector(fieldSelector);
                item[key] = fieldEl ? fieldEl.textContent.trim() : null;
            }
        } else {
            item.text = el.textContent.trim();
            item.html = el.innerHTML;
        }
        
        return item;
    });
    
    // Apply filter if provided
    if (params.filter) {
        const filterFn = new Function('item', `return ${params.filter}`);
        results = results.filter(filterFn);
    }
    
    // Apply limit
    if (params.limit) {
        results = results.slice(0, params.limit);
    }
    
    return JSON.stringify({ items: results, count: results.length });
})
"#.to_string(),
            required_params: vec!["selector".to_string()],
            optional_params: [
                ("container".to_string(), serde_json::json!("body")),
                ("fields".to_string(), serde_json::json!(null)),
                ("filter".to_string(), serde_json::json!(null)),
                ("limit".to_string(), serde_json::json!(null)),
            ].into_iter().collect(),
            timeout_ms: 30000,
            builtin: true,
        },
        
        // Paginated extract macro
        MacroDefinition {
            name: "paginated_extract".to_string(),
            description: "Extract items across multiple pages".to_string(),
            script: r#"
(async function(params) {
    const allItems = [];
    let page = 1;
    const maxPages = params.max_pages || 10;
    
    while (page <= maxPages) {
        await __wbp2_waitFor(params.selector);
        await __wbp2_waitForDomStable({ stableTime: 300 });
        
        const items = Array.from(document.querySelectorAll(params.selector));
        items.forEach((el, i) => {
            const item = { _page: page, _index: i };
            if (params.fields) {
                for (const [key, fieldSelector] of Object.entries(params.fields)) {
                    const fieldEl = el.querySelector(fieldSelector);
                    item[key] = fieldEl ? fieldEl.textContent.trim() : null;
                }
            } else {
                item.text = el.textContent.trim();
            }
            allItems.push(item);
        });
        
        // Check for next button
        const nextBtn = document.querySelector(params.next_selector);
        if (!nextBtn || nextBtn.disabled) {
            break;
        }
        
        nextBtn.click();
        await __wbp2_waitForDomStable({ stableTime: 500 });
        page++;
    }
    
    return JSON.stringify({ items: allItems, pages: page, count: allItems.length });
})
"#.to_string(),
            required_params: vec!["selector".to_string(), "next_selector".to_string()],
            optional_params: [
                ("fields".to_string(), serde_json::json!(null)),
                ("max_pages".to_string(), serde_json::json!(10)),
            ].into_iter().collect(),
            timeout_ms: 120000,
            builtin: true,
        },
        
        // Wait for SPA macro
        MacroDefinition {
            name: "wait_for_spa".to_string(),
            description: "Wait for SPA to fully load and stabilize".to_string(),
            script: r#"
(async function(params) {
    const startTime = Date.now();
    
    // Wait for initial DOM
    await __wbp2_waitFor(params.selector || 'body');
    
    // Wait for network to be idle
    try {
        await __wbp2_waitForNetworkIdle({ 
            timeout: params.network_timeout || 10000,
            idleTime: params.idle_time || 500
        });
    } catch (e) {
        // Network timeout is acceptable for some SPAs
    }
    
    // Wait for DOM stability
    await __wbp2_waitForDomStable({
        stableTime: params.stable_time || 500
    });
    
    // Extract if selector provided
    let data = null;
    if (params.extract_selector) {
        const els = document.querySelectorAll(params.extract_selector);
        data = Array.from(els).map(el => el.textContent.trim());
    }
    
    return JSON.stringify({
        ready: true,
        elapsed_ms: Date.now() - startTime,
        data: data
    });
})
"#.to_string(),
            required_params: vec![],
            optional_params: [
                ("selector".to_string(), serde_json::json!("body")),
                ("network_timeout".to_string(), serde_json::json!(10000)),
                ("idle_time".to_string(), serde_json::json!(500)),
                ("stable_time".to_string(), serde_json::json!(500)),
                ("extract_selector".to_string(), serde_json::json!(null)),
            ].into_iter().collect(),
            timeout_ms: 60000,
            builtin: true,
        },
        
        // Form fill macro
        MacroDefinition {
            name: "form_fill".to_string(),
            description: "Fill form fields and optionally submit".to_string(),
            script: r#"
(async function(params) {
    const form = await __wbp2_waitFor(params.form_selector || 'form');
    const filled = [];
    
    for (const [selector, value] of Object.entries(params.fields)) {
        const input = form.querySelector(selector);
        if (input) {
            input.value = value;
            input.dispatchEvent(new Event('input', { bubbles: true }));
            input.dispatchEvent(new Event('change', { bubbles: true }));
            filled.push(selector);
        }
    }
    
    if (params.submit) {
        const submitBtn = form.querySelector(params.submit_selector || 'button[type="submit"], input[type="submit"]');
        if (submitBtn) {
            submitBtn.click();
            await __wbp2_waitForDomStable({ stableTime: 500 });
        } else {
            form.submit();
        }
    }
    
    return JSON.stringify({
        filled: filled,
        submitted: !!params.submit
    });
})
"#.to_string(),
            required_params: vec!["fields".to_string()],
            optional_params: [
                ("form_selector".to_string(), serde_json::json!("form")),
                ("submit".to_string(), serde_json::json!(false)),
                ("submit_selector".to_string(), serde_json::json!(null)),
            ].into_iter().collect(),
            timeout_ms: 30000,
            builtin: true,
        },
    ]
}

/// Find a preset macro by name
pub fn find_preset_macro(name: &str) -> Option<MacroDefinition> {
    get_preset_macros()
        .into_iter()
        .find(|m| m.name.eq_ignore_ascii_case(name))
}

// ============================================================================
// Macro Registry
// ============================================================================

/// Registry for macros
#[derive(Debug, Clone, Default)]
pub struct MacroRegistry {
    pub macros: HashMap<String, MacroDefinition>,
}

impl MacroRegistry {
    pub fn new() -> Self {
        Self {
            macros: HashMap::new(),
        }
    }
    
    /// Register a custom macro
    pub fn register(&mut self, macro_def: MacroDefinition) -> Result<(), String> {
        if macro_def.name.is_empty() {
            return Err("Macro name cannot be empty".to_string());
        }
        
        // Check if it's a builtin
        if let Some(existing) = self.macros.get(&macro_def.name) {
            if existing.builtin {
                return Err(format!("Cannot overwrite builtin macro: {}", macro_def.name));
            }
        }
        
        self.macros.insert(macro_def.name.clone(), macro_def);
        Ok(())
    }
    
    /// Get a macro by name (custom first, then preset)
    pub fn get(&self, name: &str) -> Option<MacroDefinition> {
        self.macros.get(name).cloned()
            .or_else(|| find_preset_macro(name))
    }
    
    /// Delete a custom macro
    pub fn delete(&mut self, name: &str) -> Result<(), String> {
        if let Some(existing) = self.macros.get(name) {
            if existing.builtin {
                return Err(format!("Cannot delete builtin macro: {}", name));
            }
        }
        
        self.macros.remove(name);
        Ok(())
    }
    
    /// List all available macros
    pub fn list(&self) -> Vec<MacroDefinition> {
        let mut result: Vec<MacroDefinition> = self.macros.values().cloned().collect();
        
        for preset in get_preset_macros() {
            if !self.macros.contains_key(&preset.name) {
                result.push(preset);
            }
        }
        
        result.sort_by(|a, b| a.name.cmp(&b.name));
        result
    }
}

/// Generate executable script for a macro with parameters
pub fn generate_macro_script(macro_def: &MacroDefinition, params: &HashMap<String, serde_json::Value>) -> String {
    let params_json = serde_json::to_string(params).unwrap_or_else(|_| "{}".to_string());
    
    format!(
        "{}\n({})({});",
        generate_all_helpers(),
        macro_def.script,
        params_json
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_spa_detection() {
        let script = generate_spa_detection_script();
        assert!(script.contains("React"));
        assert!(script.contains("Vue"));
        assert!(script.contains("Angular"));
        assert!(script.contains("Svelte"));
        assert!(script.contains("Next.js"));
        assert!(script.contains("Nuxt"));
    }
    
    #[test]
    fn test_preset_macros() {
        let macros = get_preset_macros();
        assert_eq!(macros.len(), 4);
        
        let extract = find_preset_macro("extract_list");
        assert!(extract.is_some());
        assert!(extract.unwrap().builtin);
        
        let paginated = find_preset_macro("paginated_extract");
        assert!(paginated.is_some());
        
        let wait_spa = find_preset_macro("wait_for_spa");
        assert!(wait_spa.is_some());
        
        let form_fill = find_preset_macro("form_fill");
        assert!(form_fill.is_some());
        
        let nonexistent = find_preset_macro("nonexistent");
        assert!(nonexistent.is_none());
    }
    
    #[test]
    fn test_macro_registry() {
        let mut registry = MacroRegistry::new();
        
        let custom = MacroDefinition {
            name: "my_macro".to_string(),
            description: "Custom macro".to_string(),
            script: "console.log('hello')".to_string(),
            required_params: vec![],
            optional_params: HashMap::new(),
            timeout_ms: 5000,
            builtin: false,
        };
        
        registry.register(custom).unwrap();
        
        assert!(registry.get("my_macro").is_some());
        assert!(registry.get("extract_list").is_some()); // Fallback to preset
        
        registry.delete("my_macro").unwrap();
        assert!(registry.get("my_macro").is_none());
    }
    
    #[test]
    fn test_macro_registry_empty_name() {
        let mut registry = MacroRegistry::new();
        
        let custom = MacroDefinition {
            name: "".to_string(),
            description: "Empty name".to_string(),
            script: "".to_string(),
            required_params: vec![],
            optional_params: HashMap::new(),
            timeout_ms: 5000,
            builtin: false,
        };
        
        assert!(registry.register(custom).is_err());
    }
    
    #[test]
    fn test_macro_registry_cannot_overwrite_builtin() {
        let mut registry = MacroRegistry::new();
        
        // Register builtin first
        let builtin = MacroDefinition {
            name: "test_builtin".to_string(),
            description: "Builtin".to_string(),
            script: "".to_string(),
            required_params: vec![],
            optional_params: HashMap::new(),
            timeout_ms: 5000,
            builtin: true,
        };
        registry.macros.insert("test_builtin".to_string(), builtin);
        
        // Try to overwrite
        let custom = MacroDefinition {
            name: "test_builtin".to_string(),
            description: "Custom".to_string(),
            script: "".to_string(),
            required_params: vec![],
            optional_params: HashMap::new(),
            timeout_ms: 5000,
            builtin: false,
        };
        
        assert!(registry.register(custom).is_err());
    }
    
    #[test]
    fn test_macro_registry_cannot_delete_builtin() {
        let mut registry = MacroRegistry::new();
        
        let builtin = MacroDefinition {
            name: "test_builtin".to_string(),
            description: "Builtin".to_string(),
            script: "".to_string(),
            required_params: vec![],
            optional_params: HashMap::new(),
            timeout_ms: 5000,
            builtin: true,
        };
        registry.macros.insert("test_builtin".to_string(), builtin);
        
        assert!(registry.delete("test_builtin").is_err());
    }
    
    #[test]
    fn test_macro_registry_delete_nonexistent() {
        let mut registry = MacroRegistry::new();
        // Should succeed (no-op)
        assert!(registry.delete("nonexistent").is_ok());
    }
    
    #[test]
    fn test_macro_registry_list() {
        let mut registry = MacroRegistry::new();
        
        let custom = MacroDefinition {
            name: "aaa_first".to_string(),
            description: "First".to_string(),
            script: "".to_string(),
            required_params: vec![],
            optional_params: HashMap::new(),
            timeout_ms: 5000,
            builtin: false,
        };
        registry.register(custom).unwrap();
        
        let list = registry.list();
        // Should include custom and presets
        assert!(list.len() >= 5);
        // Should be sorted
        assert_eq!(list[0].name, "aaa_first");
    }
    
    #[test]
    fn test_generate_macro_script() {
        let macro_def = find_preset_macro("extract_list").unwrap();
        let params: HashMap<String, serde_json::Value> = [
            ("selector".to_string(), serde_json::json!(".item")),
        ].into_iter().collect();
        
        let script = generate_macro_script(&macro_def, &params);
        assert!(script.contains("__wbp2_waitFor"));
        assert!(script.contains(".item"));
    }
    
    #[test]
    fn test_generate_wait_for_helper() {
        let script = generate_wait_for_helper();
        assert!(script.contains("__wbp2_waitFor"));
        assert!(script.contains("Promise"));
        assert!(script.contains("querySelector"));
    }
    
    #[test]
    fn test_generate_wait_for_navigation_helper() {
        let script = generate_wait_for_navigation_helper();
        assert!(script.contains("__wbp2_waitForNavigation"));
        assert!(script.contains("location.href"));
    }
    
    #[test]
    fn test_generate_wait_for_network_idle_helper() {
        let script = generate_wait_for_network_idle_helper();
        assert!(script.contains("__wbp2_waitForNetworkIdle"));
        assert!(script.contains("fetch"));
        assert!(script.contains("XMLHttpRequest"));
    }
    
    #[test]
    fn test_generate_wait_for_dom_stable_helper() {
        let script = generate_wait_for_dom_stable_helper();
        assert!(script.contains("__wbp2_waitForDomStable"));
        assert!(script.contains("MutationObserver"));
    }
    
    #[test]
    fn test_generate_all_helpers() {
        let script = generate_all_helpers();
        assert!(script.contains("__wbp2_waitFor"));
        assert!(script.contains("__wbp2_waitForNavigation"));
        assert!(script.contains("__wbp2_waitForNetworkIdle"));
        assert!(script.contains("__wbp2_waitForDomStable"));
    }
    
    #[test]
    fn test_spa_framework_equality() {
        assert_eq!(SpaFramework::React, SpaFramework::React);
        assert_ne!(SpaFramework::React, SpaFramework::Vue);
    }
    
    #[test]
    fn test_default_timeout() {
        assert_eq!(default_timeout(), 30000);
    }
    
    #[test]
    fn test_macro_definition_deserialize() {
        let json = r##"{
            "name": "test",
            "description": "Test macro",
            "script": "console.log('test')"
        }"##;
        
        let def: MacroDefinition = serde_json::from_str(json).unwrap();
        assert_eq!(def.name, "test");
        assert_eq!(def.timeout_ms, 30000); // default
        assert!(!def.builtin);
    }
    
    #[test]
    fn test_macro_execute_request_deserialize() {
        let json = r##"{
            "session": "main",
            "name": "extract_list"
        }"##;
        
        let req: MacroExecuteRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.session, "main");
        assert_eq!(req.name, "extract_list");
        assert!(req.timeout_ms.is_none());
    }
    
    #[test]
    fn test_preset_macro_required_params() {
        let extract = find_preset_macro("extract_list").unwrap();
        assert!(extract.required_params.contains(&"selector".to_string()));
        
        let paginated = find_preset_macro("paginated_extract").unwrap();
        assert!(paginated.required_params.contains(&"selector".to_string()));
        assert!(paginated.required_params.contains(&"next_selector".to_string()));
        
        let form = find_preset_macro("form_fill").unwrap();
        assert!(form.required_params.contains(&"fields".to_string()));
    }
    
    #[test]
    fn test_preset_macro_optional_params() {
        let extract = find_preset_macro("extract_list").unwrap();
        assert!(extract.optional_params.contains_key("container"));
        assert!(extract.optional_params.contains_key("limit"));
    }
    
    #[test]
    fn test_macro_script_generation_empty_params() {
        let macro_def = find_preset_macro("wait_for_spa").unwrap();
        let params: HashMap<String, serde_json::Value> = HashMap::new();
        
        let script = generate_macro_script(&macro_def, &params);
        assert!(script.contains("__wbp2_waitFor"));
        assert!(script.contains("{}"));
    }
}

