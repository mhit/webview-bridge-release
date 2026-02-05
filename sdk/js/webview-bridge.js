/**
 * WebView Bridge JavaScript/TypeScript SDK
 * A simple client for WebView Bridge server.
 * 
 * @example
 * ```javascript
 * import { WebViewBridge, MCPBridge, WebDriverBridge } from './webview-bridge.js';
 * 
 * // Native API
 * const bridge = new WebViewBridge();
 * const sessionId = await bridge.createSession();
 * await bridge.navigate(sessionId, 'https://example.com');
 * const title = await bridge.execute(sessionId, 'return document.title');
 * await bridge.close(sessionId);
 * 
 * // MCP API (for AI agents)
 * const mcp = new MCPBridge();
 * await mcp.browse('https://example.com');
 * const html = await mcp.getHtml();
 * 
 * // WebDriver API (Selenium compatible)
 * const wd = new WebDriverBridge();
 * const session = await wd.newSession();
 * await wd.navigate(session, 'https://example.com');
 * await wd.quit(session);
 * ```
 */

/**
 * WebView Bridge Native API Client
 */
export class WebViewBridge {
    /**
     * @param {string} host - Server host
     * @param {number} port - Server port
     */
    constructor(host = 'localhost', port = 9400) {
        this.baseUrl = `http://${host}:${port}`;
    }

    /**
     * Check server health
     * @returns {Promise<boolean>}
     */
    async health() {
        try {
            const response = await fetch(`${this.baseUrl}/health`);
            return response.ok;
        } catch {
            return false;
        }
    }

    /**
     * Create a new browser session
     * @param {string} profile - Profile name
     * @param {boolean} headless - Headless mode
     * @returns {Promise<string>} Session ID
     */
    async createSession(profile = 'default', headless = false) {
        const response = await fetch(`${this.baseUrl}/create`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ profile, headless })
        });
        if (!response.ok) throw new Error(`Failed to create session: ${response.status}`);
        const data = await response.json();
        return data.id;
    }

    /**
     * Navigate to URL
     * @param {string} sessionId - Session ID
     * @param {string} url - URL to navigate to
     * @returns {Promise<boolean>}
     */
    async navigate(sessionId, url) {
        const response = await fetch(`${this.baseUrl}/navigate/${sessionId}`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ url })
        });
        if (!response.ok) throw new Error(`Navigate failed: ${response.status}`);
        return true;
    }

    /**
     * Execute JavaScript
     * @param {string} sessionId - Session ID
     * @param {string} script - JavaScript code
     * @returns {Promise<any>}
     */
    async execute(sessionId, script) {
        const response = await fetch(`${this.baseUrl}/execute/${sessionId}`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ script })
        });
        if (!response.ok) throw new Error(`Execute failed: ${response.status}`);
        const data = await response.json();
        return data.result;
    }

    /**
     * Get session status
     * @param {string} sessionId - Session ID
     * @returns {Promise<object>}
     */
    async getStatus(sessionId) {
        const response = await fetch(`${this.baseUrl}/status/${sessionId}`);
        if (!response.ok) throw new Error(`Get status failed: ${response.status}`);
        return await response.json();
    }

    /**
     * Get page snapshot
     * @param {string} sessionId - Session ID
     * @param {string} format - 'html' or 'text'
     * @returns {Promise<string>}
     */
    async snapshot(sessionId, format = 'html') {
        const response = await fetch(`${this.baseUrl}/snapshot/${sessionId}?format=${format}`);
        if (!response.ok) throw new Error(`Snapshot failed: ${response.status}`);
        const data = await response.json();
        return data.content;
    }

    /**
     * Take screenshot
     * @param {string} sessionId - Session ID
     * @returns {Promise<string>} Base64 PNG
     */
    async screenshot(sessionId) {
        const response = await fetch(`${this.baseUrl}/screenshot/${sessionId}`);
        if (!response.ok) throw new Error(`Screenshot failed: ${response.status}`);
        const data = await response.json();
        return data.image;
    }

    /**
     * Get cookies
     * @param {string} sessionId - Session ID
     * @returns {Promise<Array>}
     */
    async getCookies(sessionId) {
        const response = await fetch(`${this.baseUrl}/cookies/${sessionId}`);
        if (!response.ok) throw new Error(`Get cookies failed: ${response.status}`);
        const data = await response.json();
        return data.cookies || [];
    }

    /**
     * Wait for selector
     * @param {string} sessionId - Session ID
     * @param {string} selector - CSS selector
     * @param {number} timeoutMs - Timeout in milliseconds
     * @returns {Promise<boolean>}
     */
    async waitForSelector(sessionId, selector, timeoutMs = 10000) {
        const response = await fetch(`${this.baseUrl}/wait/${sessionId}`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ selector, timeout_ms: timeoutMs })
        });
        if (!response.ok) throw new Error(`Wait failed: ${response.status}`);
        const data = await response.json();
        return data.found;
    }

    /**
     * Extract content from elements
     * @param {string} sessionId - Session ID
     * @param {string} selector - CSS selector
     * @param {string} attribute - Attribute to extract
     * @returns {Promise<Array>}
     */
    async extract(sessionId, selector, attribute = 'text') {
        const response = await fetch(`${this.baseUrl}/extract/${sessionId}`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ selector, attribute, all: true })
        });
        if (!response.ok) throw new Error(`Extract failed: ${response.status}`);
        const data = await response.json();
        return data.results || [];
    }

    /**
     * Close session
     * @param {string} sessionId - Session ID
     * @returns {Promise<boolean>}
     */
    async close(sessionId) {
        const response = await fetch(`${this.baseUrl}/close/${sessionId}`, {
            method: 'DELETE'
        });
        if (!response.ok) throw new Error(`Close failed: ${response.status}`);
        return true;
    }
}

