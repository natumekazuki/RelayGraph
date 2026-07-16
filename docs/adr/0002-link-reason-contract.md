# ADR 0002: link reason contract

- Status: Accepted
- Date: 2026-07-16
- Supersedes: ADR 0001 statements that limit acknowledgement to sidecar version 2

## Context

relation 名だけでは、個別の link が存在する理由を CLI、export、cache から取得できない。理由を用途別 relation 名へ埋め込むと plugin vocabulary が増え、同じ relation の traversal と validation が分断される。

RelayGraph の sidecar reader は未知フィールドを拒否する。既存の versioning 方針では、optional field の追加でも旧 reader が安全に解釈できない場合は sidecar schema version を上げる必要がある。SQLite cache は Git-backed declaration から再構築でき、既存 cache の in-place migration は前提としていない。

## Decision

- 個別 link の理由は optional string field `reason` として表現する。空文字と空白のみの値は拒否する。
- `reason` は sidecar schema version 3 で導入する。version 3 は version 2 の relation acknowledgement も扱える。
- `link add --reason` と `link update --reason` は、必要な場合だけ編集対象 sidecar を version 3 へ上げる。field を削除しても schema version は下げない。
- version 2 から version 3 へ上げる場合は、編集対象外の acknowledgement に `linkRevision` を追加し、sidecar 全体を一度に移行する。endpoint fingerprint は保持する。reason の変更対象となる link は、移行せず acknowledgement を解除する。
- CLI が書く `reason` は常に double-quoted scalar とし、改行を含む値も escape して一行で保存する。既存の block scalar を更新または削除する場合は、継続行を含む field 全体を置換する。
- reason の実値を変更または削除した場合は、その link の acknowledgement を解除する。同じ値を再設定した場合は解除しない。
- version 3 の acknowledgement は endpoint fingerprint に加えて、`rel`、`to`、`reason` の組から算出した `linkRevision` を必須とする。version 2 の acknowledgement は従来どおり endpoint fingerprint のみとする。
- `linkRevision` は domain separator と length-prefixed UTF-8 component を連結した値の SHA-256 とする。手編集を含め、acknowledge 後に `rel`、`to`、`reason` が変わった場合は relation review を要求する。
- export、trace JSON、cache links JSON、cache trace JSON は `reason` key を常に返し、値がない場合は `null` とする。通常の trace 表示は reason がある relation にだけ値を併記する。
- SQLite cache schema は version 2 とし、links table に nullable `reason` column を追加する。旧 cache は migration せず、rebuild 案内付きで拒否する。

## Alternatives

### `purpose` または `description` を使う

`purpose` は将来目的を表す意味に寄り、`description` は link 自体の説明と依存理由を区別しにくいため採用しない。

### metadata または用途別 relation で表現する

edge 固有の値を resource metadata へ置くと対応関係が失われる。用途別 relation は plugin vocabulary と traversal rule を増やすため採用しない。

### reason がない JSON key を省略する

既存の export と cache projection は optional value を required nullable key として公開している。同じ契約を維持するため省略しない。

### 旧 cache を migration する

cache は再構築可能な projection であり、migration の failure state と保守コストを増やす利点がないため採用しない。

### reason 編集後も acknowledgement を保持する

依存理由は relation の意味に含まれる。変更前の意味に対する review 状態を残すため採用しない。

### acknowledgement の解除を CLI 編集だけに任せる

sidecar は手編集可能な正本であり、CLI を経由しない変更を検出できない。review 状態を編集経路に依存させないため採用しない。

### version 3 への昇格時に全 acknowledgement を解除する

編集対象外の link は、review 済みの endpoint と link identity が変わっていない。既存の review 状態を失う必要がないため、`linkRevision` を追加して移行する。

## Consequences

- reason を書く sidecar は version 3 を明示する必要がある。
- reason の追加による version 3 への昇格は、同じ sidecar の acknowledgement を不正な中間状態にせず、単一の書き込みで完了する。
- `link acknowledge` は reason のない version 1 sidecar を version 2 へ上げ、既存の version 3 sidecar を version 2 へ下げず `linkRevision` を記録する。
- version 3 の既存 acknowledgement に `linkRevision` がない場合は schema error となる。`link acknowledge` は、対象 sidecar で選択した link だけに shape error がある場合に限り修復を許可する。内容を確認して再度 acknowledge すれば復旧できる。不正な hash、兄弟 link の shape error、その他の schema error は修復入口でも拒否する。
- cache schema version 1 を読む command は失敗し、`relaygraph cache rebuild` を案内する。
- sidecar schema、export schema、CLI integration test、cache schema test が executable contract となる。
