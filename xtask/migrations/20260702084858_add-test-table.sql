-- Add migration script here
CREATE TABLE timestamp_test(
    timestamp INTEGAR,
    session_id TEXT,
    PRIMARY KEY (timestamp, session_id)
)