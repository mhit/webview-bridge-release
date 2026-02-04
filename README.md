# WebView Bridge

Windows上で動作するWebView2ベースのWebスクレイピングサーバー。OpenClawと連携し、安定したブラウザ自動化を提供。

## 🎯 なぜWebView Bridge?

| 既存手法 | 問題点 |
|---------|--------|
| Puppeteer/Chrome | WSL2で不安定、ゾンビプロセス、Cloudflareブロック |
| Browser Relay | 手動タブアタッチ必要、常時ブラウザ起動前提 |
| HttpClient直接 | Cloudflareブロック、JS実行不可 |

**WebView Bridge** は:
- ✅ Cloudflare/Bot検出を自然にバイパス
- ✅ Windowsネイティブで安定動作
- ✅ ヘッドレス実行可能
- ✅ プロファイルでログイン状態を永続化
- ✅ OpenClawのbrowser toolと互換API

## 📐 アーキテクチャ

```
OpenClaw (WSL2)          WebView Bridge (Windows)
┌─────────────┐          ┌─────────────────────────┐
│   Agent     │─HTTP/WS─▶│  HTTP API Server        │
│  (Claude)   │          │  ├─ Session Manager     │
└─────────────┘          │  └─ WebView2 Pool       │
                         └───────────┬─────────────┘
                                     │
                         ┌───────────▼─────────────┐
                         │  Edge WebView2 Runtime  │
                         └─────────────────────────┘
```

## 🚀 主要機能

- **セッション管理**: 複数WebView2インスタンスの並列実行
- **プロファイル分離**: サイトごとにCookie/認証を分離
- **REST API**: navigate, evaluate, screenshot, cookie管理
- **WebSocket**: リアルタイムイベント通知
- **OpenClaw互換**: snapshot, act APIを提供

## 📖 ドキュメント

- [設計書](docs/design.md) - アーキテクチャ、API仕様、技術詳細
- [ユースケース](docs/use-cases.md) - 具体的な利用シナリオ

## 🛣️ ロードマップ

- [ ] Phase 1: MVP (基本API、単一セッション)
- [ ] Phase 2: 機能拡充 (プール、プロファイル、WebSocket)
- [ ] Phase 3: OpenClaw統合
- [ ] Phase 4: 安定化、インストーラ

## 📝 License

MIT
