# Visual Interactivity Analysis（視覚的操作性分析）設計書

> 作成日: 2026-02-07

## 概要

セレクタだけでは判断できない「人間が見てクリックしたくなる要素」を、DOM/CSS情報を抽出してLLMで分析・スコア化する機能。

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

### SPAへの優位性

クライアントサイドレンダリング（React/Vue/Angular/Next.js等）では、静的HTMLからは何もわからない：

```html
<!-- サーバーから返るHTML -->
<div id="root"></div>

<!-- ↓ JavaScriptでレンダリング後 -->
<div id="root">
  <button class="xyz123">購入</button>  <!-- 動的生成、クラス名はハッシュ -->
  <div data-v-abc>...</div>              <!-- Vueスコープ -->
</div>
```

**本機能の優位性:**

| 静的HTML分析 | 本機能（レンダリング後分析） |
|-------------|--------------------------|
| `<div id="root"></div>`しか見えない | 実際のDOM構造を取得 |
| クラス名がハッシュ化されて意味不明 | **スタイルを取得**して判断 |
| CSS-in-JSは取得不可 | **computedStyle**で最終結果取得 |
| 状態依存の表示は不明 | 現在の状態を反映 |


### アプローチ：ハイブリッド分析

1. **DOM/CSS分析**（基本）: テキストLLMで軽量に分析
2. **アクション予測**: 「クリックできる」だけでなく「何ができるか」を推定
3. **画像分析フォールバック**: alt無しの画像リンクはVision LLMで内容分析

### なぜハイブリッド？

| 要素タイプ | 分析方法 | 理由 |
|-----------|---------|------|
| テキストボタン | DOM/CSS | スタイルとテキストで判断可能 |
| アイコンボタン | DOM/CSS + アイコン推定 | SVGクラス名などから推測 |
| 画像リンク（alt有） | DOM/CSS | altテキストで意図判断 |
| 画像リンク（alt無） | **Vision LLM** | 画像内容を見ないとわからない |

### 出力の拡張：アクション予測

単なる「クリック可能スコア」ではなく、**何ができそうか**も返す：

```json
{
  "selector": "div.dropdown-trigger",
  "interactivity": {
    "score": 0.85,
    "predicted_actions": [
      {"action": "click", "purpose": "ドロップダウンメニューを開く"},
      {"action": "hover", "purpose": "サブメニューを表示"}
    ],
    "reason": "矢印アイコン、枠線、cursor:pointer"
  }
}
```

### アクション種別

| アクション | 検出根拠 |
|-----------|---------|
| `click_navigate` | `<a href>`, router-link |
| `click_submit` | `<button type=submit>`, formAction |
| `click_toggle` | checkbox, switch, accordion |
| `click_expand` | dropdown, menu, ▼アイコン |
| `click_select` | select, option, radio |
| `type_input` | input, textarea |
| `hover_reveal` | tooltip, popover, :hover変化 |
| `scroll` | overflow: scroll, infinite scroll領域 |

### 抽出するDOM/CSS情報

```javascript
function extractVisualProperties(element) {
  const style = getComputedStyle(element);
  const hoverStyle = getHoverStyles(element); // 疑似クラス
  const rect = element.getBoundingClientRect();
  
  return {
    // 基本情報
    tag: element.tagName.toLowerCase(),
    text: element.textContent?.trim().slice(0, 50),
    role: element.getAttribute('role'),
    ariaLabel: element.getAttribute('aria-label'),
    
    // サイズ・位置
    size: { width: rect.width, height: rect.height },
    
    // ボタンらしさの指標
    cursor: style.cursor,  // "pointer" = クリック可能
    hasOnClick: !!element.onclick || element.hasAttribute('onclick'),
    
    // 視覚スタイル
    backgroundColor: style.backgroundColor,
    borderRadius: style.borderRadius,
    border: style.border,
    boxShadow: style.boxShadow,
    
    // テキストスタイル
    fontWeight: style.fontWeight,
    textTransform: style.textTransform,
    color: style.color,
    
    // ホバー効果（重要！）
    hoverChanges: {
      backgroundColor: hoverStyle?.backgroundColor !== style.backgroundColor,
      transform: hoverStyle?.transform !== 'none',
      boxShadow: hoverStyle?.boxShadow !== style.boxShadow,
      scale: hoverStyle?.transform?.includes('scale'),
    },
    
    // アイコン・画像
    hasIcon: hasIconChild(element),  // svg, img, ::before/after
    hasImage: !!element.querySelector('img'),
    
    // インタラクティブ属性
    isDisabled: element.disabled || element.getAttribute('aria-disabled') === 'true',
    tabIndex: element.tabIndex,
    
    // コンテキスト
    parentTag: element.parentElement?.tagName.toLowerCase(),
    position: style.position,
  };
}
```

