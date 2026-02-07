# Visual Interactivity Analysis（視覚的操作性分析）設計書

> 作成日: 2026-02-07

## 概要

セレクタだけでは判断できない「人間が見てクリックしたくなる要素」を、Vision LLMで分析・スコア化する機能。

## 背景・問題

### 現状

`capture`ツールは要素のセレクタ・タグ・テキストを返すが、それだけでは以下が判別できない：

| ケース | セレクタ | 実際の見た目 |
|--------|---------|-------------|
| カスタムボタン | `div.btn-primary` | 派手なグラデーション、ホバーで拡大 |
| 静的テキスト | `span.label` | 装飾なし |
| 画像リンク | `a > img` | 商品画像、クリック促進 |
| ドロップダウン | `div.dropdown-trigger` | 矢印アイコン付き |
| ホバーメニュー | `li.nav-item` | hover時にサブメニュー展開 |

### AIの問題

- AIは「`div`だからクリックできない」と誤判断しがち
- 実際の見た目やアフォーダンス（行動誘発性）を判断できない
- 結果、正しい要素を選べずエラー

## 解決策

### アプローチ

1. 各インタラクティブ要素の**周辺画像**をキャプチャ
2. **Vision LLM**に「クリック可能に見えるか？」を問い合わせ
3. **スコア（0-1）と理由**を返却

### データフロー

```
capture(analyze_interactivity=true)
    │
    ▼
┌─────────────────────────────────────────────────────┐
│ 1. ページからインタラクティブ要素を抽出              │
│    - button, a, input, [onclick], [role=button]...  │
└─────────────────────────┬───────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────┐
│ 2. 各要素の周辺領域をキャプチャ                      │
│    - 要素のboundingRect取得                         │
│    - padding追加（周辺コンテキスト含める）            │
│    - 個別にスクリーンショット（base64）              │
└─────────────────────────┬───────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────┐
│ 3. Vision LLMに送信（バッチ or 個別）               │
│                                                     │
│  プロンプト:                                        │
│  「この画像の中央にある要素は、人間がクリック        │
│   したくなるように見えますか？                      │
│   0から1のスコアと、その理由を返してください。       │
│   ボタン形状、色のコントラスト、アイコン、          │
│   ホバー効果の示唆などを考慮してください。」        │
└─────────────────────────┬───────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────┐
│ 4. 結果を集約してレスポンス                         │
└─────────────────────────────────────────────────────┘
```

## API設計

### リクエスト

```json
{
  "tool": "capture",
  "arguments": {
    "session": "default",
    "analyze_interactivity": true,
    "interactivity_options": {
      "max_elements": 20,           // 分析対象の最大数
      "min_size": [30, 20],         // 最小サイズ（px）
      "padding": 10,                // 周辺キャプチャのpadding
      "include_hover_state": true   // ホバー時の見た目も分析
    }
  }
}
```

### レスポンス

```json
{
  "url": "https://example.com",
  "title": "Example Page",
  "elements": [
    {
      "index": 1,
      "selector": "div.custom-btn",
      "tag": "div",
      "text": "今すぐ購入",
      "rect": {"x": 100, "y": 200, "width": 150, "height": 40},
      "interactivity": {
        "score": 0.95,
        "confidence": "high",
        "reason": "ボタン形状、オレンジ色の背景、太字テキスト、角丸デザイン",
        "visual_cues": ["button_shape", "high_contrast", "call_to_action_text"]
      }
    },
    {
      "index": 2,
      "selector": "span.label",
      "tag": "span", 
      "text": "商品説明",
      "rect": {"x": 50, "y": 300, "width": 80, "height": 20},
      "interactivity": {
        "score": 0.12,
        "confidence": "high",
        "reason": "装飾なし、背景と同色、静的テキストに見える",
        "visual_cues": ["no_decoration", "low_contrast"]
      }
    },
    {
      "index": 3,
      "selector": "div.dropdown",
      "tag": "div",
      "text": "カテゴリ ▼",
      "rect": {"x": 200, "y": 50, "width": 120, "height": 35},
      "interactivity": {
        "score": 0.78,
        "confidence": "medium",
        "reason": "ドロップダウン矢印アイコン、枠線あり",
        "visual_cues": ["dropdown_arrow", "border"]
      }
    }
  ]
}
```

## 技術詳細

### 1. 要素キャプチャ方法

