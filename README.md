<p align="center">
  <img src="docs/img/Gemini_Generated_Image_p2f3bwp2f3bwp2f3.png" alt="WebView Bridge Banner" width="800">
</p>

<h1 align="center">
  <img src="docs/img/icon.png" alt="icon" width="32" height="32">
  WebView Bridge
</h1>

<p align="center">
  <strong>AIやスクリプトからインターネットを自然に使う</strong><br>
  Autonomous Visual Agent — 視覚的インタラクティビティ分析 & WebView MCP
</p>

<p align="center">
  <img alt="Version" src="https://img.shields.io/badge/version-3.5.0-blue">
  <img alt="License" src="https://img.shields.io/badge/license-MIT-green">
  <img alt="Platform" src="https://img.shields.io/badge/platform-Windows-lightgrey">
  <img alt="Rust" src="https://img.shields.io/badge/rust-edition%202024-orange">
</p>

> **v3.5 — OpenClaw (CDP Native)**: Chrome DevTools Protocol直接制御により、ネットワーク監視・フルページキャプチャ・要素取得の精度が劇的に向上。

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
| **AIフレンドリー** | 構造化JSON応答、AI要約機能、MCP標準対応 |
| **ゴールベース** | 複数アクションを1回で、agentで完全自律 |
| **最小往復** | 1リクエストで複数アクション実行 |
| **自動リトライ** | 要素待機、可視性チェック内蔵 |
| **人間らしさ** | human_modeでBot検出回避 |
| **セッション永続** | Cookie/ログイン状態を保持 |
| **Webダッシュボード** | ブラウザから全機能を操作・監視 |

### 🤝 AI向け設計

**AIが使いやすいことを最優先に設計：**

| 機能 | 説明 |
|------|------|
| **構造化レスポンス** | 全てJSON、AIが即座にパース可能 |
| **AI要約** | `capture(summarize=true)`でページ内容をAIが要約 |
| **セレクタ自動提示** | captureがクリック可能要素とセレクタを返す |
| **コンテキスト節約** | 往復最小化、agentでローカル処理 |
| **MCP標準** | Claude/Gemini/OpenClawから直接利用可能 |

## 🚀 主要機能

### MCPツール（9ツール）
| ツール | 説明 |
|--------|------|
| `session` | セッション管理（作成/解放/一覧/デバイスエミュレーション） |
| `navigate` | ページ遷移（DOM安定+ネットワーク完了のスマート待機） |
| `capture` | スクリーンショット、要素取得、AI要約、**チャレンジ検出** |
| `interact` | クリック/タイプ/スクロール/待機（human_mode対応） |
| `extract` | 構造化データ抽出（**スマート待機**で動的ページにも対応） |
| `execute` | JavaScript実行 |
| `media` | YouTube字幕/ダウンロード、画像収集 |
| `network` | 🆕 **ネットワーク監視**（リクエスト/レスポンス/ヘッダー捕捉） |
| `agent` | 🔥 Agenticモード（後述） |

### 🛡️ 自動堅牢化（Robustness層）

**AIが意識せずとも、MCPツールが裏で自動処理：**

```
AI: interact(click="#submit-button")

         ↓ WebView Bridgeが自動で:

1. 要素が存在するまで待機
2. 要素が可視になるまで待機  
3. 要素が画面内に入るまで自動スクロール
4. クリック可能になるまで待機
5. クリック実行
6. 失敗したら自動リトライ（最大3回）
```

