# Walkthrough: CDP Human Mouse Simulation Implementation

CDP (Chrome DevTools Protocol) を用いた人間らしい入力シミュレーションの実装詳細です。

## 変更ファイル

### 1. `src/webview/webview_instance.rs`
- **`click_cdp`**: ベジエ曲線アルゴリズム、ジッター、オーバーシュートを実装。
- **`type_cdp`**: Typoシミュレーション、可変遅延、修正動作を実装。
- **`click_selector_cdp`**: `human_mode` 引数を追加。

### 2. `src/core/mod.rs`
- `AppCommand` と `SessionCommand` の `ClickCdp`, `TypeCdp` バリアントに `human_mode: bool` を追加。
- `CoreManager` のメソッドシグネチャを更新。

### 3. `src/main.rs`
- イベントループ内のコマンドパース処理を更新し、`human_mode` を伝搬。

### 4. `src/mcp_v3/tools.rs`
- `interact` ツールハンドラ内で、`options.human_mode` を読み取り、CDPコマンド発行時にフラグをセットするように変更。
- フォーカス用のクリックには `human_mode: false` (即時クリック) を使用するように区別。

### 5. `src/mcp_v3/robustness.rs`
- JavaScriptで実装されていた古いシミュレーションコード (`generate_human_mouse_move_script` 等) を削除/コメントアウト。

## 技術的ハイライト

### ベジエ曲線のRust実装
```rust
// 制御点の計算（垂直オフセット）
let perp_x = -(y - start_y) / distance * curviness * distance;
let perp_y = (x - start_x) / distance * curviness * distance;

// 3次ベジエ補間
let bezier = |t: f64, p0: f64, p1: f64, p2: f64, p3: f64| -> f64 {
    let u = 1.0 - t;
    u.powi(3) * p0 + 3.0 * u.powi(2) * t * p1 + 3.0 * u * t.powi(2) * p2 + t.powi(3) * p3
};
```
JSの実装を忠実に移植し、物理演算的な挙動を再現しています。

### Bot対策効果
- **mouseMovedイベントの連続送信**: 瞬間移動ではなく、中間の座標イベントを大量に発生させることで、サーバーサイドのトラッキング（例: Cloudflare, Akamai）に対して人間らしさをアピールできます。
- **入力リズムの不規則性**: 機械的な一定間隔の入力を排除し、キーストローク間の時間を統計的に人間らしく分布させています。

## 今後の課題
- ドラッグ&ドロップのCDP実装
- スクロール操作の人間らしい挙動（慣性スクロール等）のCDP化（現在はJS実装）
