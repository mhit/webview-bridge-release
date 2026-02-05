# WBP2 テスト戦略計画書

> 作成日: 2026-02-05
> 目的: 実際のユースケースとワークフローを考慮した包括的なテスト設計

---

## 1. 現状分析

### 1.1 現在のテスト状況

| モジュール | ユニットテスト | 統合テスト | E2Eテスト |
|-----------|--------------|-----------|----------|
| session_v2 | ✅ 基本あり | ❌ なし | ❌ なし |
| wait_v2 | ✅ 基本あり | ❌ なし | ❌ なし |
| screenshot_v2 | ✅ 基本あり | ❌ なし | ❌ なし |
| goal | ✅ 基本あり | ❌ なし | ❌ なし |
| macro_engine | ✅ 基本あり | ❌ なし | ❌ なし |
| media | ✅ 基本あり | ❌ なし | ❌ なし |
| ai | ✅ 基本あり | ❌ なし | ❌ なし |
| download | ✅ 基本あり | ❌ なし | ❌ なし |
| comm | ✅ 基本あり | ❌ なし | ❌ なし |
| api_v2 | ✅ 基本あり | ❌ なし | ❌ なし |

### 1.2 テストの欠落点

1. **統合テスト**: モジュール間の連携テストがない
2. **E2Eテスト**: 実際のブラウザを使用したテストがない
3. **ワークフローテスト**: 複数APIを組み合わせたシナリオテストがない
4. **エラーハンドリングテスト**: 異常系のテストが不足
5. **並行性テスト**: 複数セッション/リクエストの同時処理テストがない

---

## 2. ユースケース分析

### 2.1 主要ユースケース

#### UC-1: ECサイト商品データ収集
```
1. セッション取得
2. ログイン（AI支援またはマクロ）
3. 商品一覧ページへ移動
4. ページネーション処理（SPAスクロール）
5. 各商品の詳細情報抽出
6. 画像一括ダウンロード
7. データ構造化
8. セッション解放
```

**テスト観点**:
- ログイン失敗時のリトライ
- ページネーション中のエラー回復
- 大量画像ダウンロード時のメモリ管理
- 途中中断からの再開可能性

#### UC-2: 競合分析ワークフロー
```
1. 複数セッション（並行処理用）
2. 競合サイトA/B/Cを同時巡回
3. 価格・在庫情報の抽出
4. スクリーンショット取得
5. AI画像分析（商品品質比較）
6. レポート生成
```

**テスト観点**:
- 複数セッションの排他制御
- セッション間のリソース競合
- AI APIレート制限対応
- 部分失敗時の継続/中断判断

#### UC-3: フォーム自動入力
```
1. セッション取得
2. 入力フォームへ移動
3. 要素検出（待機）
4. データ入力（マクロ）
5. バリデーションエラー検出
6. 送信・確認
7. 結果スクリーンショット
```

**テスト観点**:
- 動的フォーム（JS生成）への対応
- バリデーションエラーの検出精度
- CAPTCHA出現時の対応
- タイムアウト設定の適切性

#### UC-4: 定期監視ワークフロー
```
1. 定期実行（外部cron）
2. 状態チェック（在庫/価格変動）
3. 変化検知時のWebhook通知
4. 証跡としてのスクリーンショット保存
5. ストレージローテーション
```

**テスト観点**:
- Webhook通知の信頼性
- ストレージクリーンアップの正確性
- 長時間稼働の安定性

---

## 3. テストレベル設計

### 3.1 レベル1: ユニットテスト（Unit Tests）

**目的**: 個別関数・構造体の正確性検証

**対象と方針**:

| カテゴリ | テスト内容 | 優先度 |
|---------|-----------|-------|
| 構造体のシリアライズ | JSON変換の正確性 | 高 |
| バリデーション | 入力値検証ロジック | 高 |
| ヘルパー関数 | ユーティリティ関数の動作 | 中 |
| エラー変換 | エラーコード・メッセージ | 中 |

**既存テストの拡充ポイント**:
- `SensitiveDataMasker` のエッジケース（国際電話番号、日本語メールアドレス）
- `FlowExecutor` の依存関係解決ロジック
- `DownloadManager` のバッチ処理における順序保証

