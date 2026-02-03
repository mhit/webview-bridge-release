namespace WebViewBridge.Api.Models;

// Request Models
public record CreateSessionRequest(
    string Profile = "default",
    bool Headless = true,
    string? UserAgent = null,
    ViewportRequest? Viewport = null
);

public record ViewportRequest(int Width = 1920, int Height = 1080);

public record NavigateRequest(
    string Url,
    string WaitUntil = "load",
    int TimeoutMs = 30000
);

public record EvaluateRequest(
    string Script,
    string ReturnType = "string"
);

public record QueryRequest(
    string Selector,
    string[]? Attributes = null,
    bool TextContent = true,
    int Limit = 100
);

public record ScreenshotRequest(
    string Format = "png",
    bool FullPage = false,
    string? Selector = null,
    int Quality = 80
);

public record ActionRequest(
    ActionItem[] Actions
);

public record ActionItem(
    string Type,  // click, type, select, wait, scroll, hover
    string? Selector = null,
    string? Text = null,
    string? Value = null,
    int? Ms = null,
    int? Y = null
);

// Response Models
public record SessionResponse(
    string SessionId,
    string Status
);

public record NavigationResponse(
    string Status,
    string FinalUrl,
    int? StatusCode,
    int LoadTimeMs
);

public record EvaluateResponse(
    object? Result
);

public record QueryResponse(
    List<Dictionary<string, object?>> Elements
);

public record ScreenshotResponse(
    string Data,
    int Width,
    int Height
);

public record ErrorResponse(
    ErrorDetail Error
);

public record ErrorDetail(
    string Code,
    string Message,
    object? Details = null
);

public record StatusResponse(
    string Status,
    int ActiveSessions,
    int MaxSessions,
    string Version
);
