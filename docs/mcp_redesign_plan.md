# MCP AIフレンドリー再設計計画書

## 概要

WebView Bridge MCPレイヤーをAIフレンドリーに再設計する。V2 REST APIは温存し、MCPレイヤーのみを抽象化。

### 設計原則
1. **V2 REST APIは変更なし**（プログラマブル用途）
2. **MCPは2つのモード**：Direct Mode（決定的）+ Agentic Mode（ACP）
3. **ツール数を8個に削減**（32個→8個）
4. **セッション単位でAI状態も永続化**

---

## アーキテクチャ

```
┌─────────────────────────────────────────────────────────────┐
│                  External AI (Claude/Gemini)                 │
│                 ユーザーの意図を解釈                          │
└─────────────────────────────────────────────────────────────┘
                              │
                             MCP
                              ↓
┌─────────────────────────────────────────────────────────────┐
│                     WebView Bridge                           │
│                                                              │
│  ┌─────────────────────┐    ┌─────────────────────────────┐ │
│  │   Direct Mode       │    │    Agentic Mode (ACP)       │ │
│  │   (決定的操作)       │    │    (ゴールベース)            │ │
│  │   AI不要            │    │    内部エージェント使用       │ │
│  │                     │    │                             │ │
│  │  - navigate         │    │  ┌─────────────────────┐    │ │
│  │  - interact         │    │  │   Internal Agent    │    │ │
│  │  - capture          │    │  │   (Gemini/Local)    │    │ │
│  │  - extract          │    │  │                     │    │ │
│  │  - session          │    │  │  Loop:              │    │ │
│  │  - media            │    │  │  1. Capture State   │    │ │
│  │  - execute          │    │  │  2. AI Analyze      │    │ │
│  │                     │    │  │  3. Plan Action     │    │ │
│  │       │             │    │  │  4. Execute V2 API  │    │ │
│  │       ↓             │    │  │  5. Check Result    │    │ │
│  │    V2 REST API      │    │  │  6. Repeat/Done     │    │ │
│  │                     │    │  └─────────────────────┘    │ │
│  └─────────────────────┘    │           │                 │ │
│                             │           ↓                 │ │
│                             │      V2 REST API            │ │
│                             └─────────────────────────────┘ │
│                                                              │
│  ┌──────────────────────────────────────────────────────────┐│
│  │              Session State (永続化)                       ││
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐      ││
│  │  │ Session A   │  │ Session B   │  │ Session C   │      ││
│  │  │ ├─ Browser  │  │ ├─ Browser  │  │ ├─ Browser  │      ││
│  │  │ │  ├─ URL   │  │ │  ├─ URL   │  │ │  ├─ URL   │      ││
│  │  │ │  ├─ Cookie│  │ │  ├─ Cookie│  │ │  ├─ Cookie│      ││
│  │  │ │  └─ History│ │ │  └─ History│ │ │  └─ History│     ││
│  │  │ └─ Agent    │  │ └─ Agent    │  │ └─ Agent    │      ││
│  │  │    ├─ Goal  │  │    ├─ Goal  │  │    ├─ Goal  │      ││
│  │  │    ├─ Steps │  │    ├─ Steps │  │    ├─ Steps │      ││
│  │  │    └─ Context│ │    └─ Context│ │    └─ Context│     ││
│  │  └─────────────┘  └─────────────┘  └─────────────┘      ││
│  └──────────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────────┘
```

---

## 現行MCPツール一覧（全32ツール）

### mcp_stdio.rs（Stdio Mode）: 10ツール
| ツール名 | 説明 | 新設計 |
|---------|------|--------|
| `browse` | URLナビゲーション | → `navigate` |
| `screenshot` | スクリーンショット | → `capture` |
| `extract` | CSSセレクタ抽出 | → `extract` |
| `click` | 要素クリック | → `interact` |
| `type_text` | テキスト入力 | → `interact` |
| `execute` | JavaScript実行 | → `execute` |
| `get_text` | ページテキスト取得 | → `capture` |
| `youtube_subtitles` | YouTube字幕 | → `media` |
| `youtube_download` | YouTube DL | → `media` |
| `collect_images` | 画像収集 | → `capture` |

