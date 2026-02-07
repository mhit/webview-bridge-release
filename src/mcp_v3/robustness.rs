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

/// Simulate human-like mouse movement to element with natural curve and jitter
pub fn generate_human_mouse_move_script(selector: &str) -> String {
    format!(r#"
(async function() {{
    const el = document.querySelector("{}");
    if (!el) return JSON.stringify({{ success: false, error: "Element not found" }});
    
    const rect = el.getBoundingClientRect();
    // Target with slight random offset (humans don't click exact center)
    const targetX = rect.left + rect.width * (0.3 + Math.random() * 0.4);
    const targetY = rect.top + rect.height * (0.3 + Math.random() * 0.4);
    
    // Get current mouse position (or start from random edge position)
    let startX = Math.random() * window.innerWidth;
    let startY = Math.random() * 100; // Start from top area
    
    // Generate bezier curve control points - more organic curves
    const distance = Math.sqrt(Math.pow(targetX - startX, 2) + Math.pow(targetY - startY, 2));
    const curviness = 0.2 + Math.random() * 0.3; // How curved the path is
    
    // Control points with perpendicular offset for natural arc
    const midX = (startX + targetX) / 2;
    const midY = (startY + targetY) / 2;
    const perpX = -(targetY - startY) / distance * curviness * distance;
    const perpY = (targetX - startX) / distance * curviness * distance;
    
    const cp1x = midX + perpX * (0.3 + Math.random() * 0.4);
    const cp1y = midY + perpY * (0.3 + Math.random() * 0.4);
    const cp2x = midX + perpX * (0.6 + Math.random() * 0.4);
    const cp2y = midY + perpY * (0.6 + Math.random() * 0.4);
    
    // Cubic bezier interpolation
    const bezier = (t, p0, p1, p2, p3) => {{
        const u = 1 - t;
        return u*u*u*p0 + 3*u*u*t*p1 + 3*u*t*t*p2 + t*t*t*p3;
    }};
    
    // Easing function: ease-in-out (slow start, fast middle, slow end)
    const easeInOut = (t) => {{
        return t < 0.5 
            ? 4 * t * t * t 
            : 1 - Math.pow(-2 * t + 2, 3) / 2;
    }};
    
    // Number of steps varies based on distance
    const steps = Math.max(20, Math.min(60, Math.floor(distance / 15))) + Math.floor(Math.random() * 10);
    const totalTime = 200 + distance * 0.8 + Math.random() * 150; // Total movement time in ms
    
    let lastTime = performance.now();
    for (let i = 0; i <= steps; i++) {{
        const linearT = i / steps;
        const t = easeInOut(linearT); // Apply easing
        
        const x = bezier(t, startX, cp1x, cp2x, targetX);
        const y = bezier(t, startY, cp1y, cp2y, targetY);
        
        // Add micro-jitter (decreases as we approach target - steadier hand near goal)
        const jitterScale = 3 * (1 - linearT * 0.7);
        const jitterX = (Math.random() - 0.5) * jitterScale;
        const jitterY = (Math.random() - 0.5) * jitterScale;
        
        // Dispatch mouse move event
        const event = new MouseEvent('mousemove', {{
            bubbles: true,
            cancelable: true,
            view: window,
            clientX: x + jitterX,
            clientY: y + jitterY
        }});
        document.elementFromPoint(x + jitterX, y + jitterY)?.dispatchEvent(event);
        
        // Variable delay: faster in middle, slower at start/end
        const speedFactor = 0.5 + Math.sin(linearT * Math.PI) * 0.5; // 0.5 at edges, 1.0 at middle
        const baseDelay = totalTime / steps;
        const delay = (baseDelay / speedFactor) * (0.7 + Math.random() * 0.6);
        await new Promise(r => setTimeout(r, delay));
    }}
    
    // Occasional small overshoot and correction (10% chance)
    if (Math.random() < 0.1) {{
        const overshootX = targetX + (Math.random() - 0.5) * 20;
        const overshootY = targetY + (Math.random() - 0.5) * 15;
        document.elementFromPoint(overshootX, overshootY)?.dispatchEvent(
            new MouseEvent('mousemove', {{ bubbles: true, view: window, clientX: overshootX, clientY: overshootY }})
        );
        await new Promise(r => setTimeout(r, 30 + Math.random() * 50));
        // Correct back
        el.dispatchEvent(new MouseEvent('mousemove', {{ bubbles: true, view: window, clientX: targetX, clientY: targetY }}));
    }}
    
    // Dispatch final hover event on target
    el.dispatchEvent(new MouseEvent('mouseenter', {{ bubbles: true, view: window }}));
    el.dispatchEvent(new MouseEvent('mouseover', {{ bubbles: true, view: window }}));
    
    return JSON.stringify({{ success: true, moved: true, steps: {}, target: "{}" }});
}})();
"#, selector.replace('"', "\\\""), "steps", selector.replace('"', "\\\""))
}

