# Changelog

All notable changes to WebView Bridge will be documented in this file.

## [3.5.0] - 2026-02-07

### 🚀 New Features

#### Visual Interactivity Analysis（視覚的操作性分析）
- **Interactivity Scoring**: 各要素に0-1のインタラクティビティスコアを付与
  - DOM/CSS分析によるルールベーススコアリング
  - UI法則に基づく位置ボーナス（viewport, F-pattern, main/footer）
  - テキストLLMによる追加分析（スコア0.3-0.8の要素）
  
- **Predicted Actions**: 要素ごとに予測アクションを返却
  - `click_navigate`, `click_submit`, `type_input`, `hover_reveal` 等
  - AIが次に何をすべきか明確に指示

#### CAPTCHA/Challenge Detection（チャレンジ自動検出）
- **対応チャレンジ**:
  - Cloudflare Turnstile
  - Cloudflare Interstitial ("Just a moment")
  - Google reCAPTCHA v2/v3
  - hCaptcha
  
- **AI自律対応戦略**:
  - `proceed`: 自動処理可能（介入不要）
  - `wait_and_retry`: 待機後リトライ
  - `click_checkbox`: チェックボックスをクリック
  - 人間介在なしでAIが自律的に対処

#### Vision LLM Integration（準備完了）
- `analyze_vision` オプション追加
- alt属性のない画像要素をVision LLMで分析
- 画像内容から説明とアクションを生成

### 📊 Output Optimization
- 低スコア要素（< 0.4）をフィルタリング
- 上位20件に出力制限
- セレクタとラベルの短縮
- 要素数表示（総数 / 表示数）

### 🔧 MCP Tool Updates
- `capture` ツールの説明を更新
- `analyze_vision` パラメータ追加
- チャレンジ検出結果を出力に含む

### 📖 Documentation
- `docs/visual_interactivity_design.md` - 設計書作成
- `README.md` - CAPTCHA対応セクション追加
- MCP定義更新

---

## [0.1.0] - Initial Release

- Basic MCP tools: session, navigate, capture, interact, extract, execute, media, agent
- Robustness layer with auto-wait, auto-scroll, retries
- human_mode for bot detection evasion
- Agentic mode with local LLM support
