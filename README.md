# WebView Bridge

**AIやスクリプトからインターネットを自然に使う**

## 🎯 なぜWebView Bridge?

### 問題：既存ツールの非効率性

Puppeteer、Selenium、CDP…これらは「ブラウザを制御するAPI」であって「Webを使うツール」ではない。

```
AI: 「Amazonで商品を検索して」

従来のアプローチ:
1. セッション作成 → レスポンス待ち
2. navigate → レスポンス待ち
3. 要素を探す → レスポンス待ち
4. クリック → レスポンス待ち  
5. テキスト入力 → レスポンス待ち
6. 送信 → レスポンス待ち
7. 結果取得 → レスポンス待ち
... 以下繰り返し

→ 大量の往復通信、AIコンテキスト消費、エラー処理の嵐
```

### 解決：ゴールベースのMCPツール

WebView Bridgeは**「何をしたいか」だけを伝える**設計。

```
// 従来: 7往復
click("#search") → type("#search", "Anker") → click("#submit") → wait(...) → ...

// WebView Bridge: 1往復
interact(actions=[
  {type: "type", target: "#search", value: "Anker"},
  {type: "click", target: "#submit"},
  {type: "wait", condition: "element", value: ".results"}
])

// さらに完全自律: 1往復
agent(goal="Amazonでワイヤレスマウスを検索") 
→ 内部AIがcapture→判断→interact→...を自動ループ
```

| 設計思想 | 実装 |
|---------|------|
| **ゴールベース** | 複数アクションを1回で、agentで完全自律 |
| **最小往復** | 1リクエストで複数アクション実行 |
| **自動リトライ** | 要素待機、可視性チェック内蔵 |
| **人間らしさ** | human_modeでBot検出回避 |
| **セッション永続** | Cookie/ログイン状態を保持 |

## 🚀 主要機能

### MCPツール
| ツール | 説明 |
|--------|------|
| `session` | セッション管理（作成/解放/一覧） |
| `navigate` | ページ遷移（wait条件付き） |
| `capture` | スクリーンショット、要素取得、AI要約 |
| `interact` | クリック/タイプ/スクロール/待機 |
| `extract` | 構造化データ抽出 |
| `execute` | JavaScript実行 |
| `media` | YouTube字幕/ダウンロード、画像収集 |
| `agent` | 🔥 Agenticモード（後述） |

### 🤖 Agenticモード - 2段階AIアーキテクチャ

通常のMCPツールは「クリック」「入力」など個別操作。それだとAIが都度呼び出す必要があり、**コンテキストを消費**する。

**agent**ツールは違う。**目標を渡すと、内部のローカルAI（Ollama/Gemini）が自律的にブラウザ操作を完遂**して結果だけ返す。

```
┌─────────────────────────────────────────────────────────────────┐
│ Claude/Gemini (メインAI)                                        │
│  「楽天でAnkerのモバイルバッテリーを検索して」                      │
└───────────────────────────┬─────────────────────────────────────┘
                            │ agent(goal="...")
                            ▼
┌─────────────────────────────────────────────────────────────────┐
│ WebView Bridge                                                   │
│  ┌─────────────────────────────────────────────────────────────┐│
│  │ 内部AI (Ollama/Gemini)                                       ││
│  │  ループ:                                                     ││
│  │   1. capture → ページ状態取得                                ││
│  │   2. 「次は何をすべき？」→ AI判断                             ││
│  │   3. interact/navigate → 実行                                ││
│  │   4. 目標達成? → 完了 or 繰り返し                            ││
│  └──────────────────────────────┬──────────────────────────────┘│
│                                  ▼                               │
│                          WebView2でブラウザ操作                   │
└─────────────────────────────────────────────────────────────────┘
                            │
                            ▼ 結果だけ返却
┌─────────────────────────────────────────────────────────────────┐
│ Claude/Gemini (メインAI)                                        │
│  「5件の商品を見つけました: [...]」                              │
└─────────────────────────────────────────────────────────────────┘
```

#### メリット

| 従来（MCPツール個別呼び出し） | Agenticモード |
|------------------------------|--------------|
| AIが毎回判断→MCPコール | **目標1つでブラウザ操作完遂** |
| コンテキスト消費大 | **ローカルAIで処理、消費ゼロ** |
| セレクタ変更 = 修正作業 | 内部AIが適応的に要素特定 |
| エラー = 停止 | 内部AIが別アプローチ試行 |

#### 設定

```json
{
  "action": {
    "type": "start",
    "goal": "楽天でAnkerのモバイルバッテリーを検索して評価順に並べる",
    "max_steps": 15,
    "human_mode": true,
    "instant_type": true
  }
}
```

**必要な設定**（config.toml）:
```toml
[ai]
enabled = true
provider = "ollama"        # or "gemini"
model = "gemma3:12b"       # ローカルLLM
# api_key = "..."          # Gemini使用時
```

### 🎭 human_mode（完全人間化）

```json
{
  "actions": [{"type": "type", "target": "#search", "value": "Anker"}],
  "options": {"human_mode": true}
}
```

| 機能 | 説明 |
|------|------|
| **ベジェ曲線マウス** | 直線じゃない自然なカーブ |
| **イージング** | 加減速（始めゆっくり→速く→ゆっくり） |
| **タイポ＆修正** | 3%で打ち間違い→BackSpace→正しい文字 |
| **Shift/Spaceミス** | 大文字忘れ、ダブルスペース |
| **思考の間** | ランダムに止まる |
| **慣性スクロール** | 徐々に減速 |

## 📐 アーキテクチャ

```
AI (Claude/Gemini)           WebView Bridge (Windows)
┌─────────────────┐          ┌─────────────────────────┐
│  MCP Client     │◀──MCP──▶│  MCP Server             │
│                 │          │  ├─ Session Manager     │
└─────────────────┘          │  ├─ Robustness Layer    │
                             │  └─ WebView2 Pool       │
                             └───────────┬─────────────┘
                                         │
                             ┌───────────▼─────────────┐
                             │  Edge WebView2 Runtime  │
                             └─────────────────────────┘
```

## 🔧 使い方

### 1. 起動
```bash
cargo run --release
```

### 2. MCP設定（config.toml）
```toml
[server]
host = "127.0.0.1"
port = 3030

[ai]
enabled = true
provider = "ollama"  # or "gemini"
model = "gpt-oss:20b"
```

### 3. AIから操作
```
Amazonを開いてワイヤレスマウスを検索して
→ agent(goal="Amazonでワイヤレスマウスを検索", human_mode=true)
```

## 📖 ドキュメント

- [クイックスタート](docs/QUICK_START.md) - 5分で始める
- [MCPツール詳細](docs/MCP_TOOLS.md) - 各ツールの使い方
- [アーキテクチャ](docs/ARCHITECTURE.md) - 内部構造
- [REST API](docs/API.md) - REST APIリファレンス

## ⚠️ Bot対策サイトのコツ

1. `headless: false` - ウィンドウ表示
2. `human_mode: true` - 人間らしい動作
3. `instant: true` - サジェスト回避（Amazon等）
4. 直接URL回避 - クリックで遷移

## 📝 License

MIT
