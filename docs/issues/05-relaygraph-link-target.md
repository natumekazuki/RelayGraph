# RelayGraph: link target を `id` 正本に寄せ、`pathHint` を派生同期できるようにする

## 背景

現在の sidecar link は `to` に単一の locator string を持つ形式で、`id:...` または `path:...` のどちらか一方だけを指定できる。  
当初は 1 つの link で `id` と `path` の両方を持つ案も候補だったが、実運用を考えると、安定参照の正本は `id` に寄せた方が扱いやすい。

## 課題

- `path` を手書きで持たせると rename / move 時に drift しやすい
- `validate` に自動書き戻しを持たせると、確認系コマンドなのに worktree を汚す意外性が出る
- `id` を正本にしないまま `path` を併記すると、どちらを canonical とみなすかが曖昧になる
- `id` 必須に寄せるなら、どの resource まで sidecar / id を要求するかの境界も整理が必要

## 期待結果

link の canonical target は `id` に統一する。  
可読性や確認補助に使う `path` 相当の情報は `pathHint` のような派生値として扱い、必要なら専用の sync / fix 操作で再生成できる状態にする。

## 対応範囲

- sidecar schema の拡張
- parser / model / graph build の拡張
- `validate`, `trace`, `export`, `cache` への反映
- `sync` / `fix` 相当コマンド、または `validate --fix` の検討
- 既存 `to` 形式との後方互換の維持
- documentation と sample の更新

## 論点整理

| 論点 | 現状 | 共通化候補 | 対応判断 |
| --- | --- | --- | --- |
| canonical な参照 | `path` と `id` のどちらを正にするかが link ごとに揺れる | `id` を canonical に統一する | この issue で対応 |
| path の位置づけ | `path` を link target 本体として使うことがある | `pathHint` などの派生・補助情報に下げる | この issue で対応 |
| link target の表現 | `to` は単一 string | `to` は `id:` 限定に寄せ、必要なら補助フィールドを追加する | この issue で対応 |
| backward compatibility | 既存 sidecar は `to` 前提 | 既存 `to: path:...` は段階的に非推奨化しつつ、互換期間は残す | この issue で対応 |
| 書き戻しの責務 | `validate` は read-only と期待されやすい | 書き戻しは `sync` / `fix` に分離し、必要なら `validate --fix` は alias として提供する | 方針をこの issue で決める |
| 既存 sidecar の一括移行 | 自動 migrate が無いと統一されない可能性がある | 新旧両対応を入れたうえで、既存 file の全面 migrate は別論点に分離する | 今回は全面移行しない |
| dual target 案 | `id` と `path` を同格に持てるが意味が散りやすい | 参考案として残しつつ、第一候補にはしない | 今回は採用しない |

## 想定インターフェース

### 現行

```yaml
links:
  - rel: realized-by
    to: id:docs.design.graph
```

### 拡張後の第一候補

```yaml
links:
  - rel: realized-by
    to: id:docs.design.graph
    pathHint: docs/design/graph.md
```

### 補足

- `to` は canonical な `id:` locator として扱う
- `pathHint` は人間向けの補助情報で、手書き必須にはしない
- `pathHint` は `sync` / `fix` 系で再生成できる前提にする

## validate / sync で確認したいこと

1. `to` が `id:` locator として解決できること
2. `to` が重複 id に解決される場合は曖昧として扱うこと
3. `pathHint` がある場合は、解決された resource path と一致していること
4. `validate` は原則 read-only とし、不一致は診断だけ返すこと
5. `sync` / `fix` 実行時は、`id` から `pathHint` を再生成または更新できること
6. 既存 `to` 形式も互換期間中は従来どおり validate / trace / export / cache で扱えること

## 検討事項

1. 書き戻しは独立した `sync` / `fix` コマンドにするか
2. `validate --fix` を alias として許可するか
3. `pathHint` を sidecar に保存するか、export / trace の表示だけで十分か
4. `id` 必須の対象を全 resource に広げるか、link される resource に絞るか

## 完了条件

1. link target の canonical 参照が `id` に統一される
2. schema が `pathHint` 相当の補助フィールドを受け付けられる
3. graph build が `id` 解決と `pathHint` 整合確認を扱える
4. `validate` が read-only のまま `pathHint` 不一致を診断できる
5. `sync` / `fix` 系、または同等の書き戻し手段が用意される
6. `trace` / `export` / `cache` が id-first 形式で動作する
7. 旧形式の互換テストと、新形式の正常系 / 異常系テストが追加されている
8. docs / sample / skill reference の少なくとも 1 箇所に新形式の例が追加されている

## 非対象

- resource `id` の命名規約そのもの
- 既存 sidecar の一括変換ツール
- `symbol:` や別 locator 種別の追加
- plugin vocabulary の設計変更
