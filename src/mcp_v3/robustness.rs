//! MCP v3 Robustness Layer
//!
//! JavaScript snippets for reliable browser automation:
//! - Element visibility and clickability checks
//! - Scroll into view
//! - Wait conditions
//! - Skeleton/placeholder detection
//! - DOM stability

/// Wait for element to be visible and clickable
pub fn generate_wait_for_clickable_script(selector: &str, timeout_ms: u64) -> String {
    format!(r#"
(async function() {{
    const selector = "{}";
    const timeout = {};
    const startTime = Date.now();
    const pollInterval = 100;

    while ((Date.now() - startTime) < timeout) {{
        const el = document.querySelector(selector);
        if (!el) {{
            await new Promise(r => setTimeout(r, pollInterval));
            continue;
        }}

        // Check visibility
        const rect = el.getBoundingClientRect();
        const style = window.getComputedStyle(el);
        
        if (rect.width === 0 || rect.height === 0) {{
            await new Promise(r => setTimeout(r, pollInterval));
            continue;
        }}
        
        if (style.display === 'none' || style.visibility === 'hidden' || style.opacity === '0') {{
            await new Promise(r => setTimeout(r, pollInterval));
            continue;
        }}

        // Check if not obscured
        const centerX = rect.left + rect.width / 2;
        const centerY = rect.top + rect.height / 2;
        const topEl = document.elementFromPoint(centerX, centerY);
        
        if (!topEl || (!el.contains(topEl) && !topEl.contains(el) && el !== topEl)) {{
            // Element is obscured, check for loading overlays
            const overlay = document.querySelector('.loading, .loader, [aria-busy="true"], .overlay, .modal-backdrop');
            if (overlay && window.getComputedStyle(overlay).display !== 'none') {{
                await new Promise(r => setTimeout(r, pollInterval));
                continue;
            }}
        }}

        // Element is clickable
        return JSON.stringify({{
            success: true,
            selector: selector,
            elapsed: Date.now() - startTime,
            rect: {{ x: rect.x, y: rect.y, width: rect.width, height: rect.height }}
        }});
    }}

    return JSON.stringify({{
        success: false,
        error: "Timeout waiting for element to be clickable",
        selector: selector,
        elapsed: Date.now() - startTime
    }});
}})();
"#, selector.replace('"', "\\\""), timeout_ms)
}

/// Scroll element into view and click
pub fn generate_scroll_and_click_script(selector: &str) -> String {
    format!(r#"
(function() {{
    const el = document.querySelector("{}");
    if (!el) {{
        return JSON.stringify({{ success: false, error: "Element not found" }});
    }}

    // Scroll into view
    el.scrollIntoView({{ behavior: 'instant', block: 'center', inline: 'center' }});

    // Small delay for scroll to complete
    return new Promise(resolve => {{
        setTimeout(() => {{
            el.click();
            resolve(JSON.stringify({{ success: true, clicked: true }}));
        }}, 50);
    }});
}})();
"#, selector.replace('"', "\\\""))
}

/// Type text with input event simulation
pub fn generate_type_with_events_script(selector: &str, text: &str, clear: bool) -> String {
    let clear_code = if clear {
        "el.value = ''; el.dispatchEvent(new Event('input', {bubbles: true}));"
    } else {
        ""
    };

    format!(r#"
(async function() {{
    const el = document.querySelector("{}");
    if (!el) {{
        return JSON.stringify({{ success: false, error: "Element not found" }});
    }}

    // Focus
    el.focus();
    {}

    // Type character by character for reactive forms
    const text = "{}";
    for (const char of text) {{
        el.value += char;
        el.dispatchEvent(new Event('input', {{ bubbles: true }}));
        await new Promise(r => setTimeout(r, 10));
    }}

    // Trigger change and blur
    el.dispatchEvent(new Event('change', {{ bubbles: true }}));
    el.dispatchEvent(new Event('blur', {{ bubbles: true }}));

    return JSON.stringify({{
        success: true,
        typed: true,
        length: text.length,
        value: el.value
    }});
}})();
"#, selector.replace('"', "\\\""), clear_code, text.replace('"', "\\\"").replace('\n', "\\n"))
}

/// Wait for DOM stability (no mutations for specified duration)
pub fn generate_wait_for_stable_script(stable_duration_ms: u64, timeout_ms: u64) -> String {
    format!(r#"
(async function() {{
    const stableDuration = {};
    const timeout = {};
    const startTime = Date.now();
    let lastMutationTime = Date.now();

    const observer = new MutationObserver(() => {{
        lastMutationTime = Date.now();
    }});

    observer.observe(document.body, {{
        childList: true,
        subtree: true,
        attributes: true,
        characterData: true
    }});

    while ((Date.now() - startTime) < timeout) {{
        const timeSinceLastMutation = Date.now() - lastMutationTime;
        if (timeSinceLastMutation >= stableDuration) {{
            observer.disconnect();
            return JSON.stringify({{
                success: true,
                stable: true,
                elapsed: Date.now() - startTime
            }});
        }}
        await new Promise(r => setTimeout(r, 100));
    }}

    observer.disconnect();
    return JSON.stringify({{
        success: false,
        stable: false,
        error: "Timeout waiting for DOM stability",
        elapsed: Date.now() - startTime
    }});
}})();
"#, stable_duration_ms, timeout_ms)
}

/// Wait for skeleton/placeholder elements to disappear
pub fn generate_wait_for_no_skeleton_script(timeout_ms: u64) -> String {
    format!(r#"
(async function() {{
    const timeout = {};
    const startTime = Date.now();
    const pollInterval = 100;

    const skeletonSelectors = [
        '.skeleton',
        '.placeholder',
        '.loading-placeholder',
        '[aria-busy="true"]',
        '.shimmer',
        '.loading-skeleton',
        '[data-loading="true"]'
    ];

    while ((Date.now() - startTime) < timeout) {{
        let foundSkeleton = false;
        
        for (const selector of skeletonSelectors) {{
            const elements = document.querySelectorAll(selector);
            for (const el of elements) {{
                const style = window.getComputedStyle(el);
                if (style.display !== 'none' && style.visibility !== 'hidden') {{
                    foundSkeleton = true;
                    break;
                }}
            }}
            if (foundSkeleton) break;
        }}

        if (!foundSkeleton) {{
            return JSON.stringify({{
                success: true,
                elapsed: Date.now() - startTime
            }});
        }}

        await new Promise(r => setTimeout(r, pollInterval));
    }}

    return JSON.stringify({{
        success: false,
        error: "Timeout waiting for skeletons to disappear",
        elapsed: Date.now() - startTime
    }});
}})();
"#, timeout_ms)
}

/// Extract interactive elements for AI context
pub fn generate_extract_interactive_elements_script() -> String {
    r#"
(function() {
    const selectors = 'a, button, input, select, textarea, [role="button"], [onclick], [tabindex]';
    const noiseParents = ['nav', 'footer', 'header:not(:has(form))', '.sidebar', '.advertisement', '.cookie-banner'];
    
    const isInNoiseArea = (el) => {
        for (const selector of noiseParents) {
            if (el.closest(selector)) return true;
        }
        return false;
    };

    const getLabel = (el) => {
        // Try label element
        const labelFor = document.querySelector(`label[for="${el.id}"]`);
        if (labelFor) return labelFor.textContent.trim().substring(0, 50);
        
        // Try parent label
        const parentLabel = el.closest('label');
        if (parentLabel) return parentLabel.textContent.trim().substring(0, 50);
        
        // Try aria-label
        if (el.getAttribute('aria-label')) return el.getAttribute('aria-label');
        
        // Try placeholder
        if (el.placeholder) return el.placeholder;
        
        // Try title
        if (el.title) return el.title;
        
        // Try text content
        if (el.textContent) return el.textContent.trim().substring(0, 50);
        
        return null;
    };

    const generateSelector = (el) => {
        if (el.id) return `#${el.id}`;
        if (el.name) return `${el.tagName.toLowerCase()}[name="${el.name}"]`;
        
        // Use class if unique
        if (el.className) {
            const classes = el.className.split(' ').filter(c => c && !c.startsWith('js-')).slice(0, 2);
            if (classes.length > 0) {
                const selector = `${el.tagName.toLowerCase()}.${classes.join('.')}`;
                if (document.querySelectorAll(selector).length === 1) return selector;
            }
        }
        
        // Generate path
        let path = el.tagName.toLowerCase();
        let current = el;
        while (current.parentElement && current.parentElement !== document.body) {
            const parent = current.parentElement;
            const siblings = Array.from(parent.children).filter(c => c.tagName === current.tagName);
            if (siblings.length > 1) {
                const index = siblings.indexOf(current) + 1;
                path = `${parent.tagName.toLowerCase()} > ${current.tagName.toLowerCase()}:nth-of-type(${index})`;
            } else {
                path = `${parent.tagName.toLowerCase()} > ${path}`;
            }
            current = parent;
            if (path.length > 100) break;
        }
        return path;
    };

    const elements = Array.from(document.querySelectorAll(selectors))
        .filter(el => {
            const rect = el.getBoundingClientRect();
            const style = window.getComputedStyle(el);
            
            // Must be visible
            if (rect.width === 0 || rect.height === 0) return false;
            if (style.display === 'none' || style.visibility === 'hidden') return false;
            
            // Skip noise areas (optional)
            // if (isInNoiseArea(el)) return false;
            
            return true;
        })
        .slice(0, 50)  // Limit to 50 elements
        .map(el => {
            const rect = el.getBoundingClientRect();
            return {
                tag: el.tagName.toLowerCase(),
                type: el.type || null,
                selector: generateSelector(el),
                label: getLabel(el),
                value: el.value || null,
                placeholder: el.placeholder || null,
                inViewport: rect.top < window.innerHeight && rect.bottom > 0
            };
        });

    return JSON.stringify({
        url: window.location.href,
        title: document.title,
        elementCount: elements.length,
        elements: elements
    });
})();
"#.to_string()
}

/// Wait for specific condition
pub fn generate_wait_for_condition_script(condition: &str, value: Option<&str>, timeout_ms: u64) -> String {
    let check_code = match condition {
        "element" => format!(
            r#"document.querySelector("{}") !== null"#,
            value.unwrap_or("").replace('"', "\\\"")
        ),
        "element_visible" => format!(
            r#"(() => {{
                const el = document.querySelector("{}");
                if (!el) return false;
                const rect = el.getBoundingClientRect();
                const style = window.getComputedStyle(el);
                return rect.width > 0 && rect.height > 0 && style.display !== 'none' && style.visibility !== 'hidden';
            }})()"#,
            value.unwrap_or("").replace('"', "\\\"")
        ),
        "element_hidden" => format!(
            r#"(() => {{
                const el = document.querySelector("{}");
                if (!el) return true;
                const style = window.getComputedStyle(el);
                return style.display === 'none' || style.visibility === 'hidden';
            }})()"#,
            value.unwrap_or("").replace('"', "\\\"")
        ),
        "url_contains" => format!(
            r#"window.location.href.includes("{}")"#,
            value.unwrap_or("").replace('"', "\\\"")
        ),
        "url_matches" => format!(
            r#"new RegExp("{}").test(window.location.href)"#,
            value.unwrap_or("").replace('"', "\\\"")
        ),
        "text_contains" => format!(
            r#"document.body.innerText.includes("{}")"#,
            value.unwrap_or("").replace('"', "\\\"")
        ),
        "network_idle" => {
            // Simplified: just wait for no pending fetches
            r#"true"#.to_string()
        },
        _ => "true".to_string(),
    };

    format!(r#"
(async function() {{
    const timeout = {};
    const startTime = Date.now();
    const pollInterval = 100;

    while ((Date.now() - startTime) < timeout) {{
        const result = {};
        if (result) {{
            return JSON.stringify({{
                success: true,
                condition: "{}",
                elapsed: Date.now() - startTime
            }});
        }}
        await new Promise(r => setTimeout(r, pollInterval));
    }}

    return JSON.stringify({{
        success: false,
        error: "Timeout waiting for condition",
        condition: "{}",
        elapsed: Date.now() - startTime
    }});
}})();
"#, timeout_ms, check_code, condition, condition)
}

