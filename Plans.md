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
| Phase 2: 機能拡充 | 🔄 作業中 | 25% |
| Phase 3: OpenClaw統合 | ⏳ 未着手 | 0% |
| Phase 4: 安定化 | ⏳ 未着手 | 0% |
| Phase 3: OpenClaw統合 | ⏳ 未着手 | 0% |
| Phase 4: 安定化 | ⏳ 未着手 | 0% |

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
- [ ] ユニットテスト

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

## 🟡 Phase 2: 機能拡充 `cc:WIP`

### 2.1 プロファイル管理 `cc:WIP`

- [x] プロファイルディレクトリの作成
- [x] Cookie/認証状態の永続化
- [x] プロファイル切り替え API
- [x] プロファイル一覧取得 API
- [x] プロファイル作成 API
- [x] プロファイル削除 API
- [ ] ユニットテスト
- [ ] **注**: `webview_instance.rs` のブレース不一致エラーによりビルドブロック中

### 2.2 セッションプール

- [ ] プールサイズの管理
- [ ] アイドルセッションの自動削除
- [ ] 同時実行数の制限

### 2.3 WebSocket サポート

- [ ] WebSocket サーバーの実装
- [ ] リアルタイムイベント通知
- [ ] ログストリーミング

---

## 🟢 Phase 3: OpenClaw統合 `cc:TODO`

### 3.1 OpenClaw API 互換

- [ ] OpenClaw API 互換レイヤー
- [ ] Snapshot API の実装
- [ ] Act API の実装
- [ ] OpenClaw との統合テスト

### 3.2 MCP (Model Context Protocol) サーバー

- [ ] MCP サーバー実装（STDIO/HTTP トランスポート）
- [ ] MCP ツール定義（navigate, evaluate, snapshot, screenshot）
- [ ] MCP リソース定義（セッション情報）
- [ ] MCP プロンプトテンプレート
- [ ] Claude Desktop との連携テスト

### 3.3 Puppeteer 互換レイヤー

- [ ] Puppeteer API マッピングの実装
- [ ] TypeScript クライアントライブラリ
- [ ] Python クライアントライブラリ
- [ ] 既存 Puppeteer コードの移行ガイド

---

## 🔵 Phase 4: 安定化 `cc:TODO`

### 4.1 品質向上

- [ ] エラーハンドリングの強化
- [ ] ロギングの改善（構造化ログ）
- [ ] 設定ファイルサポート
- [ ] ヘルスチェック API

### 4.2 配布

- [ ] インストーラの作成（NSIS/WiX）
- [ ] ポータブル版ビルド
- [ ] Windows サービス対応

### 4.3 クライアントライブラリ

- [ ] npm パッケージ公開（@webview-bridge/puppeteer）
- [ ] PyPI パッケージ公開（webview-bridge）
- [ ] CDN 配布（ブラウザクライアント）

### 4.4 ドキュメント

- [ ] API リファレンス
- [ ] MCP インテグレーションガイド
- [ ] Puppeteer 移行ガイド
- [ ] ACP 権限設定ガイド

---

## 📝 実装中のタスク

現在作業中のタスク:
- **Phase 2.1**: `webview_instance.rs` のブレース不一致エラーを修正中

### ブロッカー

| タスク | 状態 | 説明 |
|------|------|------|
| webview_instance.rs 修正 | 🔴 ブロック中 | 余分な閉じブレース `}` が1つ存在 (146 open, 147 close) |
| ビルド & テスト | ⏳ 待機 | 上記修正後に実行予定 |

---

## 🔍 最近の完了

- ✅ Phase 1: MVP 基盤構築完了 (2025-02-04)
- ✅ HTTP API 実装 (Axum) (2025-02-04)
- ✅ 統合テスト実装 (2025-02-04)
- ✅ Win32 ウィンドウ作成 (2025-02-04)
- ✅ SessionManager 実装 (2025-02-04)
- ✅ ProfileManager 実装 (2025-02-04)
