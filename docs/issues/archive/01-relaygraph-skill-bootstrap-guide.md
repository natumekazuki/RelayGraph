# RelayGraph Skill: 新規 repo 導入ガイドを追加する

## 背景

既存の Skill は、RelayGraph が既に導入されている repo で `trace` や `validate` を回すには十分だった。  
一方で、`.relaygraph.yaml` がまだ無い repo に対して初回導入する場面では、どの順で何を置けばよいかが Skill だけでは確定しにくい。

## 課題

- 新規 repo での開始点が明文化されていない
- `._relaygraph/` のような generated output の扱いが運用まで含めて説明されていない
- `validate` 実行後に、何をもって「使える graph」とみなすかが曖昧

## 期待結果

RelayGraph 未導入の repo でも、Skill だけで初回導入の流れを迷わず完了できる状態にする。

## 対応範囲

- `references/bootstrap-repo.md` を追加する
- Skill 本文から、新規 repo 導入時の参照先としてリンクする
- 最小導入の手順と完了判定を明文化する

## 論点整理

| 論点 | 現状 | 共通化候補 | 対応判断 |
| --- | --- | --- | --- |
| 新規 repo の開始条件 | `.relaygraph.yaml` がある前提で読める | 未導入 repo 向け bootstrap 手順を追加する | この issue で対応 |
| generated output の扱い | `._relaygraph/` は generated としか書かれていない | `.gitignore` と再生成前提を手順に含める | この issue で対応 |
| validate 後の成功判定 | 実行コマンドはあるが exit criteria が弱い | minimum usable graph の判定観点を追加する | この issue で対応 |
| root config の詳細 | 手順だけでは schema の確証が取れない | 詳細は config reference に分離する | 別 issue |

## 完了条件

1. `references/bootstrap-repo.md` が追加されている
2. ガイドに「検出 -> 初期設定 -> sidecar 最小追加 -> validate -> trace 確認」の順がある
3. `._relaygraph/` の扱いと `.gitignore` への反映方針が含まれている
4. `validate` 成功に加えて、最低 1 本の `trace` で期待した接続が見えることを完了条件に含めている
5. Skill 本文から、未導入 repo でこのガイドを参照する導線がある

## 非対象

- `.relaygraph.yaml` の key ごとの完全 reference
- plugin yaml の schema 詳細
- 言語別 / repo 種別ごとのサンプル集
