-- 0001_init: initial schema (SPEC §5).
-- All timestamps are UTC Unix seconds.

CREATE TABLE accounts (
  id           INTEGER PRIMARY KEY,
  kind         TEXT NOT NULL CHECK (kind IN ('local','freshrss','miniflux')),
  name         TEXT NOT NULL,
  server_url   TEXT,                  -- NULL for local
  username     TEXT,
  -- secret stored in OS keyring under key "omarss:account:<id>"
  last_sync_at INTEGER,
  sync_state   TEXT,                  -- JSON blob of provider cursors
  created_at   INTEGER NOT NULL
);

CREATE TABLE folders (
  id          INTEGER PRIMARY KEY,
  account_id  INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
  remote_id   TEXT,
  name        TEXT NOT NULL,
  sort_order  INTEGER NOT NULL DEFAULT 0,
  UNIQUE (account_id, name)
);

CREATE TABLE feeds (
  id              INTEGER PRIMARY KEY,
  account_id      INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
  folder_id       INTEGER REFERENCES folders(id) ON DELETE SET NULL,
  remote_id       TEXT,
  url             TEXT NOT NULL,       -- feed URL (after permanent redirects)
  site_url        TEXT,
  title           TEXT NOT NULL,
  custom_title    TEXT,
  description     TEXT,
  icon_path       TEXT,                -- cached favicon file
  etag            TEXT,
  last_modified   TEXT,
  last_fetched_at INTEGER,
  next_fetch_at   INTEGER,
  fetch_interval  INTEGER,             -- seconds; NULL = use global default
  error_count     INTEGER NOT NULL DEFAULT 0,
  last_error      TEXT,
  full_text_mode  INTEGER NOT NULL DEFAULT 0,  -- 0 off, 1 auto-extract on arrival
  notify          INTEGER NOT NULL DEFAULT 0,
  paused          INTEGER NOT NULL DEFAULT 0,
  sort_order      INTEGER NOT NULL DEFAULT 0,
  created_at      INTEGER NOT NULL,
  UNIQUE (account_id, url)
);

CREATE TABLE articles (
  id             INTEGER PRIMARY KEY,
  feed_id        INTEGER NOT NULL REFERENCES feeds(id) ON DELETE CASCADE,
  remote_id      TEXT,
  guid           TEXT NOT NULL,        -- dedupe key, see SPEC §7.4
  url            TEXT,
  title          TEXT NOT NULL,
  author         TEXT,
  summary_html   TEXT,                 -- sanitised
  content_html   TEXT,                 -- sanitised, from feed
  fulltext_html  TEXT,                 -- sanitised, from extractor
  fulltext_state INTEGER NOT NULL DEFAULT 0,  -- 0 none, 1 ok, 2 failed
  published_at   INTEGER,
  updated_at     INTEGER,
  fetched_at     INTEGER NOT NULL,
  is_read        INTEGER NOT NULL DEFAULT 0,
  is_starred     INTEGER NOT NULL DEFAULT 0,
  read_at        INTEGER,
  starred_at     INTEGER,
  hidden         INTEGER NOT NULL DEFAULT 0,  -- set by rules
  content_hash   TEXT,                 -- to detect updated articles
  UNIQUE (feed_id, guid)
);
CREATE INDEX idx_articles_feed_pub   ON articles(feed_id, published_at DESC);
CREATE INDEX idx_articles_unread     ON articles(is_read, published_at DESC) WHERE hidden = 0;
CREATE INDEX idx_articles_starred    ON articles(is_starred, starred_at DESC) WHERE is_starred = 1;

CREATE TABLE enclosures (
  id          INTEGER PRIMARY KEY,
  article_id  INTEGER NOT NULL REFERENCES articles(id) ON DELETE CASCADE,
  url         TEXT NOT NULL,
  mime_type   TEXT,
  length      INTEGER
);

CREATE TABLE tags (
  id    INTEGER PRIMARY KEY,
  name  TEXT NOT NULL UNIQUE,
  color TEXT
);
CREATE TABLE article_tags (
  article_id INTEGER NOT NULL REFERENCES articles(id) ON DELETE CASCADE,
  tag_id     INTEGER NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
  PRIMARY KEY (article_id, tag_id)
);

CREATE TABLE rules (
  id          INTEGER PRIMARY KEY,
  name        TEXT NOT NULL,
  enabled     INTEGER NOT NULL DEFAULT 1,
  scope       TEXT NOT NULL,           -- JSON: {"all":true} | {"feeds":[..]} | {"folders":[..]}
  conditions  TEXT NOT NULL,           -- JSON, see SPEC §10
  match_mode  TEXT NOT NULL CHECK (match_mode IN ('all','any')),
  actions     TEXT NOT NULL,           -- JSON, see SPEC §10
  sort_order  INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE pending_actions (         -- offline queue for remote accounts
  id          INTEGER PRIMARY KEY,
  account_id  INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
  kind        TEXT NOT NULL,           -- mark_read, mark_unread, star, unstar, subscribe, unsubscribe, move, rename
  payload     TEXT NOT NULL,           -- JSON
  created_at  INTEGER NOT NULL,
  attempts    INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);  -- JSON values

CREATE VIRTUAL TABLE articles_fts USING fts5(
  title, author, body,                 -- body = text of content/fulltext, HTML stripped
  content='', tokenize='unicode61 remove_diacritics 2'
);
-- articles_fts is kept in sync from Rust on insert/update/delete (rowid = articles.id).

-- There is always exactly one Local account (SPEC §9.1).
INSERT INTO accounts (kind, name, created_at)
VALUES ('local', 'Local', CAST(strftime('%s', 'now') AS INTEGER));
