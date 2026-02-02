using System.Text.Json;
using Microsoft.Extensions.Logging;
using Microsoft.Web.WebView2.Core;
using WebViewBridge.Core.Sessions;

namespace WebViewBridge.Core.WebView;

public class WebViewInstance : IDisposable
{
    private readonly Session _session;
    private readonly ILogger _logger;
    private CoreWebView2Environment? _environment;
    private CoreWebView2Controller? _controller;
    private TaskCompletionSource<bool>? _navigationTcs;
    private bool _disposed;

    public CoreWebView2? CoreWebView2 => _controller?.CoreWebView2;
    public bool IsInitialized => CoreWebView2 != null;

    public WebViewInstance(Session session, ILogger logger)
    {
        _session = session;
        _logger = logger;
    }

    public async Task InitializeAsync(string userDataFolder)
    {
        var options = new CoreWebView2EnvironmentOptions();
        
        if (_session.Options.Headless)
        {
            // WebView2 doesn't have true headless, but we can minimize/hide the window
            options.AdditionalBrowserArguments = "--disable-gpu";
        }

        _environment = await CoreWebView2Environment.CreateAsync(
            browserExecutableFolder: null,
            userDataFolder: userDataFolder,
            options: options
        );

        _controller = await _environment.CreateCoreWebView2ControllerAsync(IntPtr.Zero);
        
        if (_controller?.CoreWebView2 == null)
        {
            throw new InvalidOperationException("Failed to initialize WebView2");
        }

        var webView = _controller.CoreWebView2;
        
        // Configure settings
        webView.Settings.IsScriptEnabled = true;
        webView.Settings.IsWebMessageEnabled = true;
        webView.Settings.AreDefaultContextMenusEnabled = false;
        webView.Settings.AreDevToolsEnabled = false;

        if (!string.IsNullOrEmpty(_session.Options.UserAgent))
        {
            webView.Settings.UserAgent = _session.Options.UserAgent;
        }

        // Set up event handlers
        webView.NavigationStarting += OnNavigationStarting;
        webView.NavigationCompleted += OnNavigationCompleted;
        webView.ProcessFailed += OnProcessFailed;

        _session.WebView = webView;
        _session.Status = SessionStatus.Ready;
        
        _logger.LogInformation("WebView2 initialized for session {SessionId}", _session.Id);
    }

    public async Task<NavigationResult> NavigateAsync(string url, int timeoutMs = 30000)
    {
        if (CoreWebView2 == null)
            throw new InvalidOperationException("WebView2 not initialized");

        _session.Status = SessionStatus.Busy;
        _navigationTcs = new TaskCompletionSource<bool>();

        var startTime = DateTime.UtcNow;
        CoreWebView2.Navigate(url);

        using var cts = new CancellationTokenSource(timeoutMs);
        cts.Token.Register(() => _navigationTcs?.TrySetResult(false));

        var success = await _navigationTcs.Task;
        var loadTime = (DateTime.UtcNow - startTime).TotalMilliseconds;

        _session.Status = SessionStatus.Ready;
        _session.CurrentUrl = CoreWebView2.Source;

        return new NavigationResult
        {
            Success = success,
            FinalUrl = CoreWebView2.Source,
            LoadTimeMs = (int)loadTime
        };
    }

    public async Task<string> EvaluateScriptAsync(string script)
    {
        if (CoreWebView2 == null)
            throw new InvalidOperationException("WebView2 not initialized");

        _session.Touch();
        var result = await CoreWebView2.ExecuteScriptAsync(script);
        return result;
    }

    public async Task<byte[]> CaptureScreenshotAsync()
    {
        if (CoreWebView2 == null)
            throw new InvalidOperationException("WebView2 not initialized");

        _session.Touch();
        using var stream = new MemoryStream();
        await CoreWebView2.CapturePreviewAsync(CoreWebView2CapturePreviewImageFormat.Png, stream);
        return stream.ToArray();
    }

    public async Task<string> GetHtmlAsync()
    {
        var script = "document.documentElement.outerHTML";
        var result = await EvaluateScriptAsync(script);
        // Result is JSON-encoded string
        return JsonSerializer.Deserialize<string>(result) ?? "";
    }

    public async Task ClickAsync(string selector)
    {
        var script = $"document.querySelector('{EscapeSelector(selector)}')?.click()";
        await EvaluateScriptAsync(script);
    }

    public async Task TypeAsync(string selector, string text)
    {
        var escapedText = JsonSerializer.Serialize(text);
        var script = $@"
            const el = document.querySelector('{EscapeSelector(selector)}');
            if (el) {{
                el.focus();
                el.value = {escapedText};
                el.dispatchEvent(new Event('input', {{ bubbles: true }}));
                el.dispatchEvent(new Event('change', {{ bubbles: true }}));
            }}
        ";
        await EvaluateScriptAsync(script);
    }

    public async Task<List<CookieInfo>> GetCookiesAsync(string? domain = null)
    {
        if (CoreWebView2 == null)
            throw new InvalidOperationException("WebView2 not initialized");

        var cookieManager = CoreWebView2.CookieManager;
        var cookies = await cookieManager.GetCookiesAsync(domain ?? CoreWebView2.Source);
        
        return cookies.Select(c => new CookieInfo
        {
            Name = c.Name,
            Value = c.Value,
            Domain = c.Domain,
            Path = c.Path,
            Expires = c.Expires,
            HttpOnly = c.IsHttpOnly,
            Secure = c.IsSecure
        }).ToList();
    }

    private void OnNavigationStarting(object? sender, CoreWebView2NavigationStartingEventArgs e)
    {
        _logger.LogDebug("Navigation starting: {Uri}", e.Uri);
    }

    private void OnNavigationCompleted(object? sender, CoreWebView2NavigationCompletedEventArgs e)
    {
        _logger.LogDebug("Navigation completed: Success={Success}", e.IsSuccess);
        _navigationTcs?.TrySetResult(e.IsSuccess);
    }

    private void OnProcessFailed(object? sender, CoreWebView2ProcessFailedEventArgs e)
    {
        _logger.LogError("WebView2 process failed: {Kind}, {Reason}", e.ProcessFailedKind, e.Reason);
        _session.Status = SessionStatus.Error;
        _navigationTcs?.TrySetResult(false);
    }

    private static string EscapeSelector(string selector)
    {
        return selector.Replace("'", "\\'");
    }

    public void Dispose()
    {
        if (_disposed) return;
        _disposed = true;

        if (CoreWebView2 != null)
        {
            CoreWebView2.NavigationStarting -= OnNavigationStarting;
            CoreWebView2.NavigationCompleted -= OnNavigationCompleted;
            CoreWebView2.ProcessFailed -= OnProcessFailed;
        }

        _controller?.Close();
        _controller = null;
        _environment = null;

        _logger.LogInformation("WebViewInstance disposed for session {SessionId}", _session.Id);
    }
}

public class NavigationResult
{
    public bool Success { get; set; }
    public string FinalUrl { get; set; } = "";
    public int LoadTimeMs { get; set; }
    public int? StatusCode { get; set; }
}

public class CookieInfo
{
    public string Name { get; set; } = "";
    public string Value { get; set; } = "";
    public string Domain { get; set; } = "";
    public string Path { get; set; } = "/";
    public double Expires { get; set; }
    public bool HttpOnly { get; set; }
    public bool Secure { get; set; }
}
