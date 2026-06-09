-- Add migration script here
CREATE TABLE players (
    id INTEGAR PRIMARY KEY,
    name TEXT   
);

CREATE TABLE scores (
    started_at TEXT NOT NULL,
    player_id INTEGAR NOT NULL,
    difficulity TEXT NOT NULL,
    score INTEGAR NOT NULL,
    FOREIGN KEY (player_id) REFERENCES players(id)
);