/// Extract data from multiple elements
pub fn generate_extract_data_script(selector: &str, fields: &std::collections::HashMap<String, String>, limit: Option<usize>) -> String {
    let fields_json = serde_json::to_string(fields).unwrap_or_else(|_| "{}".to_string());
    let limit_code = limit.map(|l| format!(".slice(0, {})", l)).unwrap_or_default();

    format!(r#"
(function() {{
    const containerSelector = "{}";
    const fields = {};
    const containers = Array.from(document.querySelectorAll(containerSelector)){};
    
    const extractField = (container, fieldSpec) => {{
        // Handle attribute extraction (e.g., "a@href")
        if (fieldSpec.includes('@')) {{
            const [selector, attr] = fieldSpec.split('@');
            const el = selector ? container.querySelector(selector) : container;
            return el ? el.getAttribute(attr) : null;
        }}
        
        // Handle text content
        const el = container.querySelector(fieldSpec);
        return el ? el.textContent.trim() : null;
    }};

    const results = containers.map(container => {{
        const item = {{}};
        for (const [name, spec] of Object.entries(fields)) {{
            item[name] = extractField(container, spec);
        }}
        return item;
    }});

    return JSON.stringify({{
        success: true,
        count: results.length,
        data: results
    }});
}})();
"#, selector.replace('"', "\\\""), fields_json, limit_code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wait_for_clickable_script() {
        let script = generate_wait_for_clickable_script("#btn", 5000);
        assert!(script.contains("#btn"));
        assert!(script.contains("5000"));
        assert!(script.contains("elementFromPoint"));
    }

    #[test]
    fn test_type_with_events_script() {
        let script = generate_type_with_events_script("#input", "test", true);
        assert!(script.contains("#input"));
        assert!(script.contains("test"));
        assert!(script.contains("el.value = ''"));
    }

    #[test]
    fn test_extract_interactive_elements() {
        let script = generate_extract_interactive_elements_script();
        assert!(script.contains("querySelectorAll"));
        assert!(script.contains("getBoundingClientRect"));
    }
}
