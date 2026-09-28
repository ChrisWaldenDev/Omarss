-- 0003_polish: columns for M3 (daily-driver polish).

-- Per-feed User-Agent override for sites that block unknown agents (SPEC §7.1). NULL = default.
ALTER TABLE feeds ADD COLUMN user_agent TEXT;

-- First image of an article (Media RSS thumbnail or first <img>), for list thumbnails (SPEC §6.2, §7.3).
ALTER TABLE articles ADD COLUMN thumbnail_url TEXT;
