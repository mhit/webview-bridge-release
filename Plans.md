# WebView Bridge Plans.md

作成日: 2026-04-08

---

## 機能概要: AX Tree Snapshot (format=ax)

**目的**: CDP `Accessibility.getFullAXTree` を使った真のアクセシビリティツリー取得。
DOM 污染なし・iframe シームレス対応・AI 向けセマンティック構造を実現する。

**背景**: 現行の DOM インジェクション方式（`data-wb-ref` 属性書き込み）は
- ページ DOM を汚染する
- クロスオリジン iframe に対応できない
- AX 意味論（role/name/description）ではなく DOM 構造ベース

agent-browser の実装 (`Accessibility.getFullAXTree` + `Target.setAutoAttach`) を参考に
WebView Bridge の既存 CDP インフラ（`call_cdp_sync`）の上に構築する。

---

## Phase 1: CDP Accessibility インフラ

| Task | 内容 | DoD | Depends | Status |
|------|------|-----|---------|--------|
| 1.1 | AXNode 型定義を追加（`src/webview/ax_types.rs` 新規作成） | `AXNode`, `AXProperty`, `AXTree` 型がコンパイルエラーなし | - | cc:完了 |
| 1.2 | `get_ax_tree()` 実装（`Accessibility.enable` + `Accessibility.getFullAXTree`） | `call_cdp_sync` で AX ツリーの JSON が取得でき、AXNode にパースできる | 1.1 | cc:完了 |
| 1.3 | AX ノードの ref 割り当てロジック実装 | interactive_roles（button/link/textbox 等）に e1,e2... が付与される | 1.2 | cc:完了 |
| 1.4 | cursor-interactive 要素の補完（CSS cursor:pointer / onclick / tabindex を JS 検出） | AX ツリーに現れない要素も ref に含まれる | 1.3 | cc:完了 |

### interactive_roles 定義
```
button, link, textbox, checkbox, radio, combobox, listbox,
menuitem, menuitemcheckbox, menuitemradio, option, switch,
tab, treeitem, spinbutton, searchbox, slider
```

### AXNode 最小フィールド
```
nodeId, backendDOMNodeId, role, name, description,
value, level, checked, expanded, disabled, focused,
required, haspopup, childIds, frameId
```

---

## Phase 2: iframe（クロスオリジン）対応

| Task | 内容 | DoD | Depends | Status |
|------|------|-----|---------|--------|
| 2.1 | `Target.setAutoAttach` でクロスオリジン iframe を自動アタッチ | iframe の CDP セッションが取得できる | 1.2 | cc:完了 |
| 2.2 | frame ごとの AX Tree 取得と統合（frameId + session マッピング） | iframe 内の要素が親ページの ref と統合されてスナップショットに現れる | 2.1, 1.3 | cc:完了 |
| 2.3 | iframe の ref に frameId を埋め込み（ref 解決時に正しい CDP セッションへルーティング） | iframe 内の `@e5` を click したとき、正しい iframe の CDP セッションで操作できる | 2.2 | cc:完了 |

---

## Phase 3: API エンドポイント拡張

| Task | 内容 | DoD | Depends | Status |
|------|------|-----|---------|--------|
| 3.1 | `SnapshotRequest` に `format: SnapshotFormat` 追加（`"dom"` \| `"ax"`、デフォルト `"dom"`） | 既存の `format` なしリクエストが後方互換で動く | Phase 1 | cc:完了 |
| 3.2 | `POST /snapshot` に `format=ax` 分岐追加、`get_ax_tree()` を呼ぶ | `/snapshot` に `{"format":"ax"}` を送ると AX ツリーベースの elements が返る | 3.1, Phase 1 | cc:完了 |
| 3.3 | レスポンス構造を統一（`elements[]` の shape は dom/ax 共通、ax のみ追加フィールドを含む） | 既存クライアントが format=dom で壊れない | 3.2 | cc:完了 |
| 3.4 | `element_refs` ストアへの ax refs 登録（resolve_ref_selector が backendNodeId ベースで解決） | `@e3` が ax snapshot 後も click で使えるベースが整う | 3.2, Phase 2 | cc:完了 |

---

## Phase 4: MCP v3 統合

| Task | 内容 | DoD | Depends | Status |
|------|------|-----|---------|--------|
| 4.1 | MCP v3 `SnapshotRequest` 型に `format` フィールド追加 | コンパイルエラーなし | Phase 3 | cc:完了 |
| 4.2 | `handle_snapshot` で `format=ax` パスを追加 | MCP 経由で `{"tool":"snapshot","params":{"format":"ax"}}` が動く | 4.1 | cc:完了 |
| 4.3 | MCP ツール定義の description と enum 値を更新 | `tools/list` の `snapshot` ツールに `format` パラメータが現れる | 4.2 | cc:完了 |

---

## Phase 5: CLI 統合

| Task | 内容 | DoD | Depends | Status |
|------|------|-----|---------|--------|
| 5.1 | `wb snapshot` コマンドに `--format <dom\|ax>` フラグ追加 | `wb snapshot --format ax` がコンパイルエラーなし | Phase 3 | cc:完了 |
| 5.2 | CLI の AX Tree テキスト出力フォーマッター実装 | `wb snapshot --format ax` でターミナルに role/name/ref が整形されて表示される | 5.1 | cc:完了 |

### AX テキスト出力例（参考）
```
- button  @e1  "ログイン"
- textbox @e2  "メールアドレス"
- textbox @e3  "パスワード"  (required)
- link    @e4  "パスワードを忘れた方"
  [iframe: accounts.google.com]
- button  @e5  "Googleでログイン"
```

---

## 実装メモ

### call_cdp_sync の使い方（既存パターン）
```rust
// 既存例（Page.captureScreenshot）
let result = self.call_cdp_sync("Page.captureScreenshot", &params)?;

// AX Tree 取得（同じパターンで）
let _ = self.call_cdp_sync("Accessibility.enable", "{}")?;
let ax_json = self.call_cdp_sync("Accessibility.getFullAXTree", "{}")?;
```

### agent-browser 参考ファイル
- `cli/src/native/snapshot.rs` — AX Tree パース + ref 割り当て
- `cli/src/native/element.rs` — backendNodeId ベースの要素解決
- `cli/src/native/cdp/client.rs` — CDP セッション管理

### 後方互換戦略
- `format` 未指定 → 従来の DOM インジェクション（破壊的変更なし）
- `format=dom` → 明示的に従来方式
- `format=ax` → 新しい AX Tree 方式
