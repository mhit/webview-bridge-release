# Decisions Log

> プロジェクトで行った重要な意思決定の記録

フォーマット:
```markdown
## D{YYYY-MM-DD}-{タイトル}

**日付**: {日付}
**ステータス**: {Accepted | Deprecated | Superseded by D{ID}}

### 意思決定

{意思決定の内容}

### 理由

{その決定に至った理由}

### 結果

{その結果生じた影響}
```

---

## D2025-02-04-001: Rust での実装

**日付**: 2025-02-04
**ステータス**: Accepted

### 意思決定

WebView Bridge を Rust で実装する。

### 理由

- **安全性**: 所有権システムによるメモリ安全性の保証
- **パフォーマンス**: ゼロコスト抽象化と並列処理
- **WebView2 サポート**: `webview2-com` クレートが利用可能
- **WSL2 との相性**: WSL2 から Windows バイナリを呼び出しやすい

### 結果

- プロジェクトは Rust 2021 Edition で実装されている
- 依存関係は Axum + Tokio + webview2-com

---

## D2025-02-04-002: webview2-com のバージョン固定

**日付**: 2025-02-04
**ステータス**: Accepted

### 意思決定

`webview2-com` をバージョン `0.19.1` で固定し、関連する Windows クレートも `0.39.0` で統一する。

### 理由

- Windows クレートのバージョン不一致によるコンパイルエラーを回避
- `windows-implement` を同バージョンで組み合わせることで依存関係グラフを安定化

### 結果

- `Cargo.toml` でバージョンを明示的に指定
- 依存関係の競合が解消された

---

## D2025-02-04-003: Solo モードでの開発

**日付**: 2025-02-04
**ステータース**: Accepted

### 意思決定

Claude Code Solo モードで開発を進める。

### 理由

- 個人プロジェクトであるため
- シンプルなワークフローで十分
- 2-Agent モードのオーバーヘッドが不要

### 結果

- `AGENTS.md` に Solo モードの設定が記載されている
- Claude Code が実装・テスト・検証を担当

---

## D2025-02-04-004: OpenClaw 互換 API

**日付**: 2025-02-04
**ステータス**: Accepted

### 意思決定

OpenClaw の browser tool と互換性のある API を提供する。

### 理由

- 既存の OpenClaw ワークフローとの統合を容易にする
- ユーザーが新しい API を学ぶ必要がない
- 移行コストを最小化する

### 結果

- API 設計は OpenClaw の snapshot/act API に準拠
- REST + WebSocket のハイブリッド通信方式

---

## D2025-02-04-005: マルチプロトコル対応 (MCP/ACP/Puppeteer)

**日付**: 2025-02-04
**ステータス**: Accepted

### 意思決定

WebView Bridge を以下のプロトコルに対応させる：

1. **MCP (Model Context Protocol)** サーバーとして実装
2. **ACP (Agent Communication Protocol)** 対応
3. **Puppeteer 互換** API レイヤー

### 理由

- **MCP**: Claude/OpenClaw が直接利用可能になり、ミドルウェアが不要になる
- **ACP**: マルチエージェント協調と権限管理の標準化
- **Puppeteer 互換**: 既存のスクレイピング知識/コードを再利用可能に

### 結果

- 単一の実装で複数のインターフェースを提供
- エコシステムとの統合性が大幅に向上
- 将来の拡張性が確保される

---

（以降、新しい意思決定を追記してください）
