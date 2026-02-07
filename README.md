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

### 解決：目的指向のMCPツール

WebView Bridgeは**「何をしたいか」だけを伝える**設計。

```
AI: agent(goal="Amazonでワイヤレスマウスを検索")
→ 完了。結果を返す。
```

| 設計思想 | 実装 |
|---------|------|
| **最小往復** | 1リクエストで複数アクション実行 |
| **自動リトライ** | 要素待機、可視性チェック内蔵 |
| **人間らしさ** | human_modeでBot検出回避 |
| **セッション永続** | Cookie/ログイン状態を保持 |
| **自律操作** | agentツールで目標ベース操作 |

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
| `agent` | 🔥 自律型ブラウザ操作（後述） |

### 🤖 Agenticモード - AIがWebを自律操作

**「検索して」じゃなく「買い物して」レベルの指示でOK。**

```
あなた: 「楽天でAnkerのモバイルバッテリーを探して、10000mAh以上で評価4以上のものをリストアップして」

agent(goal="...")
  ↓ AIが自動で:
  1. 楽天を開く
  2. 検索バーを見つけて入力
  3. 検索実行
  4. フィルター操作（容量、評価）
  5. 結果をスクレイピング
  6. 条件に合う商品を返却
```

#### なぜ強力か

| 従来 | Agenticモード |
|------|--------------|
| 1ステップ = 1リクエスト | **ゴール = 1リクエスト** |
| セレクタ変更 = 修正作業 | AIが適応的に要素を特定 |
| エラー = 停止 | AIが別アプローチを試行 |
| サイトごとにコード | 自然言語で汎用指示 |

#### 活用例

```json
// ECサイト調査
{"goal": "Amazon、楽天、Yahoo!ショッピングで同じ商品の価格を比較"}

// 情報収集
{"goal": "このニュースサイトの今日のヘッドラインを10件取得"}

// フォーム操作
{"goal": "問い合わせフォームに名前と内容を入力して送信"}

// ログイン後操作
{"goal": "ログイン後、マイページから注文履歴を取得", "context": "すでにログイン済み"}
```

#### 設定オプション

```json
{
  "action": {
    "type": "start",
    "goal": "目標を自然言語で",
    "max_steps": 15,           // 最大ステップ数
    "human_mode": true,        // Bot検出回避
    "instant_type": true,      // オートコンプリート回避
    "system_prompt": "日本語で回答して"  // AIへの追加指示
  }
}
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
