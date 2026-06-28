-- Add migration script here

CREATE TABLE frames (
    timestamp INTEGAR PRIMARY KEY,
    data BLOB NOT NULL
)