### api_v2.rs（HTTP MCP Mode）: 22ツール
| ツール名 | 説明 | 新設計 |
|---------|------|--------|
| `browse` | URLナビゲーション | → `navigate` |
| `screenshot` | スクリーンショット | → `capture` |
| `extract` | CSSセレクタ抽出 | → `extract` |
| `click` | 要素クリック | → `interact` |
| `type_text` | テキスト入力 | → `interact` |
| `execute` | JavaScript実行 | → `execute` |
| `get_text` | ページテキスト取得 | → `capture` |
| `get_cookies` | Cookie取得 | → `capture` |
| `set_cookies` | Cookie設定 | → `session` |
| `session_acquire` | セッション取得 | → `session` |
| `session_list` | セッション一覧 | → `session` |
| `session_release` | セッション解放 | → `session` |
| `check_auth` | 認証状態確認 | → `agent` (analyze) |
| `goal_extract` | ゴールベース抽出 | → `agent` |
| `goal_navigate` | ゴールベースナビ | → `agent` |
| `download_file` | ファイルDL | → `media` |
| `get_images` | 画像取得 | → `capture` |
| `macro_execute` | マクロ実行 | → `interact` |
| `youtube_subtitles` | YouTube字幕 | → `media` |
| `youtube_download` | YouTube DL | → `media` |
| `collect_images` | 画像収集 | → `capture` |
| `job_status` | ジョブ状態 | → `execute` |
| `ai_analyze` | AI分析 | → `agent` |
| `ai_image_analyze` | AI画像分析 | → `agent` |
| `ai_extract` | AI抽出 | → `agent` |
| `ai_login` | AIログイン | → `agent` |
| `video_analyze` | 動画分析 | → `media` |

---

## 新MCPツール構成（8ツール）

| ツール | モード | 説明 | 統合対象 |
|--------|--------|------|----------|
| **navigate** | Direct | URLナビゲーション | browse |
| **interact** | Direct | 操作マクロ（複数アクション一括） | click, type_text, scroll, macro_execute |
| **capture** | Direct | ページ状態取得（複数項目一括） | screenshot, get_text, get_cookies, get_images, collect_images |
| **extract** | Direct | CSSセレクタ抽出 | extract |
| **session** | Direct | セッション管理 | session_*, set_cookies |
| **media** | Direct | メディア操作 | youtube_*, video_analyze, download_file |
| **execute** | Direct | カスタムJS、ジョブ管理 | execute, job_status |
| **agent** | Agentic | ゴールベース内部エージェント | ai_*, goal_*, check_auth |

---

## ツール詳細設計

### 1. `navigate` - ページ遷移（Direct）

```json
{
  "tool": "navigate",
  "session": "my-session",
  "url": "https://example.com",
  "wait_for": "load|networkidle|selector",
  "wait_selector": "#main-content",
  "timeout_ms": 30000
}
```

---

### 2. `interact` - 操作マクロ（Direct）

複数の操作を一括実行。決定的（AI不使用）。

**常に `actions` 配列を使用**（単一操作も配列で指定）:

```json
{
  "tool": "interact",
  "session": "my-session",
  "actions": [
    {"type": "type", "target": "#username", "value": "user@example.com"},
    {"type": "type", "target": "#password", "value": "secret"},
    {"type": "click", "target": "button[type=submit]"},
    {"type": "wait", "condition": "url_contains", "value": "/dashboard"}
  ]
}
```

**単一アクションの場合も配列**:
```json
{
  "tool": "interact",
  "actions": [{"type": "click", "target": "#submit-btn"}]
}
```

**サポートアクション**:
| アクション | パラメータ | 説明 |
|-----------|-----------|------|
| `click` | target | 要素クリック |
| `type` | target, value, clear? | テキスト入力 |
| `scroll` | direction, amount | スクロール |
| `hover` | target | ホバー |
| `select` | target, value | セレクト選択 |
| `wait` | condition, value, timeout_ms | 待機 |
| `screenshot` | - | 途中スクショ |

**target指定**:
- CSS: `"#login-btn"`, `".submit"`
- XPath: `"xpath://button[@type='submit']"`
- テキスト: `"text:ログイン"`

---

### 3. `capture` - 状態取得（Direct）

