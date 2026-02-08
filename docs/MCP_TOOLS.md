# MCP ツールリファレンス

WebView Bridge は9つのMCPツールを提供する。全て `POST /mcp` エンドポイント経由で呼び出す。

```json
{"tool": "<ツール名>", ...パラメータ}
```

## レスポンス形式

全ツール共通:

```json
{
  "success": true,
  "content": [
    {"type": "text", "text": "..."},
    {"type": "image", "data": "base64...", "mime_type": "image/png"}
  ],
  "error": null
}
```

エラー時:

```json
{
  "success": false,
  "error": {"code": "ERROR_CODE", "message": "説明", "details": {...}}
}
```

---

## 1. session — セッション管理

セッションの作成・解放・一覧・デバイスエミュレーション。

### セッション作成

```json
{
  "tool": "session",
  "acquire": "my-session",
  "headless": false,
  "restore": true,
  "ttl_hours": 168
}
```

| パラメータ | 型 | デフォルト | 説明 |
|-----------|-----|----------|------|
| `acquire` | string | - | 作成するセッション名 |
| `headless` | bool | `false` | ヘッドレスモード |
| `restore` | bool | `true` | 前回のCookie/状態を復元 |
| `ttl_hours` | u64 | `168` (1週間) | セッション有効期限（0=無期限） |

### セッション解放

```json
{"tool": "session", "release": "my-session"}
```

### セッション一覧

```json
{"tool": "session", "list": true}
```

### Cookie インポート

```json
{
  "tool": "session",
  "session": "my-session",
  "import": "chrome",
  "browser": "chrome",
  "domains": [".amazon.co.jp"]
}
```

### デバイスエミュレーション

```json
{
  "tool": "session",
  "session": "my-session",
  "device": "iPhone 14"
}
```

カスタムビューポート:

```json
{
  "tool": "session",
  "session": "my-session",
  "viewport_width": 375,
  "viewport_height": 812,
  "user_agent": "Mozilla/5.0 ..."
}
```

### AI設定

```json
{
  "tool": "session",
  "ai_status": true,
  "ai_config": {
    "provider": "gemini",
    "model": "gemini-1.5-flash",
    "api_key": "...",
    "enabled": true
  }
}
```

---

## 2. navigate — ページ遷移

URLへ遷移し、ページ安定を待機。

```json
{
  "tool": "navigate",
  "session": "my-session",
  "url": "https://example.com",
  "wait_for": "stable",
  "timeout_ms": 30000
}
```

| パラメータ | 型 | デフォルト | 説明 |
|-----------|-----|----------|------|
| `session` | string | `"default"` | セッション名 |
| `url` | string | **必須** | 遷移先URL |
| `wait_for` | enum | `"stable"` | 待機条件 |
| `wait_selector` | string? | - | `selector`指定時の待機要素 |
| `timeout_ms` | u64 | `30000` | タイムアウト(ms) |

**wait_for 値:**

| 値 | 説明 |
|-----|------|
| `load` | ページロード完了 |
| `stable` | DOM安定 + ネットワークアイドル |
| `network_idle` | ネットワーク通信完了 |
| `selector` | 指定要素の出現 |

---

## 3. capture — スクリーンショット & ページ情報取得

```json
{
  "tool": "capture",
  "session": "my-session",
  "screenshot": true,
  "full_page": true,
  "include": ["cookies", "full_text"],
  "summarize": true
}
```

| パラメータ | 型 | デフォルト | 説明 |
|-----------|-----|----------|------|
| `session` | string | `"default"` | セッション名 |
| `screenshot` | bool | `true` | スクリーンショット取得 |
| `full_page` | bool | `false` | フルページキャプチャ |
| `include` | array | `[]` | 追加情報 |
| `selector` | string? | - | 特定要素のみキャプチャ |
| `summarize` | bool | `false` | AIによるページ要約 |
| `analyze_interactivity` | bool | `false` | インタラクティブ要素分析 |
| `analyze_vision` | bool | `false` | Vision LLMによる画像分析 |
| `use_cdp` | bool | `true` | CDPによるスクリーンショット |
| `text_max_chars` | usize? | - | テキスト最大文字数 |