### 3.2 レベル2: 統合テスト（Integration Tests）

**目的**: モジュール間連携の検証

**テストシナリオ**:

```rust
// tests/integration/session_workflow.rs

#[tokio::test]
async fn test_session_acquire_wait_release() {
    // 1. セッション取得
    // 2. wait実行
    // 3. セッション状態確認
    // 4. セッション解放
    // 5. 解放後のアクセス拒否確認
}

#[tokio::test]
async fn test_session_lease_timeout() {
    // 1. セッション取得（短いlease_seconds）
    // 2. lease期限まで待機
    // 3. 自動解放の確認
}

#[tokio::test]
async fn test_concurrent_session_limit() {
    // 1. max_sessionまでセッション取得
    // 2. 追加取得がキュー待ちになることを確認
    // 3. 1つ解放後、待機中のリクエストが処理されることを確認
}
```

**モジュール連携マトリクス**:

| 呼び出し元 | 連携先 | テストシナリオ |
|-----------|-------|---------------|
| Goal | Wait | 待機条件付きゴール実行 |
| Goal | Screenshot | ゴール完了後スクリーンショット |
| Macro | Wait | マクロ実行前の要素待機 |
| Media | Download | 画像収集→ダウンロード |
| AI | Screenshot | 画面キャプチャ→AI分析 |
| Batch | 複数API | 依存関係付きバッチ実行 |
| Job | Webhook | ジョブ完了→Webhook通知 |

### 3.3 レベル3: APIテスト（HTTP層）

**目的**: REST APIの動作検証

**テストツール選択**:
- **axum-test**: Rust純正、高速
- **tower-test**: サービス層テスト

**テストパターン**:

```rust
// tests/api/v2_endpoints.rs

#[tokio::test]
async fn test_session_acquire_returns_201() {
    let app = create_test_app();
    let response = app
        .post("/v2/session/acquire")
        .json(&json!({"name": "test", "profile": "default"}))
        .await;
    
    assert_eq!(response.status(), StatusCode::CREATED);
    let body: Value = response.json().await;
    assert!(body["success"].as_bool().unwrap());
}

#[tokio::test]
async fn test_session_not_found_returns_404() {
    let app = create_test_app();
    let response = app
        .get("/v2/session/nonexistent")
        .await;
    
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let body: Value = response.json().await;
    assert_eq!(body["error"]["code"], "WBP2_001");
}
```

**エラーコードテストマトリクス**:

| エラーコード | 条件 | 期待レスポンス |
|-------------|-----|---------------|
| WBP2_001 | 存在しないセッション | 404 |
| WBP2_002 | ロック中セッション | 409 |
| WBP2_003 | セッションタイムアウト | 408 |
| WBP2_010 | 要素未発見 | 404 |
| WBP2_011 | 待機タイムアウト | 408 |
| WBP2_090 | 無効リクエスト | 400 |
| WBP2_099 | 内部エラー | 500 |
| WBP2_100 | AI未設定 | 503 |
| WBP2_120 | ジョブ未発見 | 404 |

### 3.4 レベル4: E2Eテスト（End-to-End）

**目的**: 実際のブラウザを使用した動作検証

**テスト環境**:
```
テスト用HTML
├── login.html          (ログインフォーム)
├── product_list.html   (商品一覧、ページネーション)
├── product_detail.html (商品詳細)
├── infinite_scroll.html (無限スクロール)
├── spa_app.html        (SPA、履歴API)
└── error_pages/
    ├── 404.html
    └── 500.html
```

**テストシナリオ**:

```rust
// tests/e2e/login_flow.rs

#[tokio::test]
#[ignore] // 実ブラウザが必要
async fn test_login_with_macro() {
    // 1. テストサーバ起動
    // 2. WBP2サーバ起動
    // 3. セッション取得
    // 4. ログインページへ移動
    // 5. マクロでフォーム入力
    // 6. 送信・結果確認
    // 7. クリーンアップ
}
```

**実行条件**:
- CI環境: スキップ（`#[ignore]`）
- ローカル: 明示的に実行（`cargo test -- --ignored`）
- 専用E2E環境: 完全実行

