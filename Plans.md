# WebView Bridge - 開発計画

> 最終更新: 2025-02-04
>
> このファイルで開発の進捗を管理します。

---

## 🔖 マーカー凡例

| マーカー | 意味 | 状態遷移 |
|---------|------|----------|
| `cc:TODO` | 未着手 | → `cc:WIP` |
| `cc:WIP` | 作業中 | → `cc:完了` |
| `cc:完了` | 完了 | - |
| `cc:ブロック中` | ブロッカーあり | → `cc:WIP` |

---

## 📊 進捗概要

| フェーズ | 状態 | 進捗 |
|---------|------|------|
| Phase 1: MVP 基盤 | ✅ 完了 | 100% |
| Phase 2: 機能拡充 | ✅ 完了 | 100% |
| Phase 3: OpenClaw統合 | ✅ 完了 | 100% |
| Phase 4: 実運用テスト統合 | ✅ 完了 | 100% |

---

## ✅ Phase 1: MVP 基盤構築 `cc:完了`

### 1.1 プロジェクト初期化 `cc:完了`

- [x] Rust プロジェクト作成
- [x] 依存関係の追加（Axum, Tokio, WebView2）
- [x] 基本ディレクトリ構造

### 1.2 Win32 ウィンドウ作成 `cc:完了`

- [x] Win32 API バインディングの設定
- [x] ウィンドウクラスの登録
- [x] メッセージループの実装
- [x] テスト: ウィンドウが正常に作成されること

### 1.3 SessionManager 実装 `cc:完了`

- [x] SessionManager 構造体の定義
- [x] セッションの作成・削除
- [x] セッション一覧取得
- [x] ユニットテスト

### 1.4 WebView2 インスタンス管理 `cc:完了`

- [x] WebViewInstance 構造体の実装
- [x] WebView2 環境の初期化
- [x] コントロールの作成とウィンドウへのアタッチ
- [x] ナビゲーション機能
- [x] スナップショット機能 (HTML/ARIA/Text)
- [x] Cookie 管理 (取得/設定)
- [x] 統合テスト通過 (Windows環境)

### 1.5 HTTP API 実装 `cc:完了`

- [x] Axum サーバーの起動
- [x] セッション作成エンドポイント: `POST /create`
- [x] ナビゲート エンドポイント: `POST /navigate/:id`
- [x] スクリプト実行 エンドポイント: `POST /execute/:id`
- [x] ステータス取得エンドポイント: `GET /status/:id`
- [x] セッション削除 エンドポイント: `DELETE /close/:id`
- [x] OpenClaw Act API: `POST /act/:id`
- [x] エラーハンドリングとレスポンス形式

### 1.6 統合テスト `cc:完了`

- [x] API の E2E テスト
- [x] セッションライフサイクルのテスト
- [x] ナビゲーションとスクリプト実行のテスト
- [x] Act API (クリック/入力) のテスト

---

## ✅ Phase 2: 機能拡充 `cc:完了`

### 2.1 WebView2 統合 `cc:完了`

- [x] main.rs を Win32 メッセージループベースにリファクタリング
- [x] SessionManager が実際の WebViewInstance を管理するように変更
- [x] 各セッションは専用スレッドで実行（COM STA 要件対応）
- [x] WM_CHECK_QUEUE メッセージでコマンド処理
- [x] プロファイル管理（ProfileManager 実装済み）
- [x] async/await によるデッドロックとパニックの解消

### 2.2 セッションプール `cc:完了`

- [x] SessionHandleにlast_accessedフィールド追加
- [x] コマンド実行時にアクセス時刻を自動更新
- [x] SessionManagerにidle_timeout_secs設定追加
- [x] cleanup_idle_sessionsメソッド実装
- [x] get_session_count、list_sessions、get_idle_timeoutユーティリティ追加
- [x] デフォルトアイドルタイムアウト: 5分

## 📝 実装中のタスク

現在作業中のタスク:
- **Phase 4.1**: テストインフラ整備

### ブロッカー

なし

---

## 🟡 Phase 3: OpenClaw統合 `cc:WIP`

### 3.1 Snapshot API `cc:完了`

