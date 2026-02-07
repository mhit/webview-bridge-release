# WebView Bridge (Rust Edition) - Design Document

## 🎯 Project Goals
Windowsネイティブで動作する、極めて軽量かつ堅牢なWebView2操作ブリッジ。
WSL2上のOpenClawからHTTP経由でブラウザを操作し、ボット検知を回避する。

## 🛠 Tech Stack
- **Language:** Rust
- **Framework:** `axum` (HTTP Server)
- **WebView2 Binding:** `webview2-com` (Direct COM access)
- **Async Runtime:** `tokio` (Multi-threaded)
- **Windows Integration:** `windows-rs` (Standard Win32 API access)

## 🏗 Architecture Detail (Multi-threaded Bridge)

### 1. Main Thread (UI/Message Loop)
WebView2はWindowsのメッセージループ（STA: Single Threaded Apartment）を必要とするため、ブラウザインスタンスはメインスレッド、または専用のUIスレッドで管理する。
- **Win32 Message Loop:** `GetMessage`/`DispatchMessage` を回し、WebView2のイベントを処理。
- **WebView2 Controller:** ウィンドウの生成、リサイズ、可視性（Headless/Visible）の制御。

### 2. Worker Threads (Axum/HTTP)
WSL2からのリクエストを待機するスレッド。
- **Request Routing:** `axum` がリクエストを受け取り、`tokio::sync::mpsc` チャネルを通じてUIスレッドへ指示を送る。
- **Response Handling:** UIスレッドで実行された結果（スクショ、HTML等）を `oneshot` チャネルで受け取り、HTTPレスポンスとして返す。

## 🚀 Internal Components

### A. Session Manager
セッションID（UUID）とWebView2インスタンスのマッピングを管理。
- **User Data Folder:** セッションごとに隔離されたディレクトリ（Cookie/LocalStorage）を管理し、並列実行を可能にする。
- **Auto Cleanup:** 一定時間アクセスのないセッションを自動破棄。

### B. Command Protocol
HTTPリクエストをWebView2の内部コマンドに変換。
- `Navigate(url)`
- `ExecuteScript(js)`
- `CaptureScreenshot`
- `ClickElement(selector)`

## 📋 API Specification (Technical)

### Session Lifecycle
- `POST /api/sessions`: セッション作成。オプションで `profile_path`, `headless` を指定。
- `DELETE /api/sessions/{id}`: セッション終了、プロセスと一時ファイルのクリーンアップ。

### Content & Interaction
- `POST /api/sessions/{id}/navigate`: 指定したURLへ移動。
- `GET /api/sessions/{id}/content`: `document.documentElement.outerHTML` を取得。
- `POST /api/sessions/{id}/action`: 
  ```json
  {
    "type": "click",
    "selector": "button#submit",
    "delay_ms": 100
  }
  ```

## 🔒 Security
- **Token Auth:** `X-Bridge-Token` ヘッダーによる簡易認証。
- **CORS:** WSL2のIPアドレスからのアクセスのみを許可。

## 🧪 Challenges & Mitigation
- **STA Threading:** Rustの `tokio` と Win32 のメッセージループの共存。
  - → `tokio::spawn_blocking` または専用の `std::thread` でメッセージループを回す。
- **Headless Mode:** WebView2は完全なヘッドレス（プロセスのみ）ではない。
  - → ウィンドウを `WS_EX_TOOLWINDOW` で作成し、画面外に配置または透明化することで「擬似ヘッドレス」を実現。