**include 値:** `cookies`, `full_text`, `html`, `images`

スクリーンショットは `browser://` URI で返却:

```
browser://screenshots/{session}/{filename}
```

HTTP取得: `GET /v2/media/screenshots/{session}/{filename}`

---

## 4. interact — ブラウザ操作

複数アクションを1リクエストで実行。自動待機・リトライ付き。

```json
{
  "tool": "interact",
  "session": "my-session",
  "actions": [
    {"type": "click", "target": "#submit"},
    {"type": "type", "target": "#search", "value": "query", "instant": true},
    {"type": "scroll", "direction": "down", "amount": 500},
    {"type": "hover", "target": ".menu"},
    {"type": "select", "target": "#country", "value": "JP"},
    {"type": "wait", "condition": "element", "value": ".results"},
    {"type": "screenshot"}
  ],
  "options": {
    "human_mode": true,
    "wait_timeout_ms": 10000,
    "retry_count": 3,
    "retry_delay_ms": 500,
    "screenshot_on_error": true,
    "slow_mode_ms": 0
  }
}
```

### アクション一覧

| type | パラメータ | 説明 |
|------|-----------|------|
| `click` | `target`, `wait_after_ms?` | 要素クリック |
| `type` | `target`, `value`, `clear?`, `instant?` | テキスト入力 |
| `scroll` | `direction?`, `amount?`, `target?` | スクロール |
| `hover` | `target` | ホバー |
| `select` | `target`, `value` | ドロップダウン選択 |
| `wait` | `condition`, `value?`, `timeout_ms?` | 条件待機 |
| `screenshot` | - | 中間スクリーンショット |

### wait condition 一覧

`element`, `element_visible`, `element_clickable`, `element_hidden`, `url_contains`, `url_matches`, `text_contains`, `network_idle`, `timeout`

### scroll direction

`down` (デフォルト), `up`, `left`, `right`

### options

| パラメータ | 型 | デフォルト | 説明 |
|-----------|-----|----------|------|
| `human_mode` | bool | `false` | 人間らしい動作 |
| `wait_timeout_ms` | u64 | `10000` | 要素待機タイムアウト |
| `retry_count` | u32 | `3` | リトライ回数 |
| `retry_delay_ms` | u64 | `500` | リトライ間隔 |
| `screenshot_on_error` | bool | `false` | エラー時スクショ |
| `slow_mode_ms` | u64 | `0` | アクション間待機 |

---

## 5. extract — 構造化データ抽出

CSSセレクタでデータを構造的に抽出。スマート待機対応。

```json
{
  "tool": "extract",
  "session": "my-session",
  "selector": ".product-card",
  "fields": {
    "name": "h2",
    "price": ".price",
    "url": "a@href",
    "image": "img@src"
  },
  "limit": 10,
  "wait_for_count": 5,
  "scroll_for_more": true
}
```

| パラメータ | 型 | デフォルト | 説明 |
|-----------|-----|----------|------|
| `session` | string | `"default"` | セッション名 |
| `selector` | string | **必須** | 要素コンテナのセレクタ |
| `fields` | map | **必須** | フィールド名→サブセレクタ |
| `limit` | usize? | - | 最大取得件数 |
| `wait_for_count` | usize? | - | 最低件数(スマート待機) |
| `wait_timeout_ms` | u64 | `10000` | 待機タイムアウト |
| `scroll_for_more` | bool | `false` | スクロールで追加読み込み |
| `scroll_max` | usize | `5` | 最大スクロール回数 |

**フィールドセレクタ形式:**
- `"h2"` — テキスト内容
- `"a@href"` — 属性値（`@`の後に属性名）
- `"img@src"` — 画像URL

---

## 6. execute — JavaScript 実行

```json
{
  "tool": "execute",
  "session": "my-session",
  "script": "return document.title",
  "timeout_ms": 30000
}
```