ページの状態を取得。**MCPコンテンツタイプでメディア分離**。

#### 設計原則: コンテキストドレイン防止

MCPプロトコルの`content`配列を活用し、テキストと画像を分離:
- `type: "text"` → AIのテキストコンテキストへ
- `type: "image"` → AIのビジョン処理へ（テキストコンテキスト消費なし）

#### 基本リクエスト
```json
{
  "tool": "capture",
  "session": "my-session"
}
```

#### MCPレスポンス
```json
{
  "content": [
    {
      "type": "text",
      "text": "URL: https://example.com/login\nTitle: ログイン - Example\n\n【状態】\nログインページ。メールとパスワードの入力欄あり。\n\n【操作可能要素】\n- input#email (空)\n- input#password (空)\n- button#login-btn 「ログイン」\n- a.forgot 「パスワードを忘れた方」"
    },
    {
      "type": "image",
      "data": "iVBORw0KGgo...",
      "mimeType": "image/png"
    }
  ]
}
```

#### テキスト部分の最適化

AIが判断に必要な情報のみ含める:

| 含める | 含めない |
|--------|----------|
| URL、タイトル | 全ページHTML |
| 状態要約（1-2文） | 全テキスト内容 |
| 操作可能要素リスト | 装飾要素 |
| エラーメッセージ | Cookie詳細 |
| フォーム状態 | 隠し要素 |

**テキスト部分の目安サイズ: 500-2000文字**

#### オプション

```json
{
  "tool": "capture",
  "include": ["cookies", "full_text"],
  "screenshot": false,
  "text_max_chars": 5000,
  "selector": "#main"
}
```

| パラメータ | 説明 |
|-----------|------|
| `screenshot` | スクリーンショット含める（default: true） |
| `include` | 追加項目: cookies, full_text, html, images |
| `text_max_chars` | full_text時の最大文字数 |
| `selector` | 特定領域のみ |
| `full_page` | フルページスクリーンショット |

---

### 4. `extract` - データ抽出（Direct）

CSSセレクタで構造化抽出。

```json
{
  "tool": "extract",
  "session": "my-session",
  "selector": ".product-item",
  "fields": {
    "name": "h2",
    "price": ".price",
    "url": "a@href"
  },
  "limit": 50
}
```

---

### 5. `session` - セッション管理（Direct）

**操作をパラメータ名で直接指定**（`action`パラメータ廃止）:

**セッション取得**:
```json
{
  "tool": "session",
  "acquire": "my-session",
  "headless": false,
  "restore": true
}
```

**セッション解放**:
```json
{
  "tool": "session",
  "release": "my-session"
}
```

**セッション一覧**:
```json
{
  "tool": "session",
  "list": true
}
```

**ブラウザCookieインポート**:
```json
{
  "tool": "session",
  "import": "my-session",
  "browser": "chrome|edge|firefox",
  "domains": ["google.com"]
}
```

---

### 6. `media` - メディア操作（Direct）

```json
{
  "tool": "media",
  "action": "subtitles|download|analyze",
  "url": "https://youtube.com/watch?v=...",
  // subtitles
  "language": "ja",
  "format": "text|srt|vtt",
  // download
  "quality": "best|hd|sd",
  "audio_only": false,
  // analyze
  "keyframes": true,
  "max_frames": 30
}
```

---

### 7. `execute` - 低レベル操作（Direct）

```json
{
  "tool": "execute",
  "session": "my-session",
  // JS実行
  "script": "document.title",
  // ジョブ状態確認
  "job_id": "abc123"
}
```

---

### 8. `agent` - 内部エージェント（Agentic/ACP）

ゴールベースの自律実行。内部AIでループ処理。

```json
{
  "tool": "agent",
  "session": "my-session",
  "action": "start|resume|status|cancel",
  
  // start: 新規ゴール開始
  "goal": "Googleにログインしてアカウント設定を開く",
  "credentials": {
    "username": "user@example.com",
    "password": "secret"
  },
  "max_steps": 10,
  "timeout_ms": 120000,
  
  // resume: 中断したタスクを再開
  "task_id": "task_abc123",
  
  // オプション
  "verbose": true,
  "screenshot_each_step": false
}
```

---

