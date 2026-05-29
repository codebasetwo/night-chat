-- Add migration script here
CREATE TABLE oauth_accounts (
    id                UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id           UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    provider          TEXT NOT NULL,           -- 'google', 'github', etc.
    provider_user_id  TEXT NOT NULL,           -- Google's `sub` field
    provider_email    TEXT,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (provider, provider_user_id)        -- prevents duplicate links
);
