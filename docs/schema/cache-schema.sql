PRAGMA foreign_keys = ON;
PRAGMA user_version = 1;

CREATE TABLE metadata (
    key TEXT PRIMARY KEY NOT NULL,
    value TEXT NOT NULL
);

-- Required row:
-- key = 'cacheSchemaVersion'
-- value = '1'

CREATE TABLE plugins (
    name TEXT PRIMARY KEY NOT NULL,
    traversal_json TEXT
);

CREATE TABLE resources (
    path TEXT PRIMARY KEY NOT NULL,
    id TEXT,
    kind TEXT,
    sidecar TEXT,
    metadata_json TEXT NOT NULL
);

CREATE TABLE links (
    source_path TEXT NOT NULL,
    rel TEXT NOT NULL,
    target_locator TEXT NOT NULL,
    target_path TEXT,
    target_id TEXT,
    relation_rank INTEGER,
    link_order INTEGER,
    FOREIGN KEY(source_path) REFERENCES resources(path)
);

CREATE INDEX links_source_path_idx ON links(source_path);
CREATE INDEX links_target_path_idx ON links(target_path);
CREATE INDEX links_target_id_idx ON links(target_id);
CREATE INDEX resources_id_idx ON resources(id);
CREATE INDEX resources_kind_idx ON resources(kind);

CREATE TABLE diagnostics (
    code TEXT NOT NULL,
    path TEXT,
    message TEXT NOT NULL
);
