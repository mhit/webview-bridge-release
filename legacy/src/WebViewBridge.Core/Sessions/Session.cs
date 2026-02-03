using Microsoft.Web.WebView2.Core;

namespace WebViewBridge.Core.Sessions;

public class Session : IDisposable
{
    public string Id { get; }
    public SessionOptions Options { get; }
    public CoreWebView2? WebView { get; internal set; }
    public DateTime CreatedAt { get; }
    public DateTime LastAccessedAt { get; private set; }
    public SessionStatus Status { get; internal set; } = SessionStatus.Creating;
    public string? CurrentUrl { get; internal set; }

    private bool _disposed;

    public Session(string id, SessionOptions options)
    {
        Id = id;
        Options = options;
        CreatedAt = DateTime.UtcNow;
        LastAccessedAt = DateTime.UtcNow;
    }

    public void Touch()
    {
        LastAccessedAt = DateTime.UtcNow;
    }

    public bool IsExpired(TimeSpan timeout)
    {
        return DateTime.UtcNow - LastAccessedAt > timeout;
    }

    public void Dispose()
    {
        if (_disposed) return;
        _disposed = true;
        Status = SessionStatus.Disposed;
        // WebView2 cleanup handled by WebViewInstance
    }
}

public enum SessionStatus
{
    Creating,
    Ready,
    Busy,
    Error,
    Disposed
}
