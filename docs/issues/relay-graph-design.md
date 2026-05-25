---
title: RelayGraph: Git 管理下でのリソーストレーサビリティ設計
status: active
type: issue
---

# RelayGraph: Git 管理下でのリソーストレーサビリティ設計

## 概要

Git で管理されるリポジトリ内の各種リソースについて、ファイル形式に依存せず、機械的に関連資源を紐づけ、任意の 1 リソースから決定論的に関連先を辿れる仕組みを定義したい。

本 issue では、永続ストアとしてグラフ DB を直接持ち込むのではなく、Git に保存可能な宣言データと、それを完全探索して整合性を検査・生成する専用スクリプトを組み合わせる方式を対象とする。

アプリ名は **RelayGraph** として確定する。

アプリ本体の主目的は、Git 上の宣言データから **汎用 resource graph** を構築・検証・探索できるようにすることである。

`feature-trace` は、その上に載る標準同梱 YAML plugin の代表的ユースケースとして扱う。

## 背景

- 対象は Markdown 文書に限らず、`docs`、`cs`、`png` など任意のファイル形式を含む
- 関連リソースは人手でも読める形で管理したい
- 一方で、探索や整合性確認は機械的に実行したい
- Git 管理を前提とするため、常設 DB を正とする構成は避けたい
- 変更したい機能が決まったとき、Root の機能ドキュメントから最終的な対象コードとテストまで辿りたい
- ただし用途固有の語彙や必須ルールは Core に埋め込まず、plugin で追加したい

## 目標

- リソース種別に依存せず、共通ルールで関連を表現できる
- 汎用 resource graph として resource / relation / locator を構築・検証・探索できる
- 任意の 1 リソースから関連先を決定論的に辿れる
- Root から専用スクリプトを 1 回実行すれば、完全探索・整合性検証・生成ができる
- rename / move に比較的強い識別方式を持てる
- YAML plugin に応じて relation 語彙と必須条件を切り替えられる
- 標準同梱の `feature-trace` plugin により、Root の機能ドキュメントから設計・モジュール・コード・テストまで必要経路を追跡できる

## 非目標

- 常設のグラフ DB 導入
- 可視化 UI の実装
- 言語ごとのシンボル解析詳細の先行設計
- 初期段階での任意コード実行 plugin 導入
- 全リソース間の完全精密マッピング

## 検討結果サマリ

- 管理対象の主語は「ドキュメント」ではなく **resource** とする
- Git に保存するのは DB ではなく **グラフを再構築できる宣言データ** とする
- 各ファイルには `*.relaygraph.yaml` の **sidecar** を置ける設計にする
- ただし sidecar は **全ファイル必須にはしない**
- sidecar がないファイルも implicit resource として探索対象に含める
- Root 実行の専用スクリプトが収集・解決・検証・生成を担当する
- アプリ本体の主目的は **汎用 resource graph の構築・検証・探索** とする
- `feature-trace` は標準同梱の **YAML plugin** として提供する
- relation 語彙と必須条件は relation type 単体ではなく、用途別の **plugin** で束ねて定義する
- Core は **Rust** 製 CLI とする
- plugin は **YAML** による宣言型定義とする
- DB を用いる場合も、正は Git 上の宣言データとし、DB は Core に組み込んだローカル生成キャッシュとする

## 命名確定事項

- アプリ名: `RelayGraph`
- CLI 名: `relaygraph`
- Root 設定ファイル: `.relaygraph.yaml`
- sidecar suffix: `.relaygraph.yaml`
- 標準 plugin 例: `relaygraph/plugins/feature-trace.yaml`
- 生成 / cache ディレクトリ: `._relaygraph/`
- ローカル DB 例: `._relaygraph/cache/relaygraph.sqlite`

## 比較表

