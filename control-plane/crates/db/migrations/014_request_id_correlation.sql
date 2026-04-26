-- P042 §9.3: request correlation. Inbound HTTP requests receive (or
-- generate) an `X-Request-ID` header that the daemon threads through
-- middleware → resolver/handler → command journal → response. A nullable
-- column keeps the tracker backwards-compatible with rows written before
-- this migration lands; new rows populate the column unconditionally.
ALTER TABLE command_journal ADD COLUMN request_id TEXT;
CREATE INDEX IF NOT EXISTS idx_command_journal_request_id
    ON command_journal(request_id);
