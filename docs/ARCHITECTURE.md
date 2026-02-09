# アーキテクチャ

WebView Bridge v3.5 (OpenClaw) の内部構造。

## レイヤー構成

```
┌─────────────────────────────────────────────────────────────┐
│  AI Client (Claude / Gemini / OpenClaw)                      │
│  MCP or REST API                                             │
└───────────────────────────┬─────────────────────────────────┘
                            │
┌───────────────────────────▼─────────────────────────────────┐
│  HTTP Server (Axum)                                          │
│  ├─ REST API (api_v2.rs)     POST /navigate, /click, ...    │
│  ├─ MCP Endpoint             POST /mcp                       │
│  ├─ Dashboard (SPA)          GET / (管理UI)                  │
│  └─ WebSocket                /ws (リアルタイムイベント)        │
└───────────────────────────┬─────────────────────────────────┘
                            │
┌───────────────────────────▼─────────────────────────────────┐
│  MCP Tools Layer (mcp_v3/)                                   │
│  ├─ tools.rs      ツールルーター (9ツール)                    │
│  ├─ types.rs      リクエスト/レスポンス型定義                 │
│  └─ robustness.rs 自動堅牢化 (待機/リトライ/スクロール)       │
└───────────────────────────┬─────────────────────────────────┘
                            │
┌───────────────────────────▼─────────────────────────────────┐
│  Core Layer (core/)                                          │
│  ├─ mod.rs          AppCommand enum + SessionManager         │
│  ├─ session_v2.rs   名前付きセッション管理                    │
│  ├─ config.rs       config.toml 設定管理                     │
│  ├─ ai.rs           AI統合 (Ollama / Gemini)                 │
│  ├─ goal.rs         Agentic モード実行                       │
│  ├─ network.rs      ネットワーク監視 (CDP)                   │
│  ├─ screenshot_v2.rs CDP スクリーンショット                   │
│  ├─ wait_v2.rs      DOM + Network 安定待機                   │
│  ├─ cookie_import.rs ブラウザCookieインポート                 │
│  ├─ media.rs        YouTube/画像メディア操作                  │
│  ├─ download.rs     ファイルダウンロード管理                  │
│  ├─ macro_engine.rs マクロ登録・実行                         │
│  ├─ comm.rs         プロセス間通信                           │
│  ├─ profile.rs      ユーザープロファイル管理                  │
│  └─ websocket.rs    WebSocket イベントハブ                   │
└───────────────────────────┬─────────────────────────────────┘
                            │
┌───────────────────────────▼─────────────────────────────────┐
│  WebView Layer (webview/)                                    │
│  ├─ webview_instance.rs  WebView2 インスタンス管理            │
│  ├─ window.rs            Win32 ウィンドウ管理                │
│  └─ mod.rs               CDP 統合 & JS 評価                 │
└───────────────────────────┬─────────────────────────────────┘
                            │ Chrome DevTools Protocol (CDP)
┌───────────────────────────▼─────────────────────────────────┐
│  Edge WebView2 Runtime                                       │
└─────────────────────────────────────────────────────────────┘
```

## スレッドモデル

```
┌──────────────────────────────────────────────────────────┐
│ Tokio Runtime (multi-thread)                              │
│                                                           │
│  ┌──────────────────┐  ┌──────────────────────────────┐  │
│  │ HTTP Server      │  │ Command Processor ×4          │  │
│  │ (Axum)           │  │ cmd_rx から AppCommand を     │  │
│  │                  │  │ 受信して非同期実行             │  │
│  │ リクエスト受信    │─▶│                              │  │
│  │ → AppCommand 生成│  │ CreateSession, Navigate,      │  │
│  │ → cmd_tx に送信  │  │ ExecuteScript, Snapshot, ...  │  │
│  └──────────────────┘  └──────────────┬───────────────┘  │
│                                        │                  │
│  ┌──────────────────┐                  │                  │
│  │ Auto-Cleanup     │                  │                  │
│  │ Timer (60s間隔)  │                  │                  │
│  │ アイドル検出      │                  │                  │
│  │ → セッション解放  │                  │                  │
│  └──────────────────┘                  │                  │
└────────────────────────────────────────┼──────────────────┘
                                         │
                    ┌────────────────────▼──────────────────┐
                    │ WebView2 (各セッションに1インスタンス)  │
                    │ Win32 ウィンドウスレッド                │
                    │ CDP 経由でブラウザ制御                  │
                    └──────────────────────────────────────┘
```

