# クイックスタート

WebView Bridge を5分で始める。

## 前提条件

- **Windows 10/11** (WebView2 ランタイムが必要)
- **Rust** (edition 2024 / nightly 推奨)
- **WebView2 Runtime** - 通常 Edge と共にインストール済み

## ビルド & 起動

```bash
# ビルド
cargo build --release

# 起動（デフォルト: 0.0.0.0:9400）
cargo run --release

# ポート指定
cargo run --release -- --port 3030

# バインドアドレス指定
cargo run --release -- --bind 127.0.0.1 --port 9400
```

起動すると以下が表示される:

```
WebView Bridge Server v2 starting...
listening on 0.0.0.0:9400 with 4 command processors
API: http://0.0.0.0:9400/
```

## config.toml 設定

データディレクトリ（`%APPDATA%/webview-bridge/` 等）に `config.toml` を配置:

```toml
[server]
bind = "0.0.0.0"
port = 9400
max_sessions = 10

[ai]
enabled = true
provider = "ollama"       # "ollama" or "gemini"
model = "gemma3:12b"      # ローカルLLM
# api_key = "your-key"    # Gemini使用時に必要

[session]
# セッションのデフォルト設定

[media]
# メディア/ダウンロード設定
```

## アクセス方法

WebView Bridge には3つのアクセス方法があります：

### 方法1: ダッシュボード (推奨)

ブラウザで `http://localhost:9400` にアクセスします。

- 11ページのWebインターフェース
- セッション管理、スクリーンショット、ダウンロード、AI設定
- オートメーション（ブラウザ操作、Goal、マクロ）
- MCPツール実行、バッチAPI

### 方法2: MCP クライアント

Claude Desktop等のMCPクライアントから接続します（下記の設定を参照）。

### 方法3: REST API

curlや各言語のHTTPクライアントから直接APIを呼び出します。

---

## MCP クライアントからの接続

WebView Bridge は **stdio** と **HTTP/SSE** の両方でMCPプロトコルに対応しています。

---

### 方式1: stdio（ローカル実行）

WebView Bridge の exe を子プロセスとして直接起動する方式。同一マシンでクライアントとサーバーが動作する場合に使用。

**Claude Desktop / Cline:**

`%APPDATA%\Claude\claude_desktop_config.json`:

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

> **Note:** exe にパスが通っていない場合はフルパスを指定:
> `"command": "C:\\Program Files\\WebViewBridge\\webview-bridge-rust.exe"`

**Claude Code（`.mcp.json`）:**

プロジェクトルートの `.mcp.json`:

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

---

### 方式2: HTTP/SSE（リモート接続）

別マシンで動作している WebView Bridge サーバーに接続する方式。LAN内の他のPC、WSL、AIエージェントなどからの利用に最適。

**前提:** サーバー側の config.toml で `bind = "0.0.0.0"` に設定し、LAN内からアクセス可能にしておく。

```toml
[server]
bind = "0.0.0.0"   # ← 127.0.0.1 だとローカルのみ
port = 9400
```

#### Claude Desktop（mcp-remote 経由）

> **既知の問題:** Claude Desktop v1.1.2998 では `"url"` 形式の MCP 設定を使うと起動時にクラッシュします（`TypeError: Cannot read properties of undefined (reading 'value')`）。`mcp-remote` をブリッジとして使用してください。

`%APPDATA%\Claude\claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "webview-bridge": {
      "command": "npx",
      "args": [
        "-y",
        "mcp-remote",
        "http://<サーバーIP>:9400/mcp",
        "--allow-http"
      ]
    }
  }
}
```

| パラメータ | 説明 |
|-----------|------|
| `npx -y` | mcp-remote を自動インストール・実行 |
| `mcp-remote` | リモート HTTP/SSE サーバーへのブリッジ |
| `--allow-http` | **必須** — HTTP接続を許可（デフォルトはHTTPSのみ） |

