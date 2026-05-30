# RelayGraph Skill: 初期 graph 粒度と repo pattern 別サンプルを追加する

## 背景

構文と手順が分かっても、初回導入で次に迷うのは「どこまで張れば十分か」だった。  
README から設計書だけ繋ぐのか、`.csproj` を module にするのか、代表 source と test まで入れるのかといった判断は、syntax reference だけでは補いづらい。

## 課題

- 初期 graph の推奨粒度が示されていない
- repo の種類ごとに、最初の張り方の例が無い
- docs / module / source / test の結び方が利用者ごとにぶれやすい

## 期待結果

利用者が repo の構成に応じて、過剰にも不足にも寄らない初期 graph を組める状態にする。

## 対応範囲

- repo pattern 別サンプルを追加する
- phase 1 / phase 2 / phase 3 の段階的な広げ方を示す
- `feature-root -> design-doc -> module -> source/test` の代表的な結び方を整理する

## 論点整理

| 論点 | 現状 | 共通化候補 | 対応判断 |
| --- | --- | --- | --- |
| 初期 graph の最小単位 | syntax は分かっても粒度判断が難しい | phase 1 / 2 / 3 の推奨粒度を追加する | この issue で対応 |
| repo pattern 差分 | doc-heavy と app-heavy で考え方が違う | pattern 別 sample を置く | この issue で対応 |
| module/source/test のつなぎ方 | 人ごとに分解粒度がぶれやすい | 代表チェーンを例示する | この issue で対応 |
| root config / plugin schema | 粒度の議論とは別 | reference issue に分離する | 別 issue |

## 完了条件

1. 少なくとも 3 パターンの sample がある
   - doc-heavy repo
   - .NET solution repo
   - library + app + tests repo
2. phase 1 / phase 2 / phase 3 の広げ方がある
3. 各 pattern に対して、最初に置く sidecar の候補と理由が書かれている
4. `feature-root -> design-doc -> module -> source/test` のサンプルチェーンがある
5. Skill 本文から sample pattern へ辿れる

## 非対象

- 特定 repo 向けの canonical path 一覧
- 全言語の網羅的なサンプル
- cache/export の内部実装説明
