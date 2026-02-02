namespace WebViewBridge.Core.Sessions;

public class SessionOptions
{
    public string Profile { get; set; } = "default";
    public bool Headless { get; set; } = true;
    public string? UserAgent { get; set; }
    public ViewportSize Viewport { get; set; } = new(1920, 1080);
    public int TimeoutMs { get; set; } = 30000;
}

public record ViewportSize(int Width, int Height);