### データフロー

```
capture(analyze_interactivity=true)
    │
    ▼
┌─────────────────────────────────────────────────────┐
│ 1. ページからインタラクティブ要素を抽出              │
│    - button, a, input, [onclick], [role=button]...  │
│    - cursor:pointer の要素                          │
└─────────────────────────┬───────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────┐
│ 2. 各要素のDOM/CSS情報を抽出                         │
│    - computed styles                                │
│    - hover時のスタイル変化                          │
│    - アイコン・画像の有無                           │
│    - ARIA属性                                      │
└─────────────────────────┬───────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────┐
│ 3. テキストLLMに送信（バッチ）                      │
│                                                     │
│  プロンプト:                                        │
│  「以下のDOM/CSS情報から、各要素が人間にとって      │
│   クリックしたくなるように見えるか評価してください。 │
│   ボタン形状、色のコントラスト、ホバー効果、        │
│   cursor:pointer などを考慮してください。」         │
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
      "max_elements": 20,
      "min_size": [30, 20]
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
      "visual_properties": {
        "cursor": "pointer",
        "backgroundColor": "rgb(255, 102, 0)",
        "borderRadius": "8px",
        "boxShadow": "0 2px 4px rgba(0,0,0,0.2)",
        "hoverChanges": {
          "backgroundColor": true,
          "transform": true
        },
        "fontWeight": "700"
      },
      "interactivity": {
        "score": 0.95,
        "reason": "cursor:pointer、オレンジ背景、角丸、影、太字、ホバーで変化"
      }
    },
    {
      "index": 2,
      "selector": "span.label",
      "tag": "span",
      "text": "商品説明",
      "visual_properties": {
        "cursor": "default",
        "backgroundColor": "transparent",
        "borderRadius": "0",
        "hoverChanges": {}
      },
      "interactivity": {
        "score": 0.08,
        "reason": "cursor:default、装飾なし、ホバー効果なし"
      }
    }
  ]
}
```

## ホバースタイル取得

### 課題

`getComputedStyle`は現在の状態しか取得できない。`:hover`時のスタイルを取得するには工夫が必要。

### 解決策

```javascript
function getHoverStyles(element) {
  // 方法1: CSSStyleSheetから:hoverルールを検索
  for (const sheet of document.styleSheets) {
    try {
      for (const rule of sheet.cssRules) {
        if (rule.selectorText?.includes(':hover') && 
            element.matches(rule.selectorText.replace(':hover', ''))) {
          return rule.style;
        }
      }
    } catch (e) {
      // Cross-origin stylesheetは読めない
    }
  }
  
  // 方法2: 一時的にhover状態をシミュレート（マウス位置移動）
  // → やや重いがより正確
  
  // 方法3: transition/animation プロパティの存在で推測
  const style = getComputedStyle(element);
  return {
    hasTransition: style.transition !== 'none' && style.transition !== 'all 0s ease 0s',
    hasAnimation: style.animation !== 'none'
  };
}
```

## 画像分析フォールバック（Vision LLM）

### 問題

```html
<a href="/product/12345">
  <img src="product.jpg">  <!-- alt無し！ -->
</a>
```

DOM/CSSだけでは「何の画像か」「クリックすると何が起こるか」わからない。

### 解決策

alt属性が無い、またはtextContentが空の画像リンクのみ、**Vision LLMで画像内容を分析**：

```javascript
function needsVisionAnalysis(element) {
  // 画像を含むリンクかどうか
  if (element.tagName !== 'A') return false;
  
  const img = element.querySelector('img');
  if (!img) return false;
  
  // altがあればVision不要
  if (img.alt && img.alt.trim() !== '') return false;
  
  // aria-labelがあればVision不要
  if (element.getAttribute('aria-label')) return false;
  
  // textContentがあればVision不要
  if (element.textContent.trim() !== '') return false;
  
  return true;  // Vision分析が必要
}
```

### Vision分析フロー

```
needsVisionAnalysis(element) = true
    │
    ▼
┌─────────────────────────────────────────────────────┐
│ 1. 画像のsrcを取得                                   │
│    - 相対パス → 絶対パスに変換                       │
│    - base64の場合はそのまま使用                      │
└─────────────────────────┬───────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────┐
│ 2. Vision LLM (Gemini/LLaVA) に送信                 │
│                                                     │
│  プロンプト:                                        │
│  「この画像は何を表していますか？                    │
│   クリックするとどこに遷移しそうですか？             │
│   簡潔に日本語で回答してください。」                 │
└─────────────────────────┬───────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────┐
│ 3. 結果を構造化                                      │
│    {                                                │
│      "image_content": "ワイヤレスマウスの商品写真",  │
│      "predicted_action": "商品詳細ページへ遷移",     │
│      "confidence": 0.9                              │
│    }                                                │
└─────────────────────────────────────────────────────┘
```