| 機能 | 説明 |
|------|------|
| **要素待機** | 存在・可視・クリック可能を自動チェック |
| **自動スクロール** | 要素が画面外なら自動で表示位置へ |
| **DOM+Network安定待機** | DOMミューテーション停止 **かつ** XHR/fetch完了まで待機 |
| **extract スマート待機** | 要素が0件なら最大5秒自動リトライ、失敗時に診断情報出力 |
| **自動リトライ** | 失敗時に指定回数リトライ |
| **エラー時スクショ** | デバッグ用に自動保存可能 |

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
AI (Claude/Gemini/OpenClaw)    WebView Bridge v3.5 (Windows)
┌─────────────────┐            ┌──────────────────────────────┐
│  MCP Client     │◀──MCP──▶  │  MCP Server (9 tools)        │
│                 │            │  ├─ Session Manager          │
└─────────────────┘            │  ├─ Robustness Layer         │
                               │  ├─ CDP Controller ← 🆕     │
                               │  │   ├─ Network Monitor      │
                               │  │   ├─ Screenshot Engine    │
                               │  │   └─ DOM Evaluator        │
                               │  └─ WebView2 Pool            │
                               └────────────┬─────────────────┘
                                            │ CDP (DevTools Protocol)
                               ┌────────────▼─────────────────┐
                               │  Edge WebView2 Runtime       │
                               └──────────────────────────────┘
```

### 🆕 OpenClaw — CDP Native 移行のベネフィット

v3.5でChrome DevTools Protocol (CDP) への直接制御に移行。従来のJS Eval方式と比較して：

| 機能 | 従来 (JS Eval) | OpenClaw (CDP) |
|------|---------------|----------------|
| **スクリーンショット** | WebView2 API経由、ビューポートのみ | CDP `Page.captureScreenshot`、**フルページ対応** |
| **ネットワーク監視** | 不可能 | `Network.requestWillBeSent`等で全通信を捕捉 |
| **要素取得** | DOM操作のみ | CDP経由で安定した実行コンテキスト |
| **ページ安定判定** | DOMミューテーションのみ | DOM + XHR/fetch追跡の**AND条件** |
| **extract精度** | 動的ページで空振り多発 | **スマート待機**で確実に取得 |
| **パス出力** | OS依存の絶対パス | `browser://` URI（OS非依存） |

#### スクリーンショットURI

スクリーンショットはOS非依存の `browser://` URIで返却：

```
browser://screenshots/{session}/{filename}

例: browser://screenshots/my-session/cap_20260208_012300.png
```

- **AIのコンテキストにはURIのみ**（base64埋め込み無し）
- HTTPで取得: `GET /v2/media/screenshots/{session}/{filename}`
- MCP `resources/list` で一覧取得可能

## 🔧 使い方

### 1. 起動
```bash
cargo run --release
```
起動後、ブラウザで `http://localhost:9400` を開くとダッシュボードが表示されます。

### 2. MCP設定

#### Claude Desktop / Cline

`claude_desktop_config.json` または MCP設定ファイル:

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

#### config.toml（サーバー設定）

`%APPDATA%/webview-bridge/config.toml`:

```toml
[server]
bind = "0.0.0.0"
port = 9400
max_sessions = 10

[ai]
enabled = true
provider = "ollama"       # "ollama" or "gemini"
model = "gemma3:12b"      # ローカルLLM
# api_key = "your-key"    # Gemini使用時
```

### 3. AIから操作
```
Amazonを開いてワイヤレスマウスを検索して
→ agent(goal="Amazonでワイヤレスマウスを検索", human_mode=true)
```

## 🖥️ ダッシュボード

サーバー起動後、`http://localhost:9400` でWebダッシュボードにアクセスできます。ブラウザ上からセッション管理、スクリーンショット、AI設定、マクロ実行など全機能をGUIで操作可能です。

### 11ページ構成

| カテゴリ | ページ | 機能 |
|---------|--------|------|
| **監視** | ダッシュボード | システム状態、セッション概要、スクリーンショットプレビュー |
| | セッション管理 | 起動/停止/クローン/削除、詳細パネル、検索フィルタ |
| | スクリーンショット | ギャラリー表示、ライトボックス、デバイスプリセット撮影 |
| | ダウンロード | ジョブ管理、一括DL、ストレージ状況 |
| **設定** | AI設定 | プロバイダ/モデル/API Key、接続テスト |
| | サーバー | バインド/ポート/最大セッション/デフォルト解像度 |
| | メディア | DL/SS保存先、動画品質、画像抽出、YouTube字幕・DL |
| **自動化** | オートメーション | ブラウザ操作(Navigate/Click/Type/Wait)、Goal実行、マクロ管理 |
| | AIツール | AIログイン、データ抽出、画像分析、使用統計 |
| **ツール** | APIテスター | クイック操作、カスタムリクエスト |
| | MCPツール | ツール定義一覧、MCPツール実行、バッチAPI |