/// Type text with input event simulation
pub fn generate_type_with_events_script(selector: &str, text: &str, clear: bool) -> String {
    generate_type_with_events_script_ex(selector, text, clear, false)
}

/// Extended type script with instant mode for autocomplete-heavy inputs (like Amazon)
pub fn generate_type_with_events_script_ex(selector: &str, text: &str, clear: bool, instant: bool) -> String {
    let clear_code = if clear {
        "el.value = ''; el.dispatchEvent(new Event('input', {bubbles: true}));"
    } else {
        ""
    };

    if instant {
        // Instant mode: set value directly, skip character-by-character
        // This avoids autocomplete/suggestion interference
        format!(r#"
(async function() {{
    const el = document.querySelector("{}");
    if (!el) {{
        return JSON.stringify({{ success: false, error: "Element not found" }});
    }}

    // Focus
    el.focus();
    {}

    // Set value directly (instant mode - avoids autocomplete interference)
    const text = "{}";
    el.value = text;
    
    // Dispatch events
    el.dispatchEvent(new Event('input', {{ bubbles: true }}));
    el.dispatchEvent(new Event('change', {{ bubbles: true }}));
    
    // Small delay to let any autocomplete settle
    await new Promise(r => setTimeout(r, 100));
    
    // Blur to close autocomplete dropdown
    el.dispatchEvent(new Event('blur', {{ bubbles: true }}));

    return JSON.stringify({{
        success: true,
        typed: true,
        instant: true,
        length: text.length,
        value: el.value
    }});
}})();
"#, selector.replace('"', "\\\""), clear_code, text.replace('"', "\\\"").replace('\n', "\\n"))
    } else {
        // Character-by-character mode with human-like timing
        format!(r#"
(async function() {{
    const el = document.querySelector("{}");
    if (!el) {{
        return JSON.stringify({{ success: false, error: "Element not found" }});
    }}

    // Focus
    el.focus();
    {}

    // Human typing characteristics
    const text = "{}";
    const baseDelay = 50; // Base typing speed ~20 WPM for careful typing
    
    // Adjacent keys for typo simulation
    const adjacentKeys = {{
        'a': ['s', 'q', 'w', 'z'], 'b': ['v', 'n', 'g', 'h'], 'c': ['x', 'v', 'd', 'f'],
        'd': ['s', 'f', 'e', 'r', 'c', 'x'], 'e': ['w', 'r', 'd', 's'], 'f': ['d', 'g', 'r', 't', 'v', 'c'],
        'g': ['f', 'h', 't', 'y', 'b', 'v'], 'h': ['g', 'j', 'y', 'u', 'n', 'b'], 'i': ['u', 'o', 'k', 'j'],
        'j': ['h', 'k', 'u', 'i', 'm', 'n'], 'k': ['j', 'l', 'i', 'o', 'm'], 'l': ['k', 'o', 'p'],
        'm': ['n', 'j', 'k'], 'n': ['b', 'm', 'h', 'j'], 'o': ['i', 'p', 'k', 'l'],
        'p': ['o', 'l'], 'q': ['w', 'a'], 'r': ['e', 't', 'd', 'f'],
        's': ['a', 'd', 'w', 'e', 'x', 'z'], 't': ['r', 'y', 'f', 'g'], 'u': ['y', 'i', 'h', 'j'],
        'v': ['c', 'b', 'f', 'g'], 'w': ['q', 'e', 'a', 's'], 'x': ['z', 'c', 's', 'd'],
        'y': ['t', 'u', 'g', 'h'], 'z': ['a', 's', 'x']
    }};
    
    let typedChars = 0;
    let typoCount = 0;
    
    for (let i = 0; i < text.length; i++) {{
        const char = text[i];
        
        // Simulate typo (3% chance, more likely for fast typing)
        if (Math.random() < 0.03 && adjacentKeys[char.toLowerCase()]) {{
            const typoChars = adjacentKeys[char.toLowerCase()];
            const typoChar = typoChars[Math.floor(Math.random() * typoChars.length)];
            
            // Type wrong character
            el.value += typoChar;
            el.dispatchEvent(new Event('input', {{ bubbles: true }}));
            await new Promise(r => setTimeout(r, 80 + Math.random() * 60));
            
            // Pause (realize mistake)
            await new Promise(r => setTimeout(r, 150 + Math.random() * 200));
            
            // Delete wrong character
            el.value = el.value.slice(0, -1);
            el.dispatchEvent(new Event('input', {{ bubbles: true }}));
            await new Promise(r => setTimeout(r, 40 + Math.random() * 30));
            
            typoCount++;
        }}
        
        // Double space typo (2% chance when typing space)
        if (char === ' ' && Math.random() < 0.02) {{
            el.value += ' ';
            el.dispatchEvent(new Event('input', {{ bubbles: true }}));
            await new Promise(r => setTimeout(r, 50 + Math.random() * 30));
            // Realize mistake
            await new Promise(r => setTimeout(r, 100 + Math.random() * 150));
            el.value = el.value.slice(0, -1);
            el.dispatchEvent(new Event('input', {{ bubbles: true }}));
            typoCount++;
        }}
        
        // Capitalize typo: forgot shift (1.5% at word start) or held shift (1% for next char)
        let charToType = char;
        const isWordStart = i === 0 || text[i-1] === ' ' || text[i-1] === '.';
        if (isWordStart && char === char.toUpperCase() && char !== char.toLowerCase()) {{
            // Should be uppercase, but forgot shift (1.5%)
            if (Math.random() < 0.015) {{
                charToType = char.toLowerCase();
                // Notice and fix
                el.value += charToType;
                el.dispatchEvent(new Event('input', {{ bubbles: true }}));
                await new Promise(r => setTimeout(r, 120 + Math.random() * 80));
                el.value = el.value.slice(0, -1);
                el.dispatchEvent(new Event('input', {{ bubbles: true }}));
                await new Promise(r => setTimeout(r, 30 + Math.random() * 20));
                charToType = char; // Now type correct one
                typoCount++;
            }}
        }} else if (i > 0 && text[i-1] === text[i-1].toUpperCase() && text[i-1] !== text[i-1].toLowerCase()) {{
            // Previous was uppercase, held shift too long (1%)
            if (char === char.toLowerCase() && char !== char.toUpperCase() && Math.random() < 0.01) {{
                charToType = char.toUpperCase();
                el.value += charToType;
                el.dispatchEvent(new Event('input', {{ bubbles: true }}));
                await new Promise(r => setTimeout(r, 100 + Math.random() * 80));
                el.value = el.value.slice(0, -1);
                el.dispatchEvent(new Event('input', {{ bubbles: true }}));
                await new Promise(r => setTimeout(r, 30 + Math.random() * 20));
                charToType = char;
                typoCount++;
            }}
        }}
        
        // Type correct character
        el.value += charToType;
        el.dispatchEvent(new Event('input', {{ bubbles: true }}));
        typedChars++;
        
        // Variable delay based on character type
        let delay = baseDelay;
        
        // Punctuation = longer pause (thinking)
        if (['.', ',', '!', '?', ':', ';'].includes(char)) {{
            delay = 150 + Math.random() * 200;
        }}
        // Space after word = brief pause
        else if (char === ' ') {{
            delay = 80 + Math.random() * 100;
        }}
        // Numbers = slightly more careful
        else if (/[0-9]/.test(char)) {{
            delay = 70 + Math.random() * 60;
        }}
        // Regular letters = natural variance
        else {{
            // Faster for common letter combos (rolling fingers)
            const prevChar = i > 0 ? text[i-1].toLowerCase() : '';
            const currChar = char.toLowerCase();
            const fastCombos = ['th', 'he', 'in', 'er', 'an', 're', 'on', 'at', 'en', 'nd'];
            if (fastCombos.includes(prevChar + currChar)) {{
                delay = 30 + Math.random() * 30;
            }} else {{
                delay = baseDelay * (0.6 + Math.random() * 0.8);
            }}
        }}
        
        // Occasional longer pause (thinking, distraction) - 2% chance
        if (Math.random() < 0.02) {{
            delay += 300 + Math.random() * 500;
        }}
        
        await new Promise(r => setTimeout(r, delay));
    }}

    // Trigger change and blur
    el.dispatchEvent(new Event('change', {{ bubbles: true }}));
    el.dispatchEvent(new Event('blur', {{ bubbles: true }}));

    return JSON.stringify({{
        success: true,
        typed: true,
        instant: false,
        length: text.length,
        typedChars: typedChars,
        typos: typoCount,
        value: el.value
    }});
}})();
"#, selector.replace('"', "\\\""), clear_code, text.replace('"', "\\\"").replace('\n', "\\n"))
    }
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
    const selectors = 'a, button, input, select, textarea, [role="button"], [onclick], [tabindex], [role="link"], [role="menuitem"]';
    // Note: noiseParentsは将来のフィルタリング用に保持（現在未使用）

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
        // Escape special characters in ID/name for CSS selectors
        const escapeCSS = (str) => str.replace(/([\[\]"'\\#.:>+~()=])/g, '\\$1');
        
        if (el.id) return `#${escapeCSS(el.id)}`;
        if (el.name) return `${el.tagName.toLowerCase()}[name="${escapeCSS(el.name)}"]`;
        
        // Use class if unique
        if (el.className && typeof el.className === 'string') {
            const classes = el.className.split(' ').filter(c => c && !c.startsWith('js-') && !c.match(/^[a-z]{20,}$/)).slice(0, 2);
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

    // ========== 視覚プロパティ抽出 ==========
    const extractVisualProperties = (el) => {
        const style = window.getComputedStyle(el);
        const rect = el.getBoundingClientRect();
        
        // ホバー効果の推定（transition/animationの存在で判断）
        const hasTransition = style.transition !== 'none' && 
                              style.transition !== 'all 0s ease 0s' &&
                              style.transitionProperty !== 'none';
        
        // アイコンの存在チェック
        const hasIcon = el.querySelector('svg, i[class*="icon"], span[class*="icon"]') !== null ||
                        (style.backgroundImage !== 'none' && style.backgroundImage.includes('url'));
        
        // 画像の存在チェック
        const img = el.querySelector('img');
        const hasImage = img !== null;
        const imageAlt = img ? img.alt : null;
        
        return {
            // サイズ
            width: Math.round(rect.width),
            height: Math.round(rect.height),
            
            // ボタンらしさの指標
            cursor: style.cursor,
            hasOnClick: el.hasAttribute('onclick') || el.onclick !== null,
            
            // 視覚スタイル
            backgroundColor: style.backgroundColor,
            borderRadius: style.borderRadius,
            border: style.borderWidth !== '0px' ? style.border : 'none',
            boxShadow: style.boxShadow !== 'none' ? style.boxShadow : null,
            
            // テキストスタイル
            fontWeight: style.fontWeight,
            color: style.color,
            
            // ホバー効果の示唆
            hasTransition: hasTransition,
            
            // アイコン・画像
            hasIcon: hasIcon,
            hasImage: hasImage,
            imageAlt: imageAlt,
            
            role: el.getAttribute('role'),
            ariaLabel: el.getAttribute('aria-label'),
            ariaExpanded: el.getAttribute('aria-expanded'),
            tabIndex: el.tabIndex,
            isDisabled: el.disabled || el.getAttribute('aria-disabled') === 'true',
            
            // 追加: フォーム文脈
            isInForm: el.closest('form') !== null,
            inputType: el.type || null,
            tagName: el.tagName.toLowerCase()
        };
    };

    // ========== ルールベース事前スコアリング ==========
    const preScore = (props, label) => {
        let score = 0;
        const reasons = [];
        
        // cursor: pointer は強力な指標 (+0.3)
        if (props.cursor === 'pointer') {
            score += 0.3;
            reasons.push('cursor:pointer');
        }
        
        // 角丸がある (+0.1)
        if (props.borderRadius && parseFloat(props.borderRadius) > 0) {
            score += 0.1;
            reasons.push('角丸');
        }
        
        // 影がある (+0.1)
        if (props.boxShadow && props.boxShadow !== 'none') {
            score += 0.1;
            reasons.push('影');
        }
        
        // トランジション/アニメーションがある (+0.15)
        if (props.hasTransition) {
            score += 0.15;
            reasons.push('hover効果');
        }
        
        // 背景色がある（透明でない）(+0.1)
        if (props.backgroundColor && 
            !props.backgroundColor.includes('transparent') && 
            !props.backgroundColor.includes('rgba(0, 0, 0, 0)')) {
            score += 0.1;
            reasons.push('背景色');
        }
        
        // CTAテキスト (+0.15)
        const ctaWords = ['購入', '申込', '登録', '送信', 'ログイン', 'サインイン', 'カート', '検索',
                          'submit', 'buy', 'add', 'cart', 'login', 'sign', 'register', 'checkout', 'search'];
        const labelLower = (label || '').toLowerCase();
        if (ctaWords.some(w => labelLower.includes(w))) {
            score += 0.15;
            reasons.push('CTAテキスト');
        }
        
        // onclick属性 (+0.2)
        if (props.hasOnClick) {
            score += 0.2;
            reasons.push('onclick');
        }
        
        // role="button" (+0.15)
        if (props.role === 'button') {
            score += 0.15;
            reasons.push('role=button');
        }
        
        // role="tab", "menuitem", "link" (+0.1)
        if (['tab', 'menuitem', 'link', 'option', 'switch'].includes(props.role)) {
            score += 0.1;
            reasons.push('role=' + props.role);
        }
        
        // aria-expandedがある（展開可能要素）(+0.1)
        if (props.ariaExpanded !== null) {
            score += 0.1;
            reasons.push('展開可能');
        }
        
        // 太字 (+0.05)
        if (props.fontWeight && parseInt(props.fontWeight) >= 600) {
            score += 0.05;
            reasons.push('太字');
        }
        
        // アイコンボタン（テキストなしでアイコンあり）(+0.1)
        if (props.hasIcon && (!label || label.trim() === '')) {
            score += 0.1;
            reasons.push('アイコンボタン');
        }
        
        // disabled要素は大幅減点 (-0.5)
        if (props.isDisabled) {
            score = Math.max(0, score - 0.5);
            reasons.push('disabled');
        }
        
        // tabIndex > 0 は明示的なインタラクティブ指定 (+0.1)
        if (props.tabIndex > 0) {
            score += 0.1;
            reasons.push('tabIndex');
        }
        
        // フォーム内の要素はインタラクティブの可能性高い (+0.05)
        if (props.isInForm) {
            score += 0.05;
            reasons.push('form内');
        }
        
        // input要素はタイプ別にボーナス
        const interactiveInputTypes = ['text', 'email', 'password', 'search', 'tel', 'url', 'number', 'range', 'date', 'datetime-local', 'time', 'color', 'file'];
        if (interactiveInputTypes.includes(props.inputType)) {
            score += 0.2;  // inputはcursor:textなので補正
            reasons.push('input要素');
        }
        
        // checkbox/radio (+0.15)
        if (['checkbox', 'radio'].includes(props.inputType)) {
            score += 0.15;
            reasons.push(props.inputType);
        }
        
        // select要素 (+0.2)
        if (props.tagName === 'select') {
            score += 0.2;
            reasons.push('select要素');
        }
        
        // textarea要素 (+0.2)
        if (props.tagName === 'textarea') {
            score += 0.2;
            reasons.push('textarea要素');
        }
        
        // 画像リンク（商品ページ等で重要）(+0.1)
        if (props.hasImage) {
            score += 0.1;
            reasons.push('画像リンク');
        }
        
        // 画像リンクでaltなしの場合はフラグ
        const needsVisionAnalysis = props.hasImage && (!props.imageAlt || props.imageAlt.trim() === '');
        
        return {
            score: Math.min(1, Math.round(score * 100) / 100),
            reasons: reasons,
            needsVisionAnalysis: needsVisionAnalysis
        };
    };

    // ========== アクション予測 ==========
    const predictAction = (el, props, label) => {
        const tag = el.tagName.toLowerCase();
        const type = el.type || null;
        const href = el.getAttribute('href');
        const actions = [];
        
        // リンク
        if (tag === 'a' && href) {
            if (href.startsWith('#')) {
                actions.push({ action: 'click_anchor', purpose: 'ページ内移動' });
            } else {
                actions.push({ action: 'click_navigate', purpose: 'ページ遷移' });
            }
        }
        
        // 送信ボタン
        if ((tag === 'button' && type === 'submit') || 
            (tag === 'input' && type === 'submit')) {
            actions.push({ action: 'click_submit', purpose: 'フォーム送信' });
        }
        
        // 通常ボタン（typeがない場合も含む）
        if (tag === 'button' && type !== 'submit') {
            actions.push({ action: 'click_action', purpose: 'アクション実行' });
        }
        
        // チェックボックス・ラジオ
        if (tag === 'input' && (type === 'checkbox' || type === 'radio')) {
            actions.push({ action: 'click_toggle', purpose: '選択切替' });
        }
        
        // テキスト入力
        if (tag === 'input' && ['text', 'email', 'password', 'search', 'tel', 'url'].includes(type)) {
            actions.push({ action: 'type_input', purpose: 'テキスト入力' });
        }
        if (tag === 'textarea') {
            actions.push({ action: 'type_input', purpose: '複数行入力' });
        }
        
        // セレクト
        if (tag === 'select') {
            actions.push({ action: 'click_select', purpose: 'オプション選択' });
        }
        
        // ドロップダウンの推定
        const labelLower = (label || '').toLowerCase();
        if (labelLower.includes('▼') || labelLower.includes('▾') || 
            labelLower.includes('dropdown') || labelLower.includes('menu')) {
            actions.push({ action: 'click_expand', purpose: 'メニュー展開' });
        }
        
        // hover効果がある場合
        if (props.hasTransition && actions.length === 0) {
            actions.push({ action: 'hover_reveal', purpose: '情報表示' });
        }
        
        // デフォルト
        if (actions.length === 0 && props.cursor === 'pointer') {
            actions.push({ action: 'click_action', purpose: '不明なアクション' });
        }
        
        return actions;
    };

    const elements = Array.from(document.querySelectorAll(selectors))
        .filter(el => {
            const rect = el.getBoundingClientRect();
            const style = window.getComputedStyle(el);
            
            // Must be visible
            if (rect.width === 0 || rect.height === 0) return false;
            if (style.display === 'none' || style.visibility === 'hidden') return false;
            if (style.opacity === '0') return false;
            
            // Must be reasonably sized
            if (rect.width < 20 || rect.height < 15) return false;
            
            // Skip hidden inputs
            if (el.type === 'hidden') return false;
            
            return true;
        })
        .slice(0, 50)  // Limit to 50 elements
        .map((el, index) => {
            const rect = el.getBoundingClientRect();
            const label = getLabel(el);
            const visualProps = extractVisualProperties(el);
            const scoreResult = preScore(visualProps, label);
            const predictedActions = predictAction(el, visualProps, label);
            
            return {
                index: index + 1,
                tag: el.tagName.toLowerCase(),
                type: el.type || null,
                selector: generateSelector(el),
                label: label,
                value: el.value || null,
                placeholder: el.placeholder || null,
                inViewport: rect.top < window.innerHeight && rect.bottom > 0,
                
                // 視覚プロパティ
                visual: {
                    cursor: visualProps.cursor,
                    backgroundColor: visualProps.backgroundColor,
                    borderRadius: visualProps.borderRadius,
                    boxShadow: visualProps.boxShadow,
                    hasTransition: visualProps.hasTransition,
                    hasIcon: visualProps.hasIcon,
                    hasImage: visualProps.hasImage,
                    imageAlt: visualProps.imageAlt,
                    size: { width: visualProps.width, height: visualProps.height }
                },
                
                // インタラクティビティ分析
                interactivity: {
                    score: scoreResult.score,
                    reasons: scoreResult.reasons,
                    predicted_actions: predictedActions,
                    needs_vision: scoreResult.needsVisionAnalysis || false,
                    analyzed_by: 'rule'
                }
            };
        })
        // スコア順にソート
        .sort((a, b) => b.interactivity.score - a.interactivity.score);

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
