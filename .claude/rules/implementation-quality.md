# Implementation Quality Rules

**Version**: 1.0.0

> 形骸化実装防止ルール - Claude Code がテストをパスさせるために「動くけど意味がない」コードを書くことを防ぎます。

---

## 🔴 禁止事項

以下の実装は**絶対に禁止**されます：

### 1. ハードコードされた戻り値

テストで期待される値を直接返すだけの実装：

```rust
// ❌ 禁止
pub fn calculate_tax(price: f64, rate: f64) -> f64 {
    10.0  // テストの期待値を直接返す
}

// ✅ 正しい
pub fn calculate_tax(price: f64, rate: f64) -> f64 {
    price * rate
}
```

### 2. テスト期待値のコピペ

テストコードの値をそのまま返す：

```rust
// テストコード
#[test]
fn test_add() {
    assert_eq!(add(1, 2), 3);
}

// ❌ 禁止
pub fn add(a: i32, b: i32) -> i32 {
    3  // テストの期待値 3 を直接返す
}

// ✅ 正しい
pub fn add(a: i32, b: i32) -> i32 {
    a + b
}
```

### 3. 空実装・スタブ

ログ出力だけ、または何もしない実装：

```rust
// ❌ 禁止
pub async fn navigate_to(url: &str) -> Result<()> {
    println!("Navigating to: {}", url);
    Ok(())  // 実際の処理なし
}

// ✅ 正しい
pub async fn navigate_to(url: &str) -> Result<()> {
    // 実際のナビゲーション処理
    webview.navigate(url)?;
    Ok(())
}
```

### 4. 常に成功する実装

エラーケースを無視して常に `Ok` を返す：

```rust
// ❌ 禁止
pub fn connect_to_server(host: &str) -> Result<Connection> {
    Ok(Connection::fake())  // 常に成功
}

// ✅ 正しい
pub fn connect_to_server(host: &str) -> Result<Connection> {
    let stream = TcpStream::connect(host)?;
    Ok(Connection::new(stream))
}
```

### 5. テスト条件を満たすだけの条件分岐

特定の入力だけ正しく動くようにした実装：

```rust
// ❌ 禁止
pub fn process(value: i32) -> i32 {
    if value == 42 {
        100  // テストケースだけ対応
    } else {
        0    // その他は適当
    }
}

// ✅ 正しい
pub fn process(value: i32) -> i32 {
    value * 2 + 16  // 一般的な実装
}
```

---

## ✅ 正しい実装の原則

### 1. ビジネスロジックを実装する

```rust
// ✅ 正しい: ドメインロジックに基づいた実装
pub fn calculate_discount(total: f64, customer_level: CustomerLevel) -> f64 {
    match customer_level {
        CustomerLevel::Gold => total * 0.2,
        CustomerLevel::Silver => total * 0.1,
        CustomerLevel::Regular => 0.0,
    }
}
```

### 2. エラー処理を適切に行う

```rust
// ✅ 正しい: エラーを適切に伝播
pub async fn fetch_url(url: &str) -> Result<String> {
    let response = reqwest::get(url).await?;
    if !response.status().is_success() {
        return Err(Error::HttpError(response.status()));
    }
    Ok(response.text().await?)
}
```

### 3. 境界条件を考慮する

```rust
// ✅ 正しい: エッジケースを考慮
pub fn safe_divide(a: f64, b: f64) -> Option<f64> {
    if b.abs() < f64::EPSILON {
        None
    } else {
        Some(a / b)
    }
}
```

### 4. ドキュメントを書く

```rust
// ✅ 正しい: ドキュメント付き
/// セッションを作成し、指定されたURLにナビゲートします。
///
/// # エラー
///
/// - WebView2の初期化に失敗した場合
/// - 無効なURLが指定された場合
pub async fn create_session(url: &str) -> Result<Session> {
    // ...
}
```

---

## 🚨 検出シグナル

以下のパターンは**形骸化実装の可能性が高い**です：

- `return 42;` や `return 0;` のようなマジックナンバー
- `Ok(())` だけの関数本体
- `println!` だけの実装
- `if x == EXPECTED { result } else { 0 }` のような条件
- `// TODO: implement` コメントが残っているコード

---

## 📋 実装前チェックリスト

実装を開始する前に：

- [ ] 関数の目的を理解している
- [ ] 必要なエラーケースを把握している
- [ ] 適切なデータ構造を選択している
- [ ] 既存のコードと一貫性がある

## 📋 実装後チェックリスト

実装完了後：

- [ ] `cargo clippy` が警告なし
- [ ] `cargo fmt` でフォーマット済み
- [ ] `cargo test` がすべてパス
- [ ] エッジケースをテストした
- [ ] ドキュメントコメントを書いた

---

## 💡 ヒント

### 実装が難しい場合

1. **まずテストを読む**: 何を期待されているか理解する
2. **既存の類似コードを見る**: パターンを学ぶ
3. **小さく始める**: 最小限の動作から始めて拡張する
4. **ドキュメントを参照**: 標準ライブラリやクレートのドキュメントを読む

### どうしても実装できない場合

```rust
// ❌ 禁止: 空実装でごまかす
pub async fn complex_feature() -> Result<()> {
    Ok(())
}

// ✅ 正しい: 未実装を明示する
pub async fn complex_feature() -> Result<()> {
    Err(Error::Unimplemented("complex_feature".to_string()))
}
```
