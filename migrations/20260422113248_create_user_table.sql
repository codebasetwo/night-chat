-- Add migration script here
-- Create users Table
CREATE TABLE users(
   id uuid NOT NULL,
   PRIMARY KEY (id),
   email CITEXT NOT NULL UNIQUE,
   first_name TEXT NOT NULL,
   last_name TEXT NOT NULL,
   password_hash BYTEA NOT NULL,
   created_at timestamp(0) with time zone NOT NULL DEFAULT NOW()
);