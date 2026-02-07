# WebView Bridge v3.5.0 - AI自律運用強化リリース 🚀

## 概要

WebView Bridgeがv0.2.0になりました。AIエージェントがWebを自律的に操作する際の重要な課題を解決する機能を追加しています。

## 🎯 ハイライト

### 1. Visual Interactivity Analysis（視覚的操作性分析）

**問題**: セレクタだけでは「クリックできる要素」が判断できない

**解決**: DOM/CSS分析でインタラクティビティスコア（0-1）を算出

```
【操作可能要素】
(50要素中 上位20件)
- #nav-hamburger-menu [score:1.00] 82×39 「メニューを開く」 → click_navigate
- #twotabsearchtextbox [score:0.95] type=text 「検索」 → type_input
- div.product-image [score:0.85] 300×200 「商品画像」 → click_product
```

### 2. CAPTCHA/チャレンジ自動対応 🔐

**問題**: Cloudflare/reCAPTCHA/hCaptchaでAIが止まる

**解決**: 自動検出 + AI向け対処戦略を返却

```
【チャレンジ検出】
- cloudflare_interstitial: 待機推奨 (15000ms後リトライ)
- google_recaptcha_v2: → click .recaptcha-checkbox で解決試行可
```

**対応チャレンジ**:
| タイプ | 自動対応 |
|--------|---------|
| Cloudflare | 待機→リトライ |
| reCAPTCHA v2 | チェックボックスクリック |
| reCAPTCHA v3 | 自動処理（介入不要） |
| hCaptcha | チェックボックスクリック |

### 3. 設計思想：人間介在なし

**重要**: MCPはAIが使うため、「人間による操作が必要」は禁止語

全てのチャレンジに `auto_strategy` を付与し、AIが自律的に対処可能：
- `proceed`: そのまま続行
- `wait_and_retry`: 待機後リトライ
- `click_checkbox`: チェックボックスをクリック

## 📦 使用例

```javascript
// チャレンジ検出と対処
const result = await capture({ session: "test" });

if (result.challenges?.length > 0) {
  for (const challenge of result.challenges) {
    if (challenge.auto_strategy.action === "wait_and_retry") {
      await wait({ timeout_ms: challenge.auto_strategy.timeout_ms });
      // リトライ
    } else if (challenge.auto_strategy.action === "click_checkbox") {
      await interact({ 
        actions: [{ type: "click", target: challenge.auto_strategy.selector }]
      });
    }
  }
}
```

## 🔗 更新方法

```bash
git pull
cargo build --release
```

## 📖 ドキュメント

- [設計書](docs/visual_interactivity_design.md)
- [CHANGELOG](CHANGELOG.md)
- [README](README.md)

---

**質問・フィードバック歓迎！**