## エージェント状態管理（ACP）

### セッションごとのエージェント状態

各WebViewセッションに対応するエージェント状態を永続化。
セッション再開時にエージェントも再開可能。

```
sessions/
├── my-session/
│   ├── profile/           # ブラウザプロファイル
│   │   ├── Cookies
│   │   ├── Local Storage/
│   │   └── ...
│   ├── state.json          # セッション状態
│   │   ├── last_url
│   │   ├── navigation_history
│   │   └── auth_status
│   └── agent/              # エージェント状態
│       ├── current_task.json
│       ├── task_history/
│       └── context.json
```

### AgentState構造

```json
{
  "session_name": "my-session",
  "current_task": {
    "id": "task_abc123",
    "goal": "Googleにログインしてアカウント設定を開く",
    "status": "in_progress|paused|completed|failed",
    "created_at": "2026-02-07T10:00:00Z",
    "updated_at": "2026-02-07T10:05:00Z",
    "steps_completed": 3,
    "steps_max": 10,
    "last_action": {
      "type": "click",
      "target": "#next-btn",
      "result": "success"
    },
    "next_planned_action": {
      "type": "type",
      "target": "#password",
      "reasoning": "パスワード入力欄が表示されたため"
    },
    "error": null
  },
  "context": {
    "page_summary": "Googleログインページ、メールアドレス入力済み",
    "detected_elements": ["#password", "#next-btn", "#forgot-password"],
    "auth_state": "partial"
  },
  "conversation_history": [
    {"role": "user", "content": "Goal: Googleにログイン..."},
    {"role": "assistant", "content": "Step 1: navigate to google.com..."},
    ...
  ]
}
```

### エージェント操作フロー

```
┌────────────────────────────────────────────────────┐
│                  agent action: start               │
└────────────────────────────────────────────────────┘
                          │
                          ↓
┌────────────────────────────────────────────────────┐
│  1. Load/Create AgentState for session             │
│  2. Set goal, initialize task                      │
└────────────────────────────────────────────────────┘
                          │
                          ↓
┌────────────────────────────────────────────────────┐
│                  Agent Loop                        │
│  ┌──────────────────────────────────────────────┐  │
│  │ while (!goal_achieved && steps < max):       │  │
│  │   1. capture(screenshot, text)               │  │
│  │   2. AI: analyze current state               │  │
│  │   3. AI: decide next action                  │  │
│  │   4. execute action via V2 API               │  │
│  │   5. save state (resumable)                  │  │
│  │   6. check: goal achieved?                   │  │
│  └──────────────────────────────────────────────┘  │
└────────────────────────────────────────────────────┘
                          │
            ┌─────────────┼─────────────┐
            ↓             ↓             ↓
       completed       paused        failed
            │             │             │
            ↓             ↓             ↓
      return result   save state   return error
                     (resumable)
```

---

## 内部エージェント用ツール（Function Calling）

内部AIがブラウザを操作するためのツール定義。
Gemini Function Calling形式で提供。

### アーキテクチャ

```
┌─────────────────────────────────────────────────────────────┐
│                    External AI (Claude)                      │
│                    MCP `agent` tool を呼び出し               │
└─────────────────────────────────────────────────────────────┘
                              │
                         agent tool
                              ↓
┌─────────────────────────────────────────────────────────────┐
│               Internal Agent (Gemini + Function Calling)     │
│                                                              │
│   System Prompt:                                             │
│   「あなたはWebブラウザを操作するエージェントです。           │
│     与えられたゴールを達成するため、以下のツールを使用して    │
│     ブラウザを操作してください。」                           │
│                                                              │
│   Available Tools (Function Calling):                        │
│   ┌─────────────────────────────────────────────────────┐   │
│   │  Browser Observation:                                │   │
│   │  - capture_page()        → screenshot + DOM info    │   │
│   │  - get_current_url()     → current URL             │   │
│   │  - check_element(sel)    → element exists?         │   │
│   │  - get_element_text(sel) → element text content    │   │
│   │                                                      │   │
│   │  Browser Actions:                                    │   │
│   │  - click(target)         → click element           │   │
│   │  - type_text(target, v)  → input text              │   │
│   │  - scroll(dir, amount)   → scroll page             │   │
│   │  - hover(target)         → hover over element      │   │
│   │  - select_option(t, v)   → select dropdown         │   │
│   │  - press_key(key)        → keyboard input          │   │
│   │  - wait_for(condition)   → wait for state          │   │
│   │                                                      │   │
│   │  Task Control:                                       │   │
│   │  - report_progress(msg)  → status update           │   │
│   │  - request_info(prompt)  → ask user for input      │   │
│   │  - complete(result)      → goal achieved           │   │
│   │  - fail(reason)          → give up                 │   │
│   │  - pause(reason)         → save & pause            │   │
│   └─────────────────────────────────────────────────────┘   │
│                              │                               │
│                              ↓                               │
│                    Tool Executor (Rust)                      │
│                              │                               │
│                              ↓                               │
│                        V2 REST API                           │
└─────────────────────────────────────────────────────────────┘
```