| 論点 | 選択肢 A | 選択肢 B | 差異 / 共通化候補 | 対応判断 |
| --- | --- | --- | --- | --- |
| 永続化方式 | グラフ DB を正とする | Git 上の宣言データを正とする | Git 運用との整合を優先するなら後者が適する | **B 採用** |
| 管理対象の単位 | ドキュメント中心 | resource 中心 | 形式非依存にするには resource へ一般化する | **B 採用** |
| 命名 | graph 系の汎用名称を使う | `RelayGraph` を採用する | graph の分かりやすさを残しつつ固有名を持てる | **B 採用** |
| 主目的 | feature traceability を主目的にする | 汎用 resource graph を主目的にする | 具体用途は plugin 側で表現する方が Core を汎化できる | **B 採用** |
| 標準ユースケース | plugin ごとに個別導入 | `feature-trace` を標準同梱する | 実価値の高い導線を初期状態で提供できる | **B 採用** |
| 関連情報の置き方 | 単一 index に集約 | ファイル近傍の sidecar | 単一 index は競合しやすく、sidecar は局所性が高い | **B 採用** |
| sidecar の必須範囲 | 全ファイル必須 | 必要なものだけ作成 | 前者は空定義が増え、後者は規模増に強い | **B 採用** |
| ノード定義 | 全 resource を明示定義 | resource は implicit、必要時のみ明示 | path / MIME / 存在確認は探索で共通化できる | **B 採用** |
| 識別子 | path を ID にする | stable ID と locator を分離する | rename 耐性と識別の責務分離が必要 | **B 採用** |
| リンク管理 | 双方向を手書き | 片方向のみ手書きし逆方向は生成 | 二重管理を避けられる | **B 採用** |
| 探索対象 | 独自 walk のみ | Git 管理対象 + 除外設定 | `.gitignore` を既定利用すると運用負荷が低い | **B 採用** |
| 拡張単位 | relation type 単体 | 用途別 plugin | 必須 relation や traversal を束ねて定義できる | **B 採用** |
| plugin 形式 | 任意コード実行 | YAML 宣言型 | 再現性・安全性・CI 適性を優先する | **B 採用** |
| Core 実装言語 | Go / C# など他の CLI 言語 | Rust | 速度最優先の CLI と単一バイナリ配布に向く | **B 採用** |
| DB 形態 | 別枠 DB / サービス | Core 同梱の組み込み DB | zero-setup と再構築容易性を両立しやすい | **B 採用** |

## 提案する構成

### 1. resource の扱い

- リポジトリ内の各ファイルは、拡張子に関係なく resource とみなす
- sidecar が存在しない場合でも implicit resource として扱う
- rename / move に耐えたい resource のみ stable ID を持つ

### 2. sidecar 規則

- 命名規則: `Foo.cs -> Foo.cs.relaygraph.yaml`
- sidecar は対象ファイルの近傍に置く
- sidecar には次だけを書く
  - stable ID
  - 自動推論できない metadata
  - outgoing links

例:

```yaml
# src/Auth/LoginService.cs.relaygraph.yaml
id: src.auth.login-service
links:
  - rel: explained-by
    to: id:doc.auth.overview
    order: 10
  - rel: illustrated-by
    to: path:assets/login-flow.png
    order: 20
```

### 3. リンクの決定論

- リンクは片方向のみを正とする
- 逆リンクは生成物で補完する
- 辿る順序は `order -> rel -> target` で固定する

### 4. 汎用 Core と標準 plugin

Core は **汎用 resource graph** を扱う。

- Core 自体は `feature-root` や `verified-by` のような用途固有語彙を持たない
- Core は resource / relation / locator / traversal / rule evaluator の共通基盤だけを提供する
- 用途固有の語彙、必須 relation、traversal preset は plugin 側で定義する

標準同梱 plugin として **`feature-trace`** を提供する。想定する最小導線は次の通り。

1. Root の機能ドキュメント
2. 設計 / 判断ドキュメント
3. 実装モジュール / 責務単位
4. コード / テスト / 図

- 全リソース間の完全グラフより、機能変更時に必ず見るべき経路の保証を優先する
- `feature-root` には少なくとも設計または実装への到達経路を要求する
- `module` には少なくともコードまたはテストへの到達経路を要求する
- 上記の必須条件は `feature-trace` plugin 側で定義し、Core は共通の評価器だけを持つ

### 5. スクリプト実行モデル

Root から専用スクリプトを実行し、以下をまとめて行う。

1. リポジトリ内 resource の完全探索
2. sidecar の収集
3. ID / path / relation の解決
4. 整合性検証
5. 検索や可視化向け生成物の出力

探索母集合は filesystem walk より、以下相当の Git 観点の列挙を正とする案が有力。

```text
git ls-files --cached --others --exclude-standard
```

