using System;
using System.Drawing;
using System.Windows;
using System.Windows.Forms;
using Application = System.Windows.Application;

namespace WpfWebView2Headless
{
    /// <summary>
    /// Interaction logic for App.xaml
    /// </summary>
    public partial class App : Application
    {
        private MainWindow? _mainWindow;
        private NotifyIcon? _notifyIcon;

        /// <summary>
        /// アプリケーション起動時の処理
        /// </summary>
        protected override void OnStartup(StartupEventArgs e)
        {
            base.OnStartup(e);

            // MainWindowをインスタンス化するが、Show()は呼ばない。
            // これによりウィンドウは非表示のままバックグラウンドで動作する。
            _mainWindow = new MainWindow();

            // トレイアイコンの設定 (オプション)
            SetupTrayIcon();
        }

        /// <summary>
        /// トレイアイコンをセットアップします。
        /// </summary>
        private void SetupTrayIcon()
        {
            _notifyIcon = new NotifyIcon
            {
                Icon = SystemIcons.Application, // 適切なアイコンに変更してください
                Visible = true,
                Text = "WebView2 Headless Host"
            };

            var contextMenu = new ContextMenuStrip();
            contextMenu.Items.Add("Exit", null, OnExitClicked);
            _notifyIcon.ContextMenuStrip = contextMenu;
        }

        /// <summary>
        /// トレイアイコンの「Exit」がクリックされたときの処理
        /// </summary>
        private void OnExitClicked(object? sender, EventArgs e)
        {
            // アプリケーションを正常にシャットダウンする
            Shutdown();
        }

        /// <summary>
        /// アプリケーション終了時の処理
        /// </summary>
        protected override void OnExit(ExitEventArgs e)
        {
            // リソースをクリーンアップ
            _mainWindow?.Close();
            _notifyIcon?.Dispose();
            _notifyIcon = null;

            base.OnExit(e);
        }
    }
}
```
```csharp
using Microsoft.AspNetCore.Builder;
using Microsoft.AspNetCore.Http;
using Microsoft.Extensions.Hosting;
using Microsoft.Web.WebView2.Core;
using Microsoft.Web.WebView2.Wpf;
using System;
using System.Collections.Concurrent;
using System.ComponentModel;
using System.Text.Json;
using System.Threading.Tasks;
using System.Windows;

namespace WpfWebView2Headless
{
    /// <summary>
    /// WebView2インスタンスをホストし、APIサーバーを起動する非表示のメインウィンドウ。
    /// </summary>
    public partial class MainWindow : Window
    {
        // スレッドセーフなDictionaryでWebView2インスタンスを管理する
        private readonly ConcurrentDictionary<string, WebView2> _webViews = new();
        private IHost? _apiHost;

        public MainWindow()
        {
            InitializeComponent();
            
            // ウィンドウを完全に非表示にする
            this.Visibility = Visibility.Hidden;

            // アプリケーションの起動時にAPIサーバーをバックグラウンドで開始
            Loaded += async (sender, e) => await StartApiServer();
        }

        /// <summary>
        /// ASP.NET Core Minimal APIサーバーを起動します。
        /// </summary>
        private async Task StartApiServer()
        {
            var builder = WebApplication.CreateBuilder();
            var app = builder.Build();

            // APIの定義
            // 新しいWebView2インスタンスを作成
            app.MapPost("/webview", CreateWebViewAsync);

            // 指定したIDのWebView2でページを読み込む
            app.MapPost("/webview/{id}/navigate", NavigateAsync);
            
            // 指定したIDのWebView2でJavaScriptを実行
            app.MapPost("/webview/{id}/execute", ExecuteScriptAsync);

            // 指定したIDのWebView2を破棄
            app.MapDelete("/webview/{id}", DeleteWebViewAsync);

            _apiHost = app;
            await app.RunAsync("http://localhost:5123"); // ポートは環境に合わせて変更してください
        }

        // POST /webview
        private async Task<IResult> CreateWebViewAsync()
        {
            var id = Guid.NewGuid().ToString();
            WebView2? webView = null;

            // WebView2はUIスレッドで作成する必要がある
            await Application.Current.Dispatcher.InvokeAsync(async () =>
            {
                webView = new WebView2
                {
                    // ウィンドウが表示されないようにサイズを0に設定
                    Width = 0,
                    Height = 0
                };
                
                // WebView2の初期化
                await webView.EnsureCoreWebView2Async(null);
                _webViews[id] = webView;
            });
            
            return Results.Ok(new { id });
        }
        
        // POST /webview/{id}/navigate
        private async Task<IResult> NavigateAsync(string id, HttpRequest request)
        {
            if (!_webViews.TryGetValue(id, out var webView))
            {
                return Results.NotFound(new { error = $"WebView with id '{id}' not found." });
            }

            var body = await new System.IO.StreamReader(request.Body).ReadToEndAsync();
            var payload = JsonSerializer.Deserialize<NavigationPayload>(body);

            if (payload?.Url == null)
            {
                return Results.BadRequest(new { error = "URL is required." });
            }

            // UIスレッドでナビゲーションを実行
            await Application.Current.Dispatcher.InvokeAsync(() =>
            {
                webView.CoreWebView2.Navigate(payload.Url);
            });

            return Results.Ok();
        }

        // POST /webview/{id}/execute
        private async Task<IResult> ExecuteScriptAsync(string id, HttpRequest request)
        {
            if (!_webViews.TryGetValue(id, out var webView))
            {
                return Results.NotFound(new { error = $"WebView with id '{id}' not found." });
            }
            
            var body = await new System.IO.StreamReader(request.Body).ReadToEndAsync();
            var payload = JsonSerializer.Deserialize<ScriptPayload>(body);

            if (payload?.Script == null)
            {
                return Results.BadRequest(new { error = "Script is required." });
            }

            string? result = null;
            // UIスreadでスクリプトを実行
            await Application.Current.Dispatcher.InvokeAsync(async () =>
            {
                result = await webView.CoreWebView2.ExecuteScriptAsync(payload.Script);
            });
            
            return Results.Ok(new { result });
        }

        // DELETE /webview/{id}
        private async Task<IResult> DeleteWebViewAsync(string id)
        {
            if (!_webViews.TryRemove(id, out var webView))
            {
                return Results.NotFound(new { error = $"WebView with id '{id}' not found." });
            }

            // UIスレッドでリソースを破棄
            await Application.Current.Dispatcher.InvokeAsync(() =>
            {
                webView.Dispose();
            });

            return Results.NoContent();
        }

        /// <summary>
        /// ウィンドウが閉じられるときのクリーンアップ処理
        /// </summary>
        protected override async void OnClosing(CancelEventArgs e)
        {
            // APIサーバーを停止
            if (_apiHost != null)
            {
                await _apiHost.StopAsync();
            }

            // すべてのWebView2インスタンスを破棄
            foreach (var webView in _webViews.Values)
            {
                webView.Dispose();
            }
            _webViews.Clear();

            base.OnClosing(e);
        }

        // APIリクエストのペイロード用レコード
        private record NavigationPayload(string Url);
        private record ScriptPayload(string Script);
    }
}