### ツール定義（Gemini Function Calling形式）

```json
{
  "tools": [
    {
      "name": "capture_page",
      "description": "現在のページのスクリーンショットとDOM情報を取得します。各ステップの開始時に必ず呼び出してください。",
      "parameters": {
        "type": "object",
        "properties": {
          "include_screenshot": {
            "type": "boolean",
            "description": "スクリーンショットを含める（default: true）"
          },
          "selector": {
            "type": "string",
            "description": "特定領域のみキャプチャ（オプション）"
          }
        }
      }
    },
    {
      "name": "get_current_url",
      "description": "現在のページURLを取得します。",
      "parameters": { "type": "object", "properties": {} }
    },
    {
      "name": "check_element",
      "description": "指定したセレクタの要素が存在するか確認します。",
      "parameters": {
        "type": "object",
        "properties": {
          "selector": {
            "type": "string",
            "description": "CSSセレクタまたはXPath"
          }
        },
        "required": ["selector"]
      }
    },
    {
      "name": "get_element_text",
      "description": "指定した要素のテキスト内容を取得します。",
      "parameters": {
        "type": "object",
        "properties": {
          "selector": {
            "type": "string",
            "description": "CSSセレクタまたはXPath"
          }
        },
        "required": ["selector"]
      }
    },
    {
      "name": "click",
      "description": "要素をクリックします。",
      "parameters": {
        "type": "object",
        "properties": {
          "target": {
            "type": "string",
            "description": "CSSセレクタ、XPath、または 'text:表示テキスト' 形式"
          }
        },
        "required": ["target"]
      }
    },
    {
      "name": "type_text",
      "description": "テキストフィールドに文字を入力します。",
      "parameters": {
        "type": "object",
        "properties": {
          "target": {
            "type": "string",
            "description": "入力フィールドのセレクタ"
          },
          "text": {
            "type": "string",
            "description": "入力するテキスト"
          },
          "clear_first": {
            "type": "boolean",
            "description": "入力前に既存テキストをクリア（default: true）"
          }
        },
        "required": ["target", "text"]
      }
    },
    {
      "name": "scroll",
      "description": "ページをスクロールします。",
      "parameters": {
        "type": "object",
        "properties": {
          "direction": {
            "type": "string",
            "enum": ["up", "down", "left", "right"],
            "description": "スクロール方向"
          },
          "amount": {
            "type": "string",
            "enum": ["small", "medium", "large", "page"],
            "description": "スクロール量"
          }
        },
        "required": ["direction"]
      }
    },
    {
      "name": "hover",
      "description": "要素の上にマウスカーソルを移動します。",
      "parameters": {
        "type": "object",
        "properties": {
          "target": {
            "type": "string",
            "description": "要素のセレクタ"
          }
        },
        "required": ["target"]
      }
    },
    {
      "name": "select_option",
      "description": "ドロップダウンから選択肢を選びます。",
      "parameters": {
        "type": "object",
        "properties": {
          "target": {
            "type": "string",
            "description": "selectまたはドロップダウンのセレクタ"
          },
          "value": {
            "type": "string",
            "description": "選択する値またはテキスト"
          }
        },
        "required": ["target", "value"]
      }
    },
    {
      "name": "press_key",
      "description": "キーボードキーを押します。",
      "parameters": {
        "type": "object",
        "properties": {
          "key": {
            "type": "string",
            "description": "キー名（Enter, Tab, Escape, ArrowDown など）"
          },
          "modifiers": {
            "type": "array",
            "items": { "type": "string" },
            "description": "修飾キー（Ctrl, Shift, Alt）"
          }
        },
        "required": ["key"]
      }
    },
    {
      "name": "wait_for",
      "description": "条件が満たされるまで待機します。",
      "parameters": {
        "type": "object",
        "properties": {
          "condition": {
            "type": "string",
            "enum": ["element", "url_contains", "url_equals", "text_visible", "timeout"],
            "description": "待機条件の種類"
          },
          "value": {
            "type": "string",
            "description": "条件の値（セレクタ、URL部分、テキストなど）"
          },
          "timeout_ms": {
            "type": "integer",
            "description": "タイムアウト（ミリ秒、default: 10000）"
          }
        },
        "required": ["condition"]
      }
    },
    {
      "name": "report_progress",
      "description": "進捗状況を報告します。重要なマイルストーン時に呼び出してください。",
      "parameters": {
        "type": "object",
        "properties": {
          "message": {
            "type": "string",
            "description": "進捗メッセージ"
          },
          "percentage": {
            "type": "integer",
            "description": "進捗率（0-100）"
          }
        },
        "required": ["message"]
      }
    },
    {
      "name": "request_info",
      "description": "ユーザーに追加情報を要求します（2FA コード、CAPTCHA解決など）。タスクは一時停止されます。",
      "parameters": {
        "type": "object",
        "properties": {
          "prompt": {
            "type": "string",
            "description": "ユーザーへの質問"
          },
          "input_type": {
            "type": "string",
            "enum": ["text", "code", "captcha", "confirmation"],
            "description": "入力タイプ"
          }
        },
        "required": ["prompt"]
      }
    },
    {
      "name": "complete",
      "description": "ゴールが達成されたことを報告します。タスクを終了します。",
      "parameters": {
        "type": "object",
        "properties": {
          "result": {
            "type": "string",
            "description": "達成結果の説明"
          },
          "data": {
            "type": "object",
            "description": "抽出したデータなど（オプション）"
          }
        },
        "required": ["result"]
      }
    },
    {
      "name": "fail",
      "description": "ゴールが達成できないことを報告します。タスクを終了します。",
      "parameters": {
        "type": "object",
        "properties": {
          "reason": {
            "type": "string",
            "description": "失敗理由"
          },
          "recoverable": {
            "type": "boolean",
            "description": "リトライ可能か"
          }
        },
        "required": ["reason"]
      }
    },
    {
      "name": "pause",
      "description": "タスクを一時停止し、状態を保存します。後で再開可能です。",
      "parameters": {
        "type": "object",
        "properties": {
          "reason": {
            "type": "string",
            "description": "一時停止理由"
          }
        },
        "required": ["reason"]
      }
    }
  ]
}
```