> **設定変更時の注意:** Claude Desktop 起動中に設定ファイルを変更すると上書きされます。必ず完全終了してから編集してください。

#### Claude Code（リモート URL）

`.mcp.json`:

```json
{
  "mcpServers": {
    "webview-bridge": {
      "url": "http://<サーバーIP>:9400/mcp"
    }
  }
}
```

#### OpenClaw

`openclaw.json` の MCP 設定:

```json
{
  "mcp_servers": {
    "webview-bridge": {
      "url": "http://<サーバーIP>:9400/mcp"
    }
  }
}
```

---

### 方式3: REST API 直接利用

MCP を使わず、curl や HTTP クライアントから直接 API を呼び出す方式。

```bash
# ヘルスチェック
curl http://localhost:9400/health

# セッション作成
curl -X POST http://localhost:9400/session/acquire \
  -H "Content-Type: application/json" \
  -d '{"name": "my-session"}'
```

### MCP HTTP エンドポイント

```bash
# MCPツール呼び出し
curl -X POST http://localhost:9400/mcp \
  -H "Content-Type: application/json" \
  -d '{"tool": "navigate", "session": "my-session", "url": "https://example.com"}'

# 利用可能ツール一覧
curl http://localhost:9400/mcp/tools
```

---

### 接続方式の選び方

| 条件 | 推奨方式 | 理由 |
|------|---------|------|
| 同一PCで利用 | stdio | セットアップ不要、最もシンプル |
| LAN内の別マシンから | HTTP/SSE | ネットワーク越しに接続可能 |
| WSLからWindowsへ | HTTP/SSE | WSLとWindows間はネットワーク経由 |
| AIエージェント（Sam等） | HTTP/SSE | サーバーが常時起動、複数クライアント対応 |
| Claude Desktop リモート | mcp-remote | `url` 設定のクラッシュバグ回避 |

## 最初のブラウザ操作

> ダッシュボードを使う場合、下記のAPIコールは不要です。
> `http://localhost:9400` → セッション管理ページから視覚的に操作できます。

### 1. セッション作成 → ページ遷移 → スクリーンショット

```bash
# セッション作成
curl -X POST http://localhost:9400/mcp \
  -H "Content-Type: application/json" \
  -d '{"tool": "session", "acquire": "demo"}'

# Googleに遷移
curl -X POST http://localhost:9400/mcp \
  -H "Content-Type: application/json" \
  -d '{"tool": "navigate", "session": "demo", "url": "https://www.google.com"}'

# スクリーンショット取得
curl -X POST http://localhost:9400/mcp \
  -H "Content-Type: application/json" \
  -d '{"tool": "capture", "session": "demo", "screenshot": true}'
```

### 2. 検索操作（複数アクションを1回で）

```bash
curl -X POST http://localhost:9400/mcp \
  -H "Content-Type: application/json" \
  -d '{
    "tool": "interact",
    "session": "demo",
    "actions": [
      {"type": "type", "target": "textarea[name=q]", "value": "WebView Bridge"},
      {"type": "click", "target": "input[name=btnK]"},
      {"type": "wait", "condition": "network_idle", "timeout_ms": 5000}
    ]
  }'
```

### 3. Agenticモード（AI自律操作）

```bash
curl -X POST http://localhost:9400/mcp \
  -H "Content-Type: application/json" \
  -d '{
    "tool": "agent",
    "session": "demo",
    "action": {
      "type": "start",
      "goal": "Amazonでワイヤレスマウスを検索して最初の5件の商品名と価格を教えて",
      "max_steps": 15,
      "human_mode": true
    }
  }'
```

## 次のステップ

- [MCPツール詳細](MCP_TOOLS.md) - 9ツールの全パラメータ
- [REST API](API.md) - REST APIリファレンス
- [アーキテクチャ](ARCHITECTURE.md) - 内部構造の理解