| パラメータ | 型 | デフォルト | 説明 |
|-----------|-----|----------|------|
| `session` | string | `"default"` | セッション名 |
| `script` | string | **必須** | 実行するJavaScript |
| `timeout_ms` | u64 | `30000` | タイムアウト(ms) |

---

## 7. media — メディア操作

### YouTube字幕取得

```json
{
  "tool": "media",
  "session": "my-session",
  "action": {
    "type": "youtube_subtitles",
    "url": "https://www.youtube.com/watch?v=...",
    "language": "ja",
    "format": "srt"
  }
}
```

### YouTubeダウンロード

```json
{
  "tool": "media",
  "session": "my-session",
  "action": {
    "type": "youtube_download",
    "url": "https://www.youtube.com/watch?v=...",
    "quality": "720p",
    "audio_only": false
  }
}
```

### 動画分析

```json
{
  "tool": "media",
  "session": "my-session",
  "action": {
    "type": "video_analyze",
    "url": "https://...",
    "keyframes": true,
    "max_frames": 10
  }
}
```

### 画像収集

```json
{
  "tool": "media",
  "session": "my-session",
  "action": {
    "type": "collect_images",
    "selector": ".gallery img",
    "min_width": 200,
    "min_height": 200,
    "download": true,
    "max_images": 20
  }
}
```

---

## 8. network — ネットワーク監視

CDP経由でリクエスト/レスポンスを捕捉。

### 監視開始

```json
{
  "tool": "network",
  "session": "my-session",
  "action": {"type": "enable", "max_logs": 100}
}
```

### ログ取得

```json
{
  "tool": "network",
  "session": "my-session",
  "action": {"type": "get_logs", "filter": "api.example.com"}
}
```

### ログクリア

```json
{
  "tool": "network",
  "session": "my-session",
  "action": {"type": "clear_logs"}
}
```

### 監視停止

```json
{
  "tool": "network",
  "session": "my-session",
  "action": {"type": "disable"}
}
```

---

## 9. agent — Agenticモード

ローカルAI（Ollama/Gemini）がブラウザ操作を自律的に完遂。

### タスク開始

```json
{
  "tool": "agent",
  "session": "my-session",
  "action": {
    "type": "start",
    "goal": "Amazonでワイヤレスマウスを検索して上位5件を教えて",
    "max_steps": 15,
    "human_mode": true,
    "instant_type": true,
    "context": "日本語で結果を返してください",
    "system_prompt": "..."
  }
}
```

| パラメータ | 型 | デフォルト | 説明 |
|-----------|-----|----------|------|
| `session` | string | `"default"` | セッション名 |
| `goal` | string | **必須** | 達成したい目標 |
| `max_steps` | u32? | - | 最大ステップ数 |
| `human_mode` | bool | `false` | 人間らしい操作 |
| `instant_type` | bool | `false` | 即時入力(オートコンプリート回避) |
| `context` | string? | - | 追加コンテキスト |
| `system_prompt` | string? | - | カスタムシステムプロンプト |

### 状態確認

```json
{"tool": "agent", "session": "my-session", "action": {"type": "status"}}
```

### 再開

```json
{"tool": "agent", "session": "my-session", "action": {"type": "resume"}}
```

### キャンセル

```json
{"tool": "agent", "session": "my-session", "action": {"type": "cancel"}}
```

---

## 自動堅牢化（Robustness層）

全ツールに自動適用される堅牢化機能:

| 機能 | 説明 |
|------|------|
| 要素待機 | 存在・可視・クリック可能を自動チェック |
| 自動スクロール | 画面外の要素を自動で表示位置へ |
| DOM + Network安定待機 | DOMミューテーション停止 + XHR/fetch完了 |
| extract スマート待機 | 0件なら最大5秒自動リトライ、診断情報出力 |
| 自動リトライ | 失敗時に指定回数リトライ |
| エラー時スクショ | `screenshot_on_error: true` で自動保存 |
