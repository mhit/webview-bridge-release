#!/bin/bash
# Gemini Implementation Script with Hook notification

HOOK_URL="http://localhost:18789/api/hook"
PROJECT_DIR=~/webview-bridge
OUTPUT_DIR="$PROJECT_DIR/gemini-output"
mkdir -p "$OUTPUT_DIR"

notify_hook() {
    local message="$1"
    curl -s -X POST "$HOOK_URL" \
        -H "Content-Type: application/json" \
        -d "{\"text\": \"$message\"}" > /dev/null 2>&1
}

notify_hook "🚀 Gemini実装開始: WebView Bridge 残り部分"

# Task 1: WebViewPool実装
echo "=== Task 1: WebViewPool ===" 
gemini "
C# .NET 8でWebView2のプール管理クラスを実装してください。

要件:
- namespace: WebViewBridge.Core.WebView
- 最大5インスタンスのプール
- アイドルインスタンスの再利用
- 使用後のリセット処理
- IDisposable実装
- async/await対応

ファイル名: WebViewPool.cs
完全なコードのみ出力（説明不要）
" > "$OUTPUT_DIR/WebViewPool.cs" 2>&1

notify_hook "✅ Task 1/4 完了: WebViewPool.cs"

# Task 2: ProfileManager実装
echo "=== Task 2: ProfileManager ==="
gemini "
C# .NET 8でWebView2のプロファイル管理クラスを実装してください。

要件:
- namespace: WebViewBridge.Core.Profiles
- プロファイル設定の読み書き（JSON）
- UserDataFolder管理（%LOCALAPPDATA%/WebViewBridge/profiles/{name}）
- デフォルトプロファイル
- ILogger使用

ファイル: Profile.cs, ProfileManager.cs
完全なコードのみ出力
" > "$OUTPUT_DIR/ProfileManager.cs" 2>&1

notify_hook "✅ Task 2/4 完了: ProfileManager.cs"

# Task 3: OpenClaw互換API
echo "=== Task 3: OpenClaw API ==="
gemini "
ASP.NET Core Minimal APIでOpenClaw browser tool互換のエンドポイントを実装してください。

エンドポイント:
- POST /api/openclaw/snapshot - ARIAスナップショット取得
- POST /api/openclaw/act - アクション実行(click, type, press等)

リクエスト/レスポンス形式はOpenClawのbrowserツールに合わせる。
namespace: WebViewBridge.Api

完全なコードのみ出力
" > "$OUTPUT_DIR/OpenClawApi.cs" 2>&1

notify_hook "✅ Task 3/4 完了: OpenClawApi.cs"

# Task 4: WPF Host Window
echo "=== Task 4: WPF Host ==="
gemini "
C# WPF (.NET 8)でWebView2をホストするための最小ウィンドウを実装してください。

要件:
- 非表示ウィンドウ（ヘッドレス動作用）
- WebView2コントロールをホスト
- 複数WebView2インスタンス対応
- APIサーバーと連携
- トレイアイコン（オプション）

ファイル: App.xaml, App.xaml.cs, MainWindow.xaml, MainWindow.xaml.cs
完全なコードのみ出力
" > "$OUTPUT_DIR/WpfHost.cs" 2>&1

notify_hook "✅ Task 4/4 完了: WpfHost.cs"

# 完了通知
notify_hook "🎉 Gemini実装完了！出力: ~/webview-bridge/gemini-output/
- WebViewPool.cs
- ProfileManager.cs  
- OpenClawApi.cs
- WpfHost.cs

レビューしてマージしてください。"

echo "=== All tasks completed ==="
ls -la "$OUTPUT_DIR"