---

## 4. ワークフローテスト設計

### 4.1 シナリオベーステスト

**Scenario 1: ECサイトスクレイピング**

```yaml
name: ec_site_scraping
description: ECサイトから商品情報を収集する完全フロー
steps:
  - action: session/acquire
    expect: success
    save: session_name
  
  - action: goal/execute
    params:
      goal: "navigate to login page"
    expect: success
  
  - action: wait
    params:
      selector: "#login-form"
    expect: success
  
  - action: macro/execute
    params:
      macro: "login"
      args: { username: "test", password: "test" }
    expect: success
  
  - action: goal/execute
    params:
      goal: "navigate to product list"
    expect: success
  
  - action: ai/extract
    params:
      description: "商品名、価格、在庫状態を抽出"
    expect: 
      type: array
      min_length: 1
  
  - action: media/images
    params:
      selector: ".product-image"
      output_format: "files"
    expect:
      type: object
      has_key: file_ref
  
  - action: session/release
    expect: success
```

**Scenario 2: エラー回復テスト**

```yaml
name: error_recovery
description: 各種エラーからの回復フロー
cases:
  - name: network_timeout_retry
    inject_fault: network_delay(5000ms)
    expect: retry_and_succeed
  
  - name: element_not_found_alternative
    inject_fault: remove_element("#primary-btn")
    fallback: click("#secondary-btn")
    expect: success
  
  - name: captcha_detection_pause
    inject_fault: show_captcha()
    expect: status=captcha_detected
    user_action: solve_captcha
    then: continue_and_succeed
```

### 4.2 長時間安定性テスト

**目的**: メモリリーク、リソース枯渇の検出

```rust
#[tokio::test]
#[ignore]
async fn test_long_running_stability() {
    let start_memory = get_process_memory();
    
    for i in 0..1000 {
        // セッション取得・操作・解放を繰り返す
        let session = acquire_session().await;
        execute_simple_workflow(&session).await;
        release_session(session).await;
        
        if i % 100 == 0 {
            let current_memory = get_process_memory();
            let growth = current_memory - start_memory;
            assert!(growth < 100_000_000, "Memory leak detected: {}MB", growth / 1_000_000);
        }
    }
}
```

---

## 5. テストデータ設計

### 5.1 テストフィクスチャ

```
tests/fixtures/
├── profiles/
│   ├── default.json       (標準プロファイル)
│   └── headless.json      (ヘッドレスプロファイル)
├── macros/
│   ├── login.json         (ログインマクロ)
│   └── search.json        (検索マクロ)
├── pages/
│   ├── simple.html        (単純なページ)
│   ├── dynamic.html       (動的コンテンツ)
│   └── spa.html           (SPA)
└── expected/
    ├── screenshot_1.png   (期待スクリーンショット)
    └── extract_result.json (期待抽出結果)
```

### 5.2 モックサービス

```rust
// tests/mocks/ai_service.rs

pub struct MockGeminiService {
    responses: HashMap<String, Value>,
}

impl MockGeminiService {
    pub fn with_login_detection() -> Self {
        Self {
            responses: hashmap! {
                "login_form" => json!({
                    "elements": [
                        {"type": "username", "selector": "#user"},
                        {"type": "password", "selector": "#pass"},
                        {"type": "submit", "selector": "#login-btn"}
                    ]
                })
            }
        }
    }
}
```

---

## 6. テスト優先度とロードマップ

### 6.1 優先度マトリクス

| カテゴリ | テスト数(予想) | 重要度 | 実装順序 |
|---------|--------------|-------|---------|
| API エンドポイント基本動作 | 38 | 🔴 高 | 1st |
| エラーハンドリング | 25 | 🔴 高 | 2nd |
| セッション管理 | 15 | 🔴 高 | 3rd |
| ワークフロー統合 | 10 | 🟡 中 | 4th |
| 並行性・競合 | 8 | 🟡 中 | 5th |
| E2E シナリオ | 5 | 🟢 低 | 6th |
| 長時間安定性 | 2 | 🟢 低 | 7th |

### 6.2 実装ロードマップ

