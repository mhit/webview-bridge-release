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

## MCP クライアントからの接続

### Claude Desktop / OpenClaw

MCP設定ファイルに以下を追加:

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

### REST API 直接利用

```bash
# ヘルスチェック
curl http://localhost:9400/health

# セッション作成
curl -X POST http://localhost:9400/session/acquire \
  -H "Content-Type: application/json" \
  -d '{"name": "my-session"}'
```

### MCP エンドポイント

```bash
# MCPツール呼び出し
curl -X POST http://localhost:9400/mcp \
  -H "Content-Type: application/json" \
  -d '{"tool": "navigate", "session": "my-session", "url": "https://example.com"}'

# 利用可能ツール一覧
curl http://localhost:9400/mcp/tools
```

## 最初のブラウザ操作

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