- [x] GET /snapshot/:id エンドポイント追加
- [x] スナップショットタイプ（html, text, aria）のサポート
- [x] OpenClaw形式のレスポンス

### 3.2 Screenshot API `cc:完了`

- [x] GET /screenshot/:id エンドポイント追加
- [x] Base64エンコードされた画像返却 (html2canvas利用)
- [x] DOM要素キャプチャ

### 3.3 Cookie管理API `cc:完了`

- [x] GET /cookies/:id - Cookie取得
- [x] POST /cookies/:id - Cookie設定

### 3.4 OpenClaw完全互換テスト `cc:TODO`

- [ ] OpenClawからの実際の呼び出しテスト
- [ ] エラーハンドリングの互換性確認

### 3.5 Wait for Selector API `cc:完了`

- [x] POST /wait/:id エンドポイント追加
- [x] タイムアウト設定
- [x] セレクターが見つからない場合のエラーハンドリング

### 3.6 Extract API `cc:完了`

- [x] POST /extract/:id エンドポイント追加
- [x] 複数要素取得 (extractAll)
- [x] 属性取得 (text, href, src, etc.)

---

## 📋 Phase 4: 実運用テスト統合 `cc:TODO`

> 参照: docs/TEST_CASES.md, docs/IMPLEMENTATION_REFERENCE.md, docs/DEVELOPER_SUMMARY.md

### 4.1 テストインフラ整備 `cc:完了`

- [x] テストフレームワーク選定 (PowerShell)
- [x] テストレポート形式の実装 (JSON)
- [x] CI/CD統合準備 (GitHub Actions)

### 4.2 Phase 1 テスト実装（High優先度）`cc:完了`

- [x] UC-10: ログイン認証テスト
- [x] UC-01: Xエゴサーチテスト
- [x] UC-02: Google Shopping価格調査テスト

### 4.3 Phase 2 テスト実装（Medium優先度）`cc:完了`

- [x] UC-03: Amazon商品確認
- [x] UC-04: Rakuten RMS注文管理
- [x] UC-05: Yahooショッピング商品確認

### 4.4 Phase 3 テスト実装（Low優先度）`cc:完了`

- [x] UC-06: 楽天ブックスレビュークロール
- [x] UC-07: ECサイト広告確認
- [x] UC-08: Adamas公式サイト巡回
- [x] UC-09: ターゲットサイトクロール

---

## � Phase 5: ブラウザ自動化ツール互換性 & MCP/ACP対応 `cc:WIP`

### 5.1 WebDriver Protocol互換レイヤー `cc:完了`

- [x] WebDriver W3C仕様準拠エンドポイント
- [x] /session系API実装
- [x] /element系API実装
- [ ] Selenium接続テスト

### 5.2 MCP (Model Context Protocol) 対応 `cc:完了`

- [x] MCP Server実装
- [x] Tool定義 (browse, click, type, screenshot, extract)
- [x] Resource定義
- [ ] Claude/GPT統合テスト

### 5.3 CDP (Chrome DevTools Protocol) 互換 `cc:TODO`

- [ ] CDP WebSocket実装
- [ ] Page.*ドメイン
- [ ] Runtime.*ドメイン
- [ ] Puppeteer接続テスト

### 5.4 SDK/ドライバー `cc:完了`

- [x] Python SDK (sdk/python/webview_bridge.py)
- [x] JavaScript SDK (sdk/js/webview-bridge.js)
- [x] TypeScript型定義 (sdk/js/webview-bridge.d.ts)

---

## �🔍 最近の完了

- ✅ Phase 4: 実運用テスト統合完了 (2026-02-05)
- ✅ Phase 3.2: Screenshot API実装 (2025-02-05)
- ✅ Phase 3.1: Snapshot API実装 (2025-02-05)
- ✅ 統合テストにおける 500エラーの修正 (async/await化) (2025-02-05)
- ✅ WebView2 コールバック待機時の Win32 メッセージループ実装 (2025-02-05)
- ✅ Windows 側での全統合テスト通過確認 (2025-02-05)
- ✅ Phase 2.1: WebView2 統合 & リファクタリング完了 (2025-02-05)
- ✅ Phase 1: MVP 基盤構築完了 (2025-02-04)

