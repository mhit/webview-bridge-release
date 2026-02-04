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

// New OpenClaw Request Models
public record OpenClawSnapshotRequest(
    string TargetUrl,
    string Refs = "aria",
    bool Interactive = true
);

public record OpenClawActRequest(
    OpenClawAction Request
);

public record OpenClawAction(
    string Kind, // e.g., "click", "type", "press", "hover", "scroll"
    string Ref // e.g., "button[Submit]"
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
```
```csharp
using System.Text.Json;
using WebViewBridge.Api.Models;
using WebViewBridge.Core.Sessions;

var builder = WebApplication.CreateBuilder(args);

// Configure services
builder.Services.AddSingleton<SessionManager>();
builder.Services.AddEndpointsApiExplorer();

var app = builder.Build();

// Auth middleware
app.Use(async (context, next) =>
{
    if (context.Request.Path.StartsWithSegments("/api"))
    {
        var token = context.Request.Headers.Authorization.FirstOrDefault()?.Replace("Bearer ", "");
        var expectedToken = Environment.GetEnvironmentVariable("WEBVIEW_BRIDGE_TOKEN") ?? "dev-token";
        
        if (token != expectedToken)
        {
            context.Response.StatusCode = 401;
            await context.Response.WriteAsJsonAsync(new ErrorResponse(
                new ErrorDetail("UNAUTHORIZED", "Invalid or missing authentication token")
            ));
            return;
        }
    }
    await next();
});

// Status endpoint
app.MapGet("/api/status", (SessionManager sessionManager) =>
{
    return Results.Ok(new StatusResponse(
        Status: "ok",
        ActiveSessions: sessionManager.ActiveSessionCount,
        MaxSessions: 5,
        Version: "1.0.0"
    ));
});

// Session endpoints
app.MapPost("/api/sessions", async (CreateSessionRequest request, SessionManager sessionManager) =>
{
    try
    {
        var options = new SessionOptions
        {
            Profile = request.Profile,
            Headless = request.Headless,
            UserAgent = request.UserAgent,
            Viewport = request.Viewport != null 
                ? new ViewportSize(request.Viewport.Width, request.Viewport.Height)
                : new ViewportSize(1920, 1080)
        };

        var session = await sessionManager.CreateSessionAsync(options);
        
        // TODO: Initialize WebView2 instance here
        // For MVP, we'll just create the session metadata
        session.Status = SessionStatus.Ready;

        return Results.Ok(new SessionResponse(session.Id, session.Status.ToString().ToLower()));
    }
    catch (InvalidOperationException ex)
    {
        return Results.BadRequest(new ErrorResponse(
            new ErrorDetail("SESSION_LIMIT_EXCEEDED", ex.Message)
        ));
    }
});

app.MapGet("/api/sessions/{sessionId}", (string sessionId, SessionManager sessionManager) =>
{
    var session = sessionManager.GetSession(sessionId);
    if (session == null)
    {
        return Results.NotFound(new ErrorResponse(
            new ErrorDetail("SESSION_NOT_FOUND", $"Session {sessionId} not found")
        ));
    }

    return Results.Ok(new
    {
        SessionId = session.Id,
        Status = session.Status.ToString().ToLower(),
        Profile = session.Options.Profile,
        CurrentUrl = session.CurrentUrl,
        CreatedAt = session.CreatedAt,
        LastAccessedAt = session.LastAccessedAt
    });
});

app.MapDelete("/api/sessions/{sessionId}", (string sessionId, SessionManager sessionManager) =>
{
    if (sessionManager.TryRemoveSession(sessionId, out _))
    {
        return Results.Ok(new { Status = "deleted" });
    }

    return Results.NotFound(new ErrorResponse(
        new ErrorDetail("SESSION_NOT_FOUND", $"Session {sessionId} not found")
    ));
});

// Navigation endpoint
app.MapPost("/api/sessions/{sessionId}/navigate", async (string sessionId, NavigateRequest request, SessionManager sessionManager) =>
{
    var session = sessionManager.GetSession(sessionId);
    if (session == null)
    {
        return Results.NotFound(new ErrorResponse(
            new ErrorDetail("SESSION_NOT_FOUND", $"Session {sessionId} not found")
        ));
    }

    // TODO: Implement actual navigation with WebView2
    // For MVP, return mock response
    session.CurrentUrl = request.Url;
    
    return Results.Ok(new NavigationResponse(
        Status: "ok",
        FinalUrl: request.Url,
        StatusCode: 200,
        LoadTimeMs: 100
    ));
});

// Evaluate endpoint
app.MapPost("/api/sessions/{sessionId}/evaluate", async (string sessionId, EvaluateRequest request, SessionManager sessionManager) =>
{
    var session = sessionManager.GetSession(sessionId);
    if (session == null)
    {
        return Results.NotFound(new ErrorResponse(
            new ErrorDetail("SESSION_NOT_FOUND", $"Session {sessionId} not found")
        ));
    }

    // TODO: Implement actual script evaluation with WebView2
    return Results.Ok(new EvaluateResponse(Result: null));
});

// Screenshot endpoint
app.MapPost("/api/sessions/{sessionId}/screenshot", async (string sessionId, ScreenshotRequest request, SessionManager sessionManager) =>
{
    var session = sessionManager.GetSession(sessionId);
    if (session == null)
    {
        return Results.NotFound(new ErrorResponse(
            new ErrorDetail("SESSION_NOT_FOUND", $"Session {sessionId} not found")
        ));
    }

    // TODO: Implement actual screenshot with WebView2
    return Results.Ok(new ScreenshotResponse(
        Data: "",
        Width: 1920,
        Height: 1080
    ));
});

// OpenClaw Endpoints
app.MapPost("/api/openclaw/snapshot", async (OpenClawSnapshotRequest request) =>
{
    // TODO: Implement ARIA snapshot logic here using WebView2
    // For now, return a mock response
    Console.WriteLine($"Received OpenClaw Snapshot Request: TargetUrl={request.TargetUrl}, Refs={request.Refs}, Interactive={request.Interactive}");
    
    return Results.Ok(new 
    {
        Snapshot = "<mock_aria_snapshot_xml>",
        Url = request.TargetUrl
    });
});

app.MapPost("/api/openclaw/act", async (OpenClawActRequest request) =>
{
    // TODO: Implement action execution logic here using WebView2
    // For now, return a mock success response
    Console.WriteLine($"Received OpenClaw Act Request: Kind={request.Request.Kind}, Ref={request.Request.Ref}");

    return Results.Ok(new 
    {
        Status = "Action executed successfully",
        Kind = request.Request.Kind,
        Ref = request.Request.Ref
    });
});


// Configure and run
var port = args.Length > 0 && args[0].StartsWith("--port=") 
    ? int.Parse(args[0].Replace("--port=", ""))
    : 9400;

app.Urls.Add($"http://127.0.0.1:{port}");

Console.WriteLine($"WebView Bridge starting on http://127.0.0.1:{port}");
Console.WriteLine("Press Ctrl+C to stop");

app.Run();