### 内部AIプロンプト例

```
あなたはWebブラウザを自動操作するAIエージェントです。

【ゴール】
{user_goal}

【利用可能な認証情報】
{credentials if provided}

【現在の状態】
- URL: {current_url}
- ページ概要: {page_summary}
- 完了ステップ: {steps_completed} / {max_steps}

【ルール】
1. 各ステップで必ず capture_page() を呼び、現在の状態を確認してください
2. 操作後は結果を確認し、期待通りでなければリトライまたは別の方法を試してください
3. CAPTCHA や 2FA が検出された場合は request_info() で人間に助けを求めてください
4. ゴール達成時は complete()、達成不可能時は fail() を呼んでください
5. 機密情報（パスワード等）は report_progress() や fail() のメッセージに含めないでください

次のアクションを決定してください。
```

### ツール実行フロー

```
┌─────────────────────────────────────────────────────────────┐
│ Internal AI: "click(target='#login-btn')" を決定            │
└─────────────────────────────────────────────────────────────┘
                              │
                              ↓
┌─────────────────────────────────────────────────────────────┐
│ Tool Executor (Rust):                                        │
│   1. ターゲット解決 ('#login-btn' → CSS selector)           │
│   2. V2 API呼び出し: POST /click {session, selector}        │
│   3. 結果取得                                                │
│   4. AgentState更新 (last_action, steps_completed)          │
│   5. 永続化 (state.json)                                    │
│   6. 結果を内部AIに返却                                      │
└─────────────────────────────────────────────────────────────┘
                              │
                              ↓
┌─────────────────────────────────────────────────────────────┐
│ Internal AI: 結果を受け取り、次のアクションを決定            │
│   → capture_page() で状態確認                               │
│   → 次のステップへ...                                        │
└─────────────────────────────────────────────────────────────┘
```