/**
 * WebDriver Protocol Compatible Client (Selenium compatible)
 */
export class WebDriverBridge {
    constructor(host = 'localhost', port = 9400) {
        this.baseUrl = `http://${host}:${port}/wd/hub`;
    }

    async newSession() {
        const response = await fetch(`${this.baseUrl}/session`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ capabilities: {} })
        });
        if (!response.ok) throw new Error(`New session failed: ${response.status}`);
        const data = await response.json();
        return data.value.sessionId;
    }

    async navigate(sessionId, url) {
        const response = await fetch(`${this.baseUrl}/session/${sessionId}/url`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ url })
        });
        if (!response.ok) throw new Error(`Navigate failed: ${response.status}`);
    }

    async getUrl(sessionId) {
        const response = await fetch(`${this.baseUrl}/session/${sessionId}/url`);
        if (!response.ok) throw new Error(`Get URL failed: ${response.status}`);
        const data = await response.json();
        return data.value;
    }

    async getTitle(sessionId) {
        const response = await fetch(`${this.baseUrl}/session/${sessionId}/title`);
        if (!response.ok) throw new Error(`Get title failed: ${response.status}`);
        const data = await response.json();
        return data.value;
    }

    async findElement(sessionId, using, value) {
        const response = await fetch(`${this.baseUrl}/session/${sessionId}/element`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ using, value })
        });
        if (!response.ok) throw new Error(`Find element failed: ${response.status}`);
        const data = await response.json();
        return data.value['element-6066-11e4-a52e-4f735466cecf'];
    }

    async executeScript(sessionId, script, args = []) {
        const response = await fetch(`${this.baseUrl}/session/${sessionId}/execute/sync`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ script, args })
        });
        if (!response.ok) throw new Error(`Execute script failed: ${response.status}`);
        const data = await response.json();
        return data.value;
    }

    async screenshot(sessionId) {
        const response = await fetch(`${this.baseUrl}/session/${sessionId}/screenshot`);
        if (!response.ok) throw new Error(`Screenshot failed: ${response.status}`);
        const data = await response.json();
        return data.value;
    }

    async quit(sessionId) {
        const response = await fetch(`${this.baseUrl}/session/${sessionId}`, {
            method: 'DELETE'
        });
        if (!response.ok) throw new Error(`Quit failed: ${response.status}`);
    }
}

/**
 * MCP (Model Context Protocol) Client
 */
export class MCPBridge {
    constructor(host = 'localhost', port = 9400) {
        this.baseUrl = `http://${host}:${port}/mcp`;
    }

    async info() {
        const response = await fetch(`${this.baseUrl}/info`);
        if (!response.ok) throw new Error(`Get info failed: ${response.status}`);
        return await response.json();
    }

    async listTools() {
        const response = await fetch(`${this.baseUrl}/tools`);
        if (!response.ok) throw new Error(`List tools failed: ${response.status}`);
        return await response.json();
    }

    async callTool(name, args) {
        const response = await fetch(`${this.baseUrl}/tools/call`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ name, arguments: args })
        });
        if (!response.ok) throw new Error(`Call tool failed: ${response.status}`);
        return await response.json();
    }

    async listResources() {
        const response = await fetch(`${this.baseUrl}/resources`);
        if (!response.ok) throw new Error(`List resources failed: ${response.status}`);
        return await response.json();
    }

    // Convenience methods
    async browse(url) {
        const result = await this.callTool('browse', { url });
        return result.content[0].text;
    }

    async screenshot() {
        const result = await this.callTool('screenshot', {});
        return result.content[0].data;
    }

    async extract(selector) {
        const result = await this.callTool('extract', { selector });
        return result.content[0].text;
    }

    async wait(selector, timeout = 10000) {
        const result = await this.callTool('wait', { selector, timeout });
        return result.content[0].text;
    }

    async getHtml() {
        const result = await this.callTool('get_html', {});
        return result.content[0].text;
    }

    async getText() {
        const result = await this.callTool('get_text', {});
        return result.content[0].text;
    }
}

// CommonJS export support
if (typeof module !== 'undefined' && module.exports) {
    module.exports = { WebViewBridge, WebDriverBridge, MCPBridge };
}
