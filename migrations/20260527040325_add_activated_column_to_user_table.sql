-- Add migration script here
ALTER TABLE users ADD COLUMN is_activated BOOLEAN DEFAULT FALSE;