これにより `.gitignore` を既定で尊重できる。

### 6. 設定ファイル

例:

```yaml
# .relaygraph.yaml
useGitIgnore: true
sidecarSuffix: ".relaygraph.yaml"
plugins:
  - "relaygraph/plugins/feature-trace.yaml"
exclude:
  - "._relaygraph/**"
  - "**/bin/**"
  - "**/obj/**"
requireSidecar:
  - "docs/**"
  - "src/**"
  - "assets/architecture/**"
```

- `exclude` は探索対象から除外する
- `requireSidecar` は sidecar 未作成をエラーにする対象
- `plugins` は読み込む YAML plugin の一覧
- sidecar が必須でない resource は implicit resource として扱う

### 7. Core と plugin の責務分離

Core は基本仕組みだけを提供し、初期実装言語は **Rust** とする。

- scan / ignore / sidecar 読み込み
- graph build
- 決定論的 traversal
- 共通 schema
- 汎用ルール評価
- export
- plugin 読み込み

plugin は **YAML** で外部定義し、用途別のトレーサビリティ契約を提供する。

- resource kind / role
- relation 語彙
- 必須・推奨ルール
- traversal preset
- エラー分類

初期段階では任意コードを実行する plugin は持たず、宣言型 YAML のみをサポートする。

`feature-trace` は標準同梱 plugin とし、それ以外は repo ごとに追加できるようにする。

例:

```yaml
# relaygraph/plugins/feature-trace.yaml
name: feature-trace
resourceKinds:
  - feature-root
  - design-doc
  - module
  - source
  - test
relations:
  - decomposes-to
  - realized-by
  - verified-by
rules:
  - when: feature-root
    requireAnyOutgoing:
      - decomposes-to
      - realized-by
  - when: module
    requireAnyOutgoing:
      - realized-by
      - verified-by
traversal:
  startKinds:
    - feature-root
  relationOrder:
    - decomposes-to
    - realized-by
    - verified-by
```

### 8. DB 方針

- 正は常に Git 上の sidecar と YAML plugin とする
- DB は別枠サービスにせず、Core に組み込んで配布するローカル DB とする
- DB の役割は query / cache / export の高速化であり、source of truth にはしない
- DB ファイルは `._relaygraph/cache/relaygraph.sqlite` などの生成領域に置き、Git 管理対象にしない
- DB が欠落または破損しても、Root 実行で sidecar から再構築できるようにする

### 9. 想定 subcommand

- `relaygraph validate`
  - 未作成 sidecar、壊れた relation、schema 不整合を検出する
- `relaygraph init`
  - 必須対象の未作成 sidecar を生成する
- `relaygraph export`
  - `._relaygraph/generated/relaygraph.json` などの生成物を出力する

## 検出したいエラー

- `missing-sidecar`
- `orphan-sidecar`
- `duplicate-id`
- `unresolved-id`
- `missing-path`
- `unknown-kind`
- `unknown-relation`
- `missing-required-relation`
- `plugin-load-error`
- `schema-error`

## 受け入れ条件

- 任意のファイル形式の resource を同一ルールで探索できる
- Core が用途固有語彙を持たない汎用 resource graph として動作する
- sidecar がない resource も探索結果に含まれる
- `requireSidecar` 対象の未作成ファイルを検出できる
- sidecar の relation 破損を Root 実行の検証で一括検出できる
- 標準同梱の `feature-trace` plugin により、Root の機能ドキュメントから設計・モジュール・コード・テストまで規則に従って同じ順序で関連先を辿れる
- YAML plugin により relation 語彙と必須条件を切り替えられる
- plugin で要求された relation 欠落を検出できる
- Core CLI が Rust 実装であり、YAML plugin の追加だけで用途別拡張ができる
- 組み込み DB を使う場合も Git 上の宣言データから再構築できる
- 生成物なしでも Git 上の宣言データだけで関係を再構築できる

## 追加検討事項

- `feature-trace` 以外に標準同梱する plugin をどう切るか
- `to` の参照形式を `id:` と `path:` のみに絞るか
- ファイル単位に加えて、symbol / page / region locator をいつ導入するか
- 生成物の出力先を `._relaygraph/` とするか別ディレクトリにするか
- sidecar と YAML plugin の schema version をどう管理するか
