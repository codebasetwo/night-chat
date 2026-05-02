-- Create messages table
CREATE TABLE IF NOT EXISTS messages (
    id UUID NOT NULL PRIMARY KEY,
    sender_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    receiver_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    text TEXT,
    image TEXT,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL,
    CHECK (text IS NOT NULL OR image IS NOT NULL),
    CHECK (sender_id != receiver_id)
);

-- Create index for faster queries
CREATE INDEX IF NOT EXISTS idx_messages_sender_receiver ON messages(sender_id, receiver_id);
CREATE INDEX IF NOT EXISTS idx_messages_receiver_sender ON messages(receiver_id, sender_id);
CREATE INDEX IF NOT EXISTS idx_messages_created_at ON messages(created_at DESC);
