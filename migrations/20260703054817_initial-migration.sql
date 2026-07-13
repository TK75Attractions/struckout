-- Add migration script here
CREATE TABLE frames(
    timestamp INTEGAR NOT NULL,
    session_id TEXT NOT NULL,
    data BLOB NOT NULL,
    PRIMARY KEY (timestamp, session_id)
);

CREATE TABLE players (
    id INTEGAR PRIMARY KEY,
    name TEXT NOT NULL
);

CREATE TABLE scores (
    started_at TEXT NOT NULL,
    player_id INTEGAR NOT NULL,
    difficulity TEXT NOT NULL,
    score INTEGAR NOT NULL,
    FOREIGN KEY (player_id) REFERENCES players(id)
);