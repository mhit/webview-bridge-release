using System;
using System.Collections.Concurrent;
using System.Threading;
using System.Threading.Tasks;
using Microsoft.Web.WebView2.Core;

namespace WebViewBridge.Core.WebView
{
    public sealed class WebViewPool : IAsyncDisposable, IDisposable
    {
        private readonly CoreWebView2Environment _environment;
        private readonly ConcurrentBag<CoreWebView2Controller> _pool = new();
        private readonly SemaphoreSlim _semaphore;

        public WebViewPool(CoreWebView2Environment environment, int maxInstances = 5)
        {
            if (maxInstances <= 0)
            {
                throw new ArgumentOutOfRangeException(nameof(maxInstances), "Max instances must be greater than zero.");
            }
            _environment = environment ?? throw new ArgumentNullException(nameof(environment));
            _semaphore = new SemaphoreSlim(maxInstances, maxInstances);
        }

        public async Task<WebView2ControllerWrapper> GetControllerAsync(CancellationToken cancellationToken = default)
        {
            await _semaphore.WaitAsync(cancellationToken);

            if (_pool.TryTake(out var controller))
            {
                return new WebView2ControllerWrapper(controller, this);
            }

            try
            {
                var newController = await CreateNewControllerAsync();
                return new WebView2ControllerWrapper(newController, this);
            }
            catch
            {
                _semaphore.Release();
                throw;
            }
        }

        internal async Task ReturnControllerAsync(CoreWebView2Controller controller)
        {
            try
            {
                // Reset the WebView2 state before returning to the pool
                await controller.CoreWebView2.NavigateAsync("about:blank");
                _pool.Add(controller);
            }
            catch
            {
                // If reset fails, the controller might be in a bad state. Dispose it.
                try
                {
                    controller.Close();
                }
                catch
                {
                    // Ignore close errors
                }
            }
            finally
            {
                _semaphore.Release();
            }
        }

        private async Task<CoreWebView2Controller> CreateNewControllerAsync()
        {
            // Note: Creating a CoreWebView2Controller typically requires a parent window handle (HWND).
            // This implementation assumes that it can be created with IntPtr.Zero for off-screen usage,
            // which is common in server-side or non-UI applications.
            return await _environment.CreateCoreWebView2ControllerAsync(IntPtr.Zero);
        }

        public async ValueTask DisposeAsync()
        {
            while (_pool.TryTake(out var controller))
            {
                try
                {
                    controller.Close();
                }
                catch
                {
                    // Ignore errors during disposal
                }
            }
            _semaphore.Dispose();
        }

        public void Dispose()
        {
            DisposeAsync().AsTask().GetAwaiter().GetResult();
        }
    }

    public sealed class WebView2ControllerWrapper : IAsyncDisposable
    {
        private WebViewPool? _pool;
        public CoreWebView2Controller Controller { get; }
        public CoreWebView2 CoreWebView2 => Controller.CoreWebView2;

        internal WebView2ControllerWrapper(CoreWebView2Controller controller, WebViewPool pool)
        {
            Controller = controller;
            _pool = pool;
        }

        public async ValueTask DisposeAsync()
        {
            var pool = Interlocked.Exchange(ref _pool, null);
            if (pool != null)
            {
                await pool.ReturnControllerAsync(Controller);
            }
        }
    }
}
