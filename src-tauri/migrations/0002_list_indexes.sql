-- 0002_list_indexes: keep the article list and unread counts fast at 200k articles (SPEC §12).

-- "All" and "Today" views, newest or oldest first, with keyset pagination on (published_at, id).
CREATE INDEX idx_articles_published ON articles(published_at DESC, id DESC) WHERE hidden = 0;

-- Per-feed unread counts for the sidebar.
CREATE INDEX idx_articles_feed_unread ON articles(feed_id) WHERE is_read = 0 AND hidden = 0;