```javascript
// JavaScript側
function captureElementRegion(selector, padding = 10) {
  const el = document.querySelector(selector);
  const rect = el.getBoundingClientRect();
  
  // paddingを追加した領域
  const captureRect = {
    x: Math.max(0, rect.left - padding),
    y: Math.max(0, rect.top - padding),
    width: rect.width + padding * 2,
    height: rect.height + padding * 2
  };
  
  // その領域だけをキャプチャ（Canvas API or WebView2 API）
  return captureRegion(captureRect);
}
```

### 2. Vision LLMプロンプト

```
あなたはUIデザインの専門家です。以下の画像の中央にある要素を分析してください。

質問: この要素は、ユーザーがクリック/タップしたくなるように見えますか？

評価基準:
- ボタンらしい形状（角丸、影、立体感）
- 色のコントラスト（背景との差）
- テキスト内容（「購入」「送信」などのCTA）
- アイコンの存在
- 枠線やホバー効果の示唆
- サイズの適切さ

出力形式（JSON）:
{
  "score": 0.0-1.0,
  "confidence": "high" | "medium" | "low",
  "reason": "分析理由を日本語で",
  "visual_cues": ["検出した視覚的手がかりの配列"]
}
```

### 3. 対応Vision LLM

| プロバイダ | モデル | 備考 |
|-----------|-------|------|
| Ollama | LLaVA, Bakllava | ローカル、無料 |
| Gemini | gemini-2.0-flash | 高速、安価 |
| OpenAI | gpt-4o-mini | 高精度 |

### 4. config.toml設定

```toml
[ai.vision]
enabled = true
provider = "ollama"          # ollama, gemini, openai
model = "llava:13b"          # Vision対応モデル
# api_key = "..."            # Gemini/OpenAI用
```

## パフォーマンス考慮

### 最適化戦略

| 戦略 | 説明 |
|------|------|
| **要素数制限** | `max_elements`でN件に限定（デフォルト20） |
| **サイズフィルタ** | 小さすぎる要素は除外（クリック困難） |
| **バッチ処理** | 複数要素を1回のLLMコールで分析 |
| **キャッシュ** | 同一ページ・同一要素は再分析しない |
| **事前フィルタ** | CSSで明らかにnon-interactiveな要素は除外 |

### 事前フィルタ例

```javascript
// 分析不要な要素を除外
function shouldAnalyze(el) {
  const style = getComputedStyle(el);
  
  // 非表示
  if (style.display === 'none' || style.visibility === 'hidden') return false;
  
  // 小さすぎる
  const rect = el.getBoundingClientRect();
  if (rect.width < 30 || rect.height < 20) return false;
  
  // pointer-events: none
  if (style.pointerEvents === 'none') return false;
  
  return true;
}
```

## 想定所要時間

| 要素数 | Ollama (LLaVA 13B) | Gemini Flash |
|--------|-------------------|--------------|
| 5件 | ~3秒 | ~1秒 |
| 10件 | ~6秒 | ~2秒 |
| 20件 | ~12秒 | ~4秒 |

※ バッチ処理で軽減可能

## 実装フェーズ

### Phase 1: 基本実装
- [ ] 要素の個別キャプチャ機能
- [ ] Vision LLM連携（Ollama）
- [ ] 基本的なスコアリング

### Phase 2: 最適化
- [ ] バッチ処理
- [ ] キャッシュ機能
- [ ] 事前フィルタ

### Phase 3: 拡張
- [ ] ホバー状態分析
- [ ] Gemini/OpenAI対応
- [ ] スコアに基づくソート

## 使用例

### AIエージェントでの活用

```
AI: capture(analyze_interactivity=true)

レスポンス:
{
  "elements": [
    {"selector": ".buy-btn", "interactivity": {"score": 0.95, ...}},
    {"selector": ".info-text", "interactivity": {"score": 0.1, ...}},
    ...
  ]
}

AI: 「score 0.95の.buy-btnが購入ボタンだな」
    → interact(click=".buy-btn")
```

### Agenticモードでの活用

内部AIがcapture時に自動でinteractivity分析を行い、より正確な要素選択が可能になる。

## まとめ

| 項目 | 内容 |
|------|------|
| **目的** | セレクタだけでは判断できない視覚的操作性を分析 |
| **手法** | 要素周辺キャプチャ + Vision LLM |
| **出力** | スコア（0-1）、理由、視覚的手がかり |
| **利点** | AIがより正確に操作対象を特定可能 |
| **トレードオフ** | 分析時間が増加（要素数に比例） |