認証トークンはサーバー起動時に自動生成され、ダッシュボードに自動注入されます。外部からREST APIを利用する場合は、起動ログに表示されるBearerトークンを使用してください。

## 📖 ドキュメント

- [クイックスタート](docs/QUICK_START.md) - 5分で始める
- [MCPツール詳細](docs/MCP_TOOLS.md) - 各ツールの使い方
- [アーキテクチャ](docs/ARCHITECTURE.md) - 内部構造
- [REST API](docs/API.md) - REST APIリファレンス

## 📊 Autonomous Visual Agent — 技術概要

<details>
<summary>スライドを展開（10ページ）</summary>

### ブラウザ自動化の終焉と新生
<img src="docs/img/Autonomous_Visual_Agents_Page1.png" alt="ブラウザ自動化の終焉と新生" width="700">

### エグゼクティブサマリー
<img src="docs/img/Autonomous_Visual_Agents_Page2.png" alt="エグゼクティブサマリー" width="700">

### AIエージェントの進化論：第3世代への到達
<img src="docs/img/Autonomous_Visual_Agents_Page3.png" alt="AIエージェント進化論" width="700">

### 革新1：Visual Interactivity Analysis
<img src="docs/img/Autonomous_Visual_Agents_Page4.png" alt="Visual Interactivity Analysis" width="700">

### 革新2：セッションの相乗り
<img src="docs/img/Autonomous_Visual_Agents_Page5.png" alt="セッションの相乗り" width="700">

### 革新3：自己修復とボット防御
<img src="docs/img/Autonomous_Visual_Agents_Page6.png" alt="自己修復とボット防御" width="700">

### 比較分析：既存ツール vs WebView Bridge
<img src="docs/img/Autonomous_Visual_Agents_Page7.png" alt="比較分析" width="700">

### 競合ランドスケープ分析
<img src="docs/img/Autonomous_Visual_Agents_Page8.png" alt="競合ランドスケープ" width="700">

### 新しい共生関係（Symbiosis）
<img src="docs/img/Autonomous_Visual_Agents_Page9.png" alt="Symbiosis" width="700">

### 開発者コミュニティへの衝撃
<img src="docs/img/Autonomous_Visual_Agents_Page10.png" alt="開発者コミュニティへの衝撃" width="700">

</details>

## ⚠️ Bot対策サイトのコツ

1. `headless: false` - ウィンドウ表示
2. `human_mode: true` - 人間らしい動作
3. `instant: true` - サジェスト回避（Amazon等）
4. 直接URL回避 - クリックで遷移

### 🔐 CAPTCHA/チャレンジ自動対応

`capture`が以下を自動検出し、対処戦略を返す：

| チャレンジ | 自動対応 |
|-----------|---------|
| Cloudflare Turnstile | `wait_and_retry`: 10秒待機→リトライ |
| Cloudflare "Just a moment" | `wait_and_retry`: 15秒待機→リトライ |
| Google reCAPTCHA v2 | `click_checkbox`: チェックボックスクリック |
| Google reCAPTCHA v3 | `proceed`: 自動処理（介入不要） |
| hCaptcha | `click_checkbox`: チェックボックスクリック |

```
【チャレンジ検出】
- cloudflare_interstitial: 待機推奨 (15000ms後リトライ)
- google_recaptcha_v2: → click .recaptcha-checkbox で解決試行可
```


## 📝 License

MIT
