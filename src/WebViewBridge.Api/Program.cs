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

// Configure and run
var port = args.Length > 0 && args[0].StartsWith("--port=") 
    ? int.Parse(args[0].Replace("--port=", ""))
    : 9400;

app.Urls.Add($"http://127.0.0.1:{port}");

Console.WriteLine($"WebView Bridge starting on http://127.0.0.1:{port}");
Console.WriteLine("Press Ctrl+C to stop");

app.Run();