```
Week 1: 基盤整備
├── テストユーティリティ作成
├── モックサービス実装
└── テストサーバ構築

Week 2: レベル1-2（ユニット・統合）
├── 既存ユニットテスト拡充
├── モジュール間連携テスト
└── エラーハンドリングテスト

Week 3: レベル3（API）
├── 全38エンドポイントの正常系テスト
├── 異常系テスト（エラーコード検証）
└── 認証・認可テスト（将来対応）

Week 4: レベル4（E2E・ワークフロー）
├── テストHTMLページ作成
├── シナリオテスト実装
└── 長時間テスト実装
```

---

## 7. テスト実行環境

### 7.1 ローカル開発環境

```bash
# ユニットテスト（高速）
cargo test --lib

# 統合テスト
cargo test --test integration

# 全テスト（E2E除く）
cargo test

# E2Eテスト（明示的）
cargo test -- --ignored
```

### 7.2 CI/CD環境

```yaml
# .github/workflows/test.yml
name: WBP2 Tests
on: [push, pull_request]

jobs:
  unit-tests:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4
      - run: cargo test --lib

  integration-tests:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4
      - run: cargo test --test integration

  # E2Eテストは手動トリガーまたは定期実行
  e2e-tests:
    if: github.event_name == 'workflow_dispatch'
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4
      - run: cargo test -- --ignored
```

---

## 8. 決定事項と次のアクション

### 8.1 決定事項

1. **テストレベル**: 4段階（ユニット→統合→API→E2E）
2. **優先度**: APIエンドポイント > エラーハンドリング > ワークフロー
3. **E2Eテスト**: **毎コミット実行**（CI必須）
4. **テストデータ**: フィクスチャ + モックサービス
5. **Gemini API テスト**: **実際のAPIを使用**（環境変数で切替可能）
6. **テストカバレッジ目標**: **90%**
7. **パフォーマンステスト**: **必須**（レスポンスタイム、スループット）

### 8.2 パフォーマンステスト基準

| 指標 | 目標値 | 測定方法 |
|-----|-------|---------|
| API レスポンスタイム (P50) | < 50ms | criterion ベンチマーク |
| API レスポンスタイム (P99) | < 200ms | criterion ベンチマーク |
| セッション取得 | < 100ms | E2E計測 |
| スクリーンショット取得 | < 500ms | E2E計測 |
| 同時セッション処理 | 10並列 | 負荷試験 |
| メモリ増加率 | < 10MB/1000操作 | 長時間テスト |

### 8.3 次のアクション

- [ ] テストユーティリティモジュール作成 (`tests/common/mod.rs`)
- [ ] Gemini API 統合テスト環境構築
- [ ] テストサーバフレームワーク構築
- [ ] APIエンドポイント正常系テスト実装開始
- [ ] criterion ベンチマークセットアップ
- [ ] CI/CD パイプラインにE2E・パフォーマンステスト追加

---

## 9. 解決済み確認事項

| 質問 | 決定 |
|-----|------|
| Gemini API テスト | ✅ **実際のAPIを使用**（モック併用、環境変数切替） |
| テストカバレッジ目標 | ✅ **90%** |
| E2E テスト頻度 | ✅ **毎コミット** |
| パフォーマンステスト | ✅ **必須** |

---

## 10. CI/CD 更新計画

```yaml
# .github/workflows/test.yml (更新版)
name: WBP2 Tests
on: [push, pull_request]

env:
  GEMINI_API_KEY: ${{ secrets.GEMINI_API_KEY }}

jobs:
  unit-tests:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4
      - run: cargo test --lib
      - run: cargo llvm-cov --lcov --output-path lcov.info
      - uses: codecov/codecov-action@v4
        with:
          files: lcov.info
          fail_ci_if_error: true
          threshold: 90%  # 90%カバレッジ必須

  integration-tests:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4
      - run: cargo test --test integration

  e2e-tests:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4
      - run: cargo test -- --ignored  # 毎コミット実行

  performance-tests:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4
      - run: cargo bench
      - uses: actions/upload-artifact@v4
        with:
          name: benchmark-results
          path: target/criterion/
```

