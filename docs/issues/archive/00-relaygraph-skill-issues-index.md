# RelayGraph Skill 改善 Issue 一覧

## 分割方針

- 現状の Skill は、`trace -> read -> edit -> validate` の基本フローを理解するには十分だった。
- 一方で、新規 repo への初回導入では、手順の流れだけでは足りず、設定 schema と concrete example の確証が必要だった。
- 1 件にまとめると「導入手順」「root config」「plugin」「初期 graph の粒度」が混ざるので、利用者が必要な論点だけ拾いにくい。
- そのため、workflow / root config / plugin / sample pattern の 4 件に分割する。

## Issue 一覧

| ファイル | 主題 | 分けた理由 | 推奨順 |
| --- | --- | --- | --- |
| `01-relaygraph-skill-bootstrap-guide.md` | 新規 repo 導入ガイド | 最初に迷うのは手順の入口なので独立させる | 1 |
| `02-relaygraph-skill-root-config-reference.md` | `.relaygraph.yaml` の reference | root config の shape が最も不足していたため独立させる | 2 |
| `03-relaygraph-skill-plugin-reference.md` | plugin yaml と custom plugin 判断基準 | vocabulary と repo rule の境界を分けて整理するため | 2 |
| `04-relaygraph-skill-sample-patterns.md` | 初期 graph 粒度と repo pattern 別サンプル | 構文理解後の運用判断を別論点として切り出すため | 3 |

## 推奨着手順

1. `01-relaygraph-skill-bootstrap-guide.md`
2. `02-relaygraph-skill-root-config-reference.md`
3. `03-relaygraph-skill-plugin-reference.md`
4. `04-relaygraph-skill-sample-patterns.md`

## 補足

- 各 issue は単体で読めるように、背景、課題、対応範囲、完了条件、非対象を含めている。
- どの issue も project 固有事情ではなく、RelayGraph Skill を一般利用できる形に寄せる前提で書いている。
