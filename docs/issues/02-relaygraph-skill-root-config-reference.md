# RelayGraph Skill: `.relaygraph.yaml` の reference を追加する

## 背景

初回導入で最も不足していたのは、sidecar ではなく root config の concrete example だった。  
`schemaVersion`, `plugins`, `exclude`, `requireSidecar` などの key が分かっても、どの組み合わせで書けるかを Skill だけでは確証しづらい。

## 課題

- sidecar v1 の例はあるが、root config の例が無い
- よく使う key の役割と相互関係が整理されていない
- 最小構成と少し育った構成の差分が見えない

## 期待結果

`.relaygraph.yaml` を、元ソースを見に行かずに Skill 内の reference だけで記述・レビューできる状態にする。

## 対応範囲

- `references/config-v1.md` を追加する
- root config の最小例と拡張例を載せる
- よく使う key の説明と注意点を整理する

## 論点整理

| 論点 | 現状 | 共通化候補 | 対応判断 |
| --- | --- | --- | --- |
| root config の shape | Skill からは全体像が読めない | config v1 reference を追加する | この issue で対応 |
| よく使う key | key 名は断片的にしか分からない | `schemaVersion`, `plugins`, `exclude`, `requireSidecar`, `useGitIgnore`, `sidecarSuffix` をまとめる | この issue で対応 |
| 最小例と拡張例 | どこまで書けば十分か判断しづらい | 2 段階の sample を置く | この issue で対応 |
| plugin yaml の shape | root config と論点が異なる | plugin reference に分離する | 別 issue |

## 完了条件

1. `references/config-v1.md` が追加されている
2. 最小構成の `.relaygraph.yaml` sample がある
3. `exclude` や `requireSidecar` を含む、実務寄りの拡張 sample がある
4. key ごとに「何のために使うか」と「よくある使いどころ」が書かれている
5. Skill 本文から config reference へ辿れる

## 非対象

- plugin yaml の schema と設計指針
- repo pattern 別の graph 設計例
- project 固有の naming rule