### レスポンス例（画像分析付き）

```json
{
  "selector": "a.product-link",
  "tag": "a",
  "text": "",
  "has_image": true,
  "image_analysis": {
    "content": "ワイヤレスマウスの商品画像、Ankerロゴ入り",
    "predicted_action": "click_navigate",
    "purpose": "商品詳細ページへ遷移",
    "analyzed_by": "vision"
  },
  "interactivity": {
    "score": 0.92,
    "reason": "商品画像リンク、クリックで詳細ページへ遷移と推定"
  }
}
```

### パフォーマンス考慮

Vision分析は重いため、最小限に抑える：

| 条件 | Vision使用 |
|------|-----------|
| alt属性あり | ❌ 不要 |
| aria-labelあり | ❌ 不要 |
| textContentあり | ❌ 不要 |
| 上記全て無し | ✅ 使用 |
| 画像サイズ小（< 50px） | ❌ アイコン扱い |



LLMを呼ぶ前に、ルールベースで事前スコアを計算して効率化：

```javascript
function preScore(props) {
  let score = 0;
  
  // cursor: pointer は強力な指標
  if (props.cursor === 'pointer') score += 0.3;
  
  // 角丸がある
  if (parseFloat(props.borderRadius) > 0) score += 0.1;
  
  // 影がある
  if (props.boxShadow !== 'none') score += 0.1;
  
  // ホバーで変化する
  if (Object.values(props.hoverChanges).some(v => v)) score += 0.2;
  
  // 背景色がある（透明でない）
  if (!props.backgroundColor.includes('transparent') && 
      !props.backgroundColor.includes('rgba(0, 0, 0, 0)')) score += 0.1;
  
  // CTAテキスト
  const ctaWords = ['購入', '申込', '登録', '送信', 'submit', 'buy', 'add', 'cart'];
  if (ctaWords.some(w => props.text?.toLowerCase().includes(w))) score += 0.15;
  
  // onclick属性
  if (props.hasOnClick) score += 0.2;
  
  return Math.min(1, score);
}
```

## LLMプロンプト

```
あなたはUI/UXデザインの専門家です。以下のDOM/CSS情報から、各要素が人間にとって「クリックしたくなる」ように見えるか評価してください。

評価基準:
- cursor: pointer → 強くクリック可能を示唆
- borderRadius > 0 → ボタンらしい形状
- boxShadow → 立体感、押せそう
- ホバー時のスタイル変化 → インタラクティブ
- backgroundColor（透明でない）→ 目立つ
- fontWeight: bold → 重要なアクション
- テキスト内容（購入、送信など）→ CTA

入力:
[要素1]
tag: div
text: "今すぐ購入"
cursor: pointer
backgroundColor: rgb(255, 102, 0)
borderRadius: 8px
boxShadow: 0 2px 4px rgba(0,0,0,0.2)
hoverChanges: { backgroundColor: true, transform: true }
fontWeight: 700

[要素2]
tag: span
text: "商品説明"
cursor: default
backgroundColor: transparent
borderRadius: 0
hoverChanges: {}
fontWeight: 400

出力形式（JSON配列）:
[
  {"index": 1, "score": 0.95, "reason": "..."},
  {"index": 2, "score": 0.08, "reason": "..."}
]
```

## パフォーマンス

| 処理 | 時間目安 |
|------|---------|
| DOM/CSS抽出（20要素） | ~50ms |
| ルールベース事前スコア | ~5ms |
| LLM分析（テキスト） | ~500ms-2s |
| **合計** | **~1-3秒** |

Vision LLMの場合は5-15秒かかるため、**大幅に高速化**。

## 実装フェーズ

### Phase 1: DOM/CSS抽出
- [ ] extractVisualProperties関数
- [ ] ホバースタイル推定
- [ ] ルールベース事前スコア

### Phase 2: LLM分析
- [ ] プロンプト設計
- [ ] バッチ処理
- [ ] レスポンスパース

### Phase 3: 統合
- [ ] capture APIに統合
- [ ] agent内部で自動活用
- [ ] キャッシュ

## まとめ

| 項目 | 内容 |
|------|------|
| **目的** | セレクタだけでは判断できない視覚的操作性を分析 |
| **手法** | DOM/CSS抽出 + ルールベース + テキストLLM |
| **Visionとの違い** | 軽量、高速、任意のLLMで動作 |
| **出力** | スコア（0-1）、理由 |
| **性能** | 20要素で1-3秒 |
