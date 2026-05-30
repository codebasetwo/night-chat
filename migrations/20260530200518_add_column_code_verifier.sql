-- Add migration script here
ALTER TABLE oauth_states ADD COLUMN code_verifier TEXT NOT NULL DEFAULT '';