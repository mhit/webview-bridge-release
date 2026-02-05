/**
 * WebView Bridge TypeScript Definitions
 */

export interface SessionOptions {
    profile?: string;
    headless?: boolean;
}

export interface SessionStatus {
    status: 'Initializing' | 'Ready' | 'Busy' | 'Error';
    url?: string;
}

export interface Cookie {
    name: string;
    value: string;
    path?: string;
    domain?: string;
    secure?: boolean;
    httpOnly?: boolean;
}

export interface McpServerInfo {
    name: string;
    version: string;
    protocol_version: string;
}

export interface McpTool {
    name: string;
    description: string;
    input_schema: object;
}

export interface McpResource {
    uri: string;
    name: string;
    description: string;
    mimeType: string;
}

export interface McpContent {
    type: string;
    text?: string;
    data?: string;
    mimeType?: string;
}

export interface McpToolCallResponse {
    content: McpContent[];
    isError: boolean;
}

export declare class WebViewBridge {
    constructor(host?: string, port?: number);
    health(): Promise<boolean>;
    createSession(profile?: string, headless?: boolean): Promise<string>;
    navigate(sessionId: string, url: string): Promise<boolean>;
    execute(sessionId: string, script: string): Promise<any>;
    getStatus(sessionId: string): Promise<SessionStatus>;
    snapshot(sessionId: string, format?: 'html' | 'text'): Promise<string>;
    screenshot(sessionId: string): Promise<string>;
    getCookies(sessionId: string): Promise<Cookie[]>;
    waitForSelector(sessionId: string, selector: string, timeoutMs?: number): Promise<boolean>;
    extract(sessionId: string, selector: string, attribute?: string): Promise<string[]>;
    close(sessionId: string): Promise<boolean>;
}

export declare class WebDriverBridge {
    constructor(host?: string, port?: number);
    newSession(): Promise<string>;
    navigate(sessionId: string, url: string): Promise<void>;
    getUrl(sessionId: string): Promise<string>;
    getTitle(sessionId: string): Promise<string>;
    findElement(sessionId: string, using: string, value: string): Promise<string>;
    executeScript(sessionId: string, script: string, args?: any[]): Promise<any>;
    screenshot(sessionId: string): Promise<string>;
    quit(sessionId: string): Promise<void>;
}

export declare class MCPBridge {
    constructor(host?: string, port?: number);
    info(): Promise<McpServerInfo>;
    listTools(): Promise<McpTool[]>;
    callTool(name: string, args: object): Promise<McpToolCallResponse>;
    listResources(): Promise<McpResource[]>;
    browse(url: string): Promise<string>;
    screenshot(): Promise<string>;
    extract(selector: string): Promise<string>;
    wait(selector: string, timeout?: number): Promise<string>;
    getHtml(): Promise<string>;
    getText(): Promise<string>;
}
