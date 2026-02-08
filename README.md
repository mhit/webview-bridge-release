<p align="center">
  <img src="docs/icon-128.png" alt="WebView Bridge" width="96">
</p>

<h1 align="center">WebView Bridge</h1>

<p align="center">
  <strong>AI MCPクライアント向け ブラウザ自動操作サーバー</strong><br>
  <sub>Windows 専用 / WebView2 ベース / CDP Native</sub>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/version-3.5.0-blue" alt="Version">
  <img src="https://img.shields.io/badge/platform-Windows%2010%2F11-0078d4" alt="Platform">
  <img src="https://img.shields.io/badge/protocol-MCP-purple" alt="MCP">
  <img src="https://img.shields.io/badge/license-MIT-green" alt="License">
</p>

---

## WebView Bridge とは

WebView Bridge は、**AI エージェント (Claude, Gemini 等) が実際のブラウザを操作する**ための MCP (Model Context Protocol) サーバーです。

- ブラウザセッションの作成・管理 (Cookie 永続化)
- ページ遷移・クリック・入力・スクロール
- スクリーンショット・DOM キャプチャ
- 構造化データ抽出
- ネットワーク監視
- AI による自律ブラウジング (Agentic モード)

システムトレイに常駐し、バックグラウンドで動作します。

---

## ダッシュボード

ブラウザで `http://localhost:9400` にアクセスすると、管理ダッシュボードが表示されます。

### メイン画面

セッション数・AI 状態・スクリーンショット数などの概要を一覧表示。

![ダッシュボード](docs/screenshots/01-dashboard.png)

### セッション管理

ブラウザセッションの新規作成・起動・クローン・削除。Cookie はセッション間で永続化されます。

![セッション管理](docs/screenshots/02-sessions.png)

### AI 設定

LLM プロバイダー (Ollama / Google Gemini) の設定。ページ要約や自律操作に使用します。

![AI設定](docs/screenshots/03-ai-config.png)

### サーバー設定

HTTP サーバーのバインドアドレス・ポート・最大セッション数・デフォルトビューポートなどの設定。

![サーバー設定](docs/screenshots/04-server-config.png)

### MCP ツール一覧

利用可能な 9 つの MCP ツールとそのパラメータを確認できます。

![MCPツール](docs/screenshots/05-mcp-tools.png)

---

## インストール

### インストーラー (推奨)

1. [Releases](https://github.com/mhit/webview-bridge-release/releases) から `WebViewBridge-3.5.0-Setup.exe` をダウンロード
2. インストーラーを実行
3. インストール完了後、自動的にシステムトレイに常駐

### 前提条件

- **Windows 10/11** (64-bit)
- **WebView2 Runtime** - 通常 Microsoft Edge と共にインストール済み

---

## 使い方

### 起動

インストーラーでインストールした場合、スタートメニューまたはデスクトップのショートカットから起動できます。
起動するとシステムトレイにアイコンが表示されます。

- **右クリック → ダッシュボードを開く**: ブラウザでダッシュボードを表示
- **右クリック → 終了**: サーバーを停止

### コマンドラインオプション

```
webview-bridge-rust.exe [OPTIONS]

Options:
  --bind <IP>       バインドアドレス (デフォルト: 0.0.0.0)
  --port <PORT>     ポート番号 (デフォルト: 9400)
  --help, -h        ヘルプを表示
```

---

## MCP クライアント設定

### Claude Desktop / Cline / その他 MCP クライアント

MCP 設定ファイルに以下を追加してください:

#### HTTP (StreamableHTTP) 接続 — 推奨

```json
{
  "mcpServers": {
    "webview-bridge": {
      "serverUrl": "http://127.0.0.1:9400/mcp"
    }
  }
}
```

#### stdio 接続

```json
{
  "mcpServers": {
    "webview-bridge": {
      "command": "webview-bridge-rust.exe",
      "args": ["--mcp-stdio"]
    }
  }
}
```

> **Note**: stdio 接続の場合、exe のパスを PATH に通すか、フルパスで指定してください。

---

## MCP ツール

| ツール | 説明 |
|--------|------|
| `session` | セッション管理 (作成・削除・一覧・Cookie インポート) |
| `navigate` | URL 遷移 (load/stable/networkidle/selector 待機) |
| `interact` | ブラウザ操作 (click/type/scroll/hover/select/wait) |
| `capture` | ページ状態キャプチャ (スクリーンショット・DOM・要素一覧) |
| `extract` | 構造化データ抽出 (CSS セレクタベース) |
| `execute` | JavaScript 実行 |
| `media` | メディア操作 (YouTube ダウンロード・画像収集) |
| `agent` | AI 自律ブラウジング (ゴールベース) |
| `network` | ネットワーク監視 (リクエスト/レスポンスキャプチャ) |

### 使用例

#### ページ遷移 + スクリーンショット

```json
// 1. セッション作成
{"tool": "session", "acquire": "demo"}

// 2. ページ遷移
{"tool": "navigate", "session": "demo", "url": "https://example.com", "wait_for": "stable"}

// 3. スクリーンショット取得
{"tool": "capture", "session": "demo", "screenshot": true}
```

#### 検索操作 (複数アクション)

```json
{
  "tool": "interact",
  "session": "demo",
  "actions": [
    {"type": "type", "target": "textarea[name=q]", "value": "WebView Bridge"},
    {"type": "click", "target": "input[name=btnK]"},
    {"type": "wait", "condition": "network_idle", "timeout_ms": 5000}
  ]
}
```

#### Bot 検知対策 (human_mode)

```json
{
  "tool": "interact",
  "session": "demo",
  "actions": [
    {"type": "click", "target": "a.product-link"}
  ],
  "options": {"human_mode": true}
}
```

> `human_mode: true` を指定すると、ベジェ曲線のマウス移動・タイプミス・ランダム遅延で人間らしい操作を行います。

---

## config.toml

データディレクトリ (`%APPDATA%\webview-bridge\`) に `config.toml` を配置することで設定をカスタマイズできます。
インストーラーがサンプルを自動生成します。

```toml
[server]
bind = "0.0.0.0"
port = 9400
max_sessions = 10

[ai]
enabled = false
provider = "ollama"       # "ollama" or "gemini"
model = "gemma3:12b"      # Ollama のモデル名
# api_key = "your-key"    # Gemini 使用時に必要
# timeout_ms = 30000
# daily_budget_usd = 1.0

[session]
# default_ttl_hours = 168  # セッション有効期間 (デフォルト: 1週間)

[media]
# download_dir = "downloads"
```

---

## REST API

MCP 経由でなく、REST API を直接利用することもできます。

```bash
# ヘルスチェック
curl http://localhost:9400/health

# セッション作成
curl -X POST http://localhost:9400/session/acquire \
  -H "Content-Type: application/json" \
  -d '{"name": "my-session"}'

# セッション一覧
curl http://localhost:9400/session/list

# MCP ツール呼び出し
curl -X POST http://localhost:9400/mcp \
  -H "Content-Type: application/json" \
  -d '{"tool": "navigate", "session": "my-session", "url": "https://example.com"}'
```

---

## アンインストール

1. システムトレイのアイコンを右クリック → **終了**
2. Windows の **設定 → アプリ → WebView Bridge** からアンインストール
3. アンインストーラーで「セッションデータも削除」を選択すると Cookie 等も削除されます

---

## ライセンス

MIT License

---

<p align="center">
  <sub>Built with Rust + WebView2 + CDP</sub>
</p>