### セッション再開

```json
// 1. セッション取得（ブラウザ＋エージェント状態復元）
{
  "tool": "session",
  "action": "acquire",
  "name": "my-session",
  "restore": true  // last_url + agent state も復元
}

// 2. 中断中のタスクがあれば再開
{
  "tool": "agent",
  "session": "my-session",
  "action": "resume"
  // task_id省略時は最新の中断タスクを再開
}

// または状態確認のみ
{
  "tool": "agent",
  "session": "my-session",
  "action": "status"
}
```

---

## V2 API → MCP マッピング表

| MCP Tool | Action/Mode | V2 API Endpoint |
|----------|-------------|-----------------|
| navigate | - | POST /navigate |
| interact | click | POST /click |
| interact | type | POST /type |
| interact | wait | POST /wait |
| interact | scroll | POST /execute (JS) |
| capture | screenshot | POST /screenshot |
| capture | text | POST /execute (innerText) |
| capture | cookies | GET /cookies |
| extract | - | POST /execute (querySelectorAll) |
| session | acquire | POST /session/acquire |
| session | release | POST /session/release |
| session | list | GET /session/list |
| media | subtitles | External yt-dlp |
| media | download | External yt-dlp |
| execute | script | POST /execute |
| agent | * | Internal loop → V2 API |

---

## 期待される効果

### Before（現状）: 5回コール
```
click → #username
type_text → "user@example.com"
click → #password  
type_text → "secret"
click → button[type=submit]
```

### After（Direct Mode）: 1回コール
```json
{
  "tool": "interact",
  "actions": [
    {"type": "type", "target": "#username", "value": "user@example.com"},
    {"type": "type", "target": "#password", "value": "secret"},
    {"type": "click", "target": "button[type=submit]"}
  ]
}
```

### After（Agentic Mode）: 1回コール + 自律実行
```json
{
  "tool": "agent",
  "goal": "Googleにログインする",
  "credentials": {"username": "...", "password": "..."}
}
// → 内部エージェントが自律的にログイン完了まで実行
// → セッション切断しても再開可能
```

---

## 実装計画

### フェーズ1: 新ツール定義（2-3時間）
1. `src/mcp_v3/mod.rs` 作成
2. 8ツールのスキーマ定義
3. ルーティング追加

### フェーズ2: Direct Mode実装（4-5時間）
1. `navigate` - 既存コードラップ
2. `interact` - マクロエンジン
3. `capture` - 複合データ取得
4. `extract` - フィールドマッピング
5. `session` - 既存コードラップ
6. `media` - 既存コードラップ
7. `execute` - 既存コードラップ

### フェーズ3: Agentic Mode実装（6-8時間）
1. `AgentState` 構造体定義
2. エージェントループエンジン
3. AI連携（Gemini API）
4. 状態永続化・復元
5. エラーハンドリング・リトライ

### フェーズ4: 後方互換性（1時間）
1. 旧ツール名→新ツールへのエイリアス
2. 移行ガイド作成

---

## 設計上の考慮事項

### Direct Mode vs Agentic Mode の使い分け

| 状況 | 推奨モード |
|------|-----------|
| 操作手順が明確 | Direct (`interact`) |
| セレクタが分かっている | Direct |
| 確実性・速度が重要 | Direct |
| 複雑なマルチステップ | Agentic (`agent`) |
| セレクタ不明、動的ページ | Agentic |
| CAPTCHA、2FAがありえる | Agentic |
| 長時間タスク（中断再開必要） | Agentic |

### セキュリティ考慮

- `credentials`はメモリ内のみ、永続化しない
- エージェントログにパスワード含めない
- 内部AI呼び出しはオプトイン（APIキー設定必須）