### コマンドフロー

1. **HTTPリクエスト受信** — Axum ハンドラが `AppCommand` を生成
2. **チャネル送信** — `mpsc::channel` (容量1000) 経由で送信
3. **コマンドプロセッサ** — 4ワーカーが並列で受信・実行
4. **WebView操作** — `SessionManager` → `WebViewInstance` → CDP
5. **レスポンス返却** — `oneshot::channel` で呼び出し元に結果を返す

## ダッシュボードアーキテクチャ

### Single-File SPA

ダッシュボードは `src/dashboard.html` (約2,400行) に CSS/HTML/JS を統合した単一ファイルです。

- **ビルド時**: `include_str!("dashboard.html")` で `api_v2.rs` にバイナリ埋め込み
- **起動時**: `{{AUTH_TOKEN}}` プレースホルダを実トークンに置換して配信
- **クライアント側**: `window.__WB_TOKEN` でAPI認証を自動処理

### 構成

```
src/dashboard.html (2,400行)
├── CSS    (1-900行)   デザインシステム、レスポンシブ、アニメーション
├── HTML   (900-1635行) 11ページ、ダイアログ、ライトボックス
└── JS     (1670-2400行) 50+関数、API連携、状態管理
```

### ページ構成 (11ページ)

| ID | ページ |
|----|--------|
| `pg-dash` | システム概要 |
| `pg-sess` | セッション管理 |
| `pg-ss` | スクリーンショット |
| `pg-dl` | ダウンロード |
| `pg-ai` | AI設定 |
| `pg-srv` | サーバー設定 |
| `pg-med` | メディア管理 |
| `pg-test` | APIテスター |
| `pg-mcp` | MCPツール |
| `pg-auto` | オートメーション |
| `pg-aitool` | AIツール |

### 状態管理

- `SC` (Session Cache): セッション一覧のグローバルキャッシュ
- `fillAllSessSelects()`: 11個のセッション選択ドロップダウンを同期
- `loadAll()`: 6並列API fetch + 排他制御 (`_loading`フラグ)
- 10秒間隔のポーリング（`visibilitychange`でタブ非表示時は停止）

### イベント処理

- Event Delegation: `data-action` 属性による動的要素のクリック処理
- 非同期ハンドラ: `async/await` + `try/catch` でエラー安全

## CDP 統合

v3.5 で Chrome DevTools Protocol (CDP) に移行。

### CDP の役割

| 機能 | CDP メソッド |
|------|-------------|
| スクリーンショット | `Page.captureScreenshot` |
| フルページキャプチャ | `Page.getLayoutMetrics` + `Emulation.setDeviceMetricsOverride` |
| ネットワーク監視 | `Network.enable`, `Network.requestWillBeSent` |
| DOM安定待機 | DOM MutationObserver + XHR/fetch 追跡 |
| JavaScript実行 | `Runtime.evaluate` |
| デバイスエミュレーション | `Emulation.setDeviceMetricsOverride`, `Network.setUserAgentOverride` |

### スクリーンショットURI

スクリーンショットは `browser://` URI で返却（OS非依存）:

```
browser://screenshots/{session}/{filename}
```

- AIコンテキストにはURIのみ（base64埋め込みなし）
- HTTP取得: `GET /media/screenshots/{session}/{filename}`

## データ永続化

| データ | 保存先 | 説明 |
|--------|--------|------|
| config.toml | `%APPDATA%/webview-bridge/` | アプリ設定 |
| セッション状態 | 同上 + SQLite | Cookie/URL履歴 |
| スクリーンショット | temp dir | 一時保存、TTL管理 |
| マクロ定義 | データディレクトリ | 登録済みマクロ |

## セッション管理

```
SessionManagerV2 (名前付きセッション)
  ├─ acquire("name") → SessionHandle
  ├─ release("name") → 状態保存
  ├─ restore("name") → Cookie/状態復元
  └─ auto_suspend_idle(300s) → アイドルセッション自動解放

SessionManager (内部ID管理)
  └─ WebViewInstance ×N (最大20)
```

- **TTL管理**: デフォルト168時間（1週間）、0で無期限
- **自動サスペンド**: 5分アイドルでWebViewウィンドウ解放
- **状態復元**: `restore: true` で前回のCookie/URL を復元
