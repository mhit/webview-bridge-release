# WebView Bridge アーキテクチャ

> 最終更新: 2026-02-07

## 概要

WebView Bridgeは、Rust製のブラウザ自動化MCPサーバーです。Windows上でEdge WebView2を制御し、AIエージェント（Claude/Gemini等）からのMCPリクエストを処理します。

## システム構成図

```
┌─────────────────────────────────────────────────────────────────┐
│                     AI Agent (Claude/Gemini)                     │
└─────────────────────────────┬───────────────────────────────────┘
                              │ MCP Protocol (JSON-RPC over stdio)
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                    WebView Bridge Server                         │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │                    MCP Layer (mcp_v3/)                      │ │
│  │  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌────────────┐ │ │
│  │  │ navigate │  │ capture  │  │ interact │  │   agent    │ │ │
│  │  │          │  │          │  │          │  │ (AI loop)  │ │ │
│  │  └────┬─────┘  └────┬─────┘  └────┬─────┘  └─────┬──────┘ │ │
│  │       │             │             │              │        │ │
│  │  ┌────▼─────────────▼─────────────▼──────────────▼──────┐ │ │
│  │  │              Robustness Layer                         │ │ │
│  │  │  • Element visibility check                           │ │ │
│  │  │  • Auto-retry on failure                              │ │ │
│  │  │  • Human-like behavior (bezier, typos, inertia)       │ │ │
│  │  └────────────────────────┬─────────────────────────────┘ │ │
│  └───────────────────────────┼──────────────────────────────┘ │
│                              │                                 │
│  ┌───────────────────────────▼──────────────────────────────┐ │
│  │                  Session Manager (V2)                     │ │
│  │  • Named sessions (persistent across restarts)           │ │
│  │  • Cookie/profile management                              │ │
│  │  • WebView2 instance pool                                 │ │
│  └───────────────────────────┬──────────────────────────────┘ │
│                              │                                 │
│  ┌───────────────────────────▼──────────────────────────────┐ │
│  │                   WebView2 Wrapper                        │ │
│  │  • JavaScript execution                                   │ │
│  │  • Screenshot capture                                     │ │
│  │  • Navigation control                                     │ │
│  └───────────────────────────┬──────────────────────────────┘ │
└──────────────────────────────┼──────────────────────────────────┘
                               │
                               ▼
┌─────────────────────────────────────────────────────────────────┐
│                   Edge WebView2 Runtime                          │
│                   (Windows 10/11 built-in)                       │
└─────────────────────────────────────────────────────────────────┘
```

## ディレクトリ構成

```
src/
├── main.rs                 # エントリーポイント
├── config.rs               # 設定管理 (config.toml)
├── webview2/               # WebView2ラッパー
│   ├── mod.rs
│   ├── instance.rs         # WebView2インスタンス管理
│   └── scripts.rs          # JavaScript実行
│
├── api_v2.rs               # REST API (V2)
│
├── mcp_v3/                 # MCPツール実装
│   ├── mod.rs
│   ├── types.rs            # リクエスト/レスポンス型定義
│   ├── tools.rs            # MCPツールハンドラ
│   └── robustness.rs       # 堅牢性レイヤー
│
└── session_v2/             # セッション管理
    ├── mod.rs
    └── manager.rs          # SessionManagerV2
```

## 主要コンポーネント

### 1. MCP Layer (`mcp_v3/`)

MCPプロトコルでAIと通信するレイヤー。

| ファイル | 役割 |
|---------|------|
| `types.rs` | リクエスト/レスポンスの型定義（Serde対応） |
| `tools.rs` | 各MCPツール（navigate, capture, interact等）の実装 |
| `robustness.rs` | 堅牢性機能（リトライ、human_mode、JS生成） |

**MCPツール一覧:**
- `session` - セッション管理
- `navigate` - ページ遷移
- `capture` - スクリーンショット、DOM取得、AI要約
- `interact` - クリック、タイプ、スクロール
- `extract` - 構造化データ抽出
- `execute` - JavaScript実行
- `agent` - 自律型ブラウザ操作

### 2. Robustness Layer

Bot検出回避と安定性のための機能群。

```rust
// robustness.rs の主要関数
generate_human_mouse_move_script()    // ベジェ曲線マウス移動
generate_type_with_events_script_ex() // 人間らしいタイピング
generate_wait_for_clickable_script()  // 要素可視化待機
generate_scroll_and_click_script()    // スクロール＆クリック
```

**human_mode 機能:**
| 機能 | 説明 |
|------|------|
| ベジェ曲線移動 | 直線ではなく自然なカーブ |
| イージング | 加速→減速 |
| マイクロジッター | 手の震え |
| オーバーシュート | 10%で行き過ぎ→戻る |
| タイポ＆修正 | 3%で隣接キー打ち間違い |
| シフトミス | 大文字忘れ、次文字も大文字 |
| 慣性スクロール | 徐々に減速 |

### 3. Session Manager V2

セッションの永続化と管理。

```rust
// session_v2/manager.rs
pub struct SessionManagerV2 {
    sessions: HashMap<String, SessionV2>,
    data_dir: PathBuf,  // ~/.webview-bridge/sessions/
}

impl SessionManagerV2 {
    pub fn acquire(&mut self, name: &str) -> SessionV2;
    pub fn release(&mut self, name: &str);
    pub fn save_to_disk(&self);      // 永続化
    pub fn load_from_disk(&mut self); // 復元
}
```

**セッション永続化:**
- Cookie/ログイン状態を保持
- サーバー再起動後も復元可能
- プロファイル分離対応

### 4. WebView2 Wrapper

Windows WebView2 APIのRustラッパー。

```rust
// webview2/instance.rs
pub struct WebView2Instance {
    controller: ComPtr<ICoreWebView2Controller>,
    webview: ComPtr<ICoreWebView2>,
}

impl WebView2Instance {
    pub async fn navigate(&self, url: &str);
    pub async fn execute_script(&self, script: &str) -> String;
    pub async fn capture_screenshot(&self) -> Vec<u8>;
}
```

## データフロー

### 1. 通常のMCPリクエスト

```
AI → [navigate] → MCP Layer → Session Manager → WebView2 → Web Page
                     ↓
              Robustness Layer
              (wait, retry, human_mode)
```

### 2. Agent（自律操作）

```
AI → [agent goal="検索"] 
         ↓
    Agent Loop:
      1. capture → ページ状態取得
      2. AI分析 → 次のアクション決定
      3. execute_action → click/type/navigate
      4. 目標達成まで繰り返し
         ↓
    Result → AI
```

## 設定

`~/.webview-bridge/config.toml`:

```toml
[server]
host = "127.0.0.1"
port = 3030

[ai]
enabled = true
provider = "ollama"     # or "gemini"
model = "gpt-oss:20b"
# api_key = "..."       # Gemini用

[session]
default_timeout_ms = 30000
max_sessions = 10
```

## 技術スタック

| 項目 | 技術 |
|------|------|
| 言語 | Rust |
| 非同期ランタイム | Tokio |
| HTTP/WebSocket | Warp |
| JSON | serde_json |
| WebView2 | webview2 crate (Windows API) |
| AI通信 | reqwest (HTTP) |

## ビルド

```bash
# リリースビルド
cargo build --release

# 実行
cargo run --release

# テスト
cargo test
```

## 関連ドキュメント

- [README.md](../README.md) - プロジェクト概要
- [MCP_TOOLS.md](./MCP_TOOLS.md) - MCPツール詳細
- [API.md](./API.md) - REST API リファレンス
