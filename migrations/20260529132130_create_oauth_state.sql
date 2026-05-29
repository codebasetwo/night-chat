-- Add migration script here
CREATE TABLE oauth_states (
    state      TEXT PRIMARY KEY,
    expires_at TIMESTAMPTZ NOT NULL
);