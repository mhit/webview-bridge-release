# WebView Bridge MCP Tools

Claude/Gemini等のAIがブラウザを操作するためのMCPサーバー。

## 🔧 Available Tools

### 1. **session** - セッション管理
```json
{
  "acquire": "my-session",    // セッション取得（なければ作成）
  "headless": false,          // false=ウィンドウ表示（Bot対策サイト推奨）
  "release": "my-session",    // セッション解放
  "list": true                // 全セッション一覧
}
```

### 2. **navigate** - ページ遷移
```json
{
  "session": "default",
  "url": "https://example.com",
  "wait_for": "stable",       // "load" | "stable" | "networkidle" | "selector"
  "wait_selector": "#content" // wait_for=selectorの場合
}
```

### 3. **capture** - ページ状態取得
```json
{
  "session": "default",
  "screenshot": true,         // スクリーンショット保存
  "summarize": true,          // AIでページを要約
  "include": ["full_text", "cookies", "html", "images"],
  "full_page": true           // 全ページキャプチャ
}
```

### 4. **interact** - ブラウザ操作
```json
{
  "session": "default",
  "actions": [
    {"type": "click", "target": "#button"},
    {"type": "type", "target": "#input", "value": "text", "clear": true, "instant": true},
    {"type": "scroll", "direction": "down", "amount": 500},
    {"type": "wait", "condition": "element", "value": "#loaded"}
  ],
  "options": {
    "human_mode": true        // Bot対策：人間らしい動き
  }
}
```

#### Type Action Options
| オプション | 説明 |
|-----------|------|
| `clear` | 入力前にフィールドをクリア |
| `instant` | **NEW** 値を一発セット（サジェスト回避） |

### 5. **extract** - 構造化データ抽出
```json
{
  "session": "default",
  "selector": ".product-card",
  "fields": {
    "title": ".title",
    "price": ".price",
    "link": "a@href"          // @attrで属性値取得
  },
  "limit": 10,
  "scroll_for_more": true     // 無限スクロール対応
}
```

### 6. **execute** - JavaScript実行
```json
{
  "session": "default",
  "script": "document.title"
}
```

### 7. **agent** - 自律型ブラウザ操作
```json
{
  "session": "default",
  "action": {
    "type": "start",
    "goal": "Amazonで「Anker モバイルバッテリー」を検索する",
    "max_steps": 10,
    "human_mode": true,
    "instant_type": true,
    "system_prompt": "価格を重視して判断してください"
  }
}
```

**エージェントが自動で:**
- ページ状態を分析
- 次のアクションを決定（click/type/navigate）
- 目標達成まで繰り返し
- **内部でMCPツールを使用**（robustness層活用）

### 8. **media** - メディア操作

#### YouTubeダウンロード（要: yt-dlp）
```json
{
  "action": {
    "type": "youtube_download",
    "url": "https://youtube.com/watch?v=xxx",
    "quality": "hd",
    "audio_only": false
  }
}
```

#### YouTube字幕取得（要: yt-dlp）
```json
{
  "action": {
    "type": "youtube_subtitles",
    "url": "https://youtube.com/watch?v=xxx",
    "language": "ja,en",
    "format": "json3"
  }
}
```

#### 動画解析（要: ffprobe, ffmpeg）
```json
{
  "action": {
    "type": "video_analyze",
    "url": "C:/path/to/video.mp4",
    "keyframes": true,
    "extract_audio": true
  }
}
```

#### ページ画像収集
```json
{
  "action": {
    "type": "collect_images",
    "selector": ".product-image",
    "min_width": 200,
    "min_height": 200,
    "max_images": 20
  }
}
```

---

## 🤖 human_mode の詳細

`human_mode: true` を指定すると、以下の人間らしい動作がシミュレートされます。

### マウス移動
| 機能 | 説明 |
|------|------|
| **ベジェ曲線移動** | 直線ではなく自然なカーブで移動 |
| **イージング（加減速）** | 始めゆっくり→中間速く→終わりゆっくり |
| **マイクロジッター** | 手の震えをシミュレート |
| **オーバーシュート** | 10%の確率で行き過ぎて戻る |

### タイピング（`instant: false` の場合）
| 機能 | 確率 | 説明 |
|------|------|------|
| **隣接キー打ち間違い** | 3% | `thw` → BackSpace → `the` |
| **ダブルスペース** | 2% | スペース2回→気づいて削除 |
| **Shift押し忘れ** | 1.5% | `hello` → `Hello`に修正 |
| **Shift離し忘れ** | 1% | `THe` → `The`に修正 |
| **思考の間** | 2% | 300-800msランダム停止 |
| **高速コンボ** | - | `th`, `er`等は高速入力 |
| **句読点で休止** | - | 考え中をシミュレート |

### スクロール
| 機能 | 説明 |
|------|------|
| **慣性スクロール** | 始め速く→徐々に減速 |
| **ホイールジッター** | 手の動きによる揺れ |

---

## ⚡ instant_type の使い方

Amazonなどサジェスト機能が強いサイトでは、文字入力が干渉されて失敗することがあります。

```json
// サジェストに邪魔されるサイト向け
{"type": "type", "target": "#search", "value": "Anker", "instant": true}
```

| モード | 動作 | 用途 |
|--------|------|------|
| `instant: false` | 1文字ずつ入力 | リアクティブフォーム |
| `instant: true` | 値を一発セット | サジェスト回避 |

**Lenient Mode**: `instant: true`で内部エラーが発生しても、値がセットされていれば成功として扱います。

---

## ⚠️ Bot対策サイトのコツ

1. **headless: false** - ウィンドウ表示モード
2. **human_mode: true** - 人間らしい操作
3. **instant_type: true** - サジェスト干渉回避
4. **直接URL回避** - ホームページ→クリックで遷移
5. **適度な待機** - wait actionを挟む

---

## 📍 設定

AI設定（config.toml）:
```toml
[ai]
enabled = true
provider = "ollama"           # または "gemini"
model = "gpt-oss:20b"
# api_key = "..."             # Geminiの場合
```

---

## 🧪 テスト例

### 基本的なナビゲーション
```
セッションを作成してGoogleを開き、ページタイトルを確認して
```

### AI要約機能
```
capture(summarize=true) でWikipediaを要約
```

### 自律型エージェント（Amazon）
```json
{
  "action": {
    "type": "start",
    "goal": "Amazonでワイヤレスマウスを検索して価格順に並べる",
    "human_mode": true,
    "instant_type": true
  }
}
```

### データ抽出
```json
{
  "selector": ".product",
  "fields": {
    "title": ".title",
    "price": ".price",
    "rating": ".stars@data-rating"
  }
}
```
