# RelayGraph Skill: plugin yaml の reference と custom plugin 判断基準を追加する

## 背景

Skill には `feature-trace` の利用前提はあるが、plugin yaml 自体の shape や、default をそのまま使う場合と repo-local plugin を置く場合の境界が見えにくい。  
そのため、resource kind や relation を少し変えたいだけでも、どこまでが標準運用でどこからが custom plugin なのか判断しづらい。

## 課題

- plugin yaml の最低限の書き方が Skill 内に無い
- `feature-trace` を repo-local に持つ例が無い
- custom plugin を作る条件が曖昧

## 期待結果

Skill 利用者が、plugin の追加や repo-local 化を「必要な時だけ、根拠を持って」選べる状態にする。

## 対応範囲

- `references/plugin-v1.md` を追加する
- minimal plugin sample を載せる
- `feature-trace` 利用、repo-local copy、custom plugin の判断基準を整理する

## 論点整理

| 論点 | 現状 | 共通化候補 | 対応判断 |
| --- | --- | --- | --- |
| plugin yaml の shape | Skill だけでは具体形が分からない | plugin v1 reference を追加する | この issue で対応 |
| `feature-trace` の扱い | 使う前提はあるが持ち方の例が弱い | repo-local plugin の sample を載せる | この issue で対応 |
| custom plugin の境界 | default rule の上書きか独自 vocabulary か判断しにくい | 判断基準を表で整理する | この issue で対応 |
| graph の初期粒度 | plugin を理解しても設計粒度は別問題 | sample pattern issue に分離する | 別 issue |

## 完了条件

1. `references/plugin-v1.md` が追加されている
2. `resourceKinds` と `relations` を含む minimal sample がある
3. `feature-trace` をそのまま使う場合、repo-local に置く場合、custom plugin を作る場合の比較表がある
4. custom plugin を作るべき条件が、少なくとも 3 パターン以上で整理されている
5. Skill 本文から plugin reference へ辿れる

## 非対象

- 具体的な業務 vocabulary の設計
- repo 固有の resource id 命名規則
- sidecar 記法そのものの詳細説明
