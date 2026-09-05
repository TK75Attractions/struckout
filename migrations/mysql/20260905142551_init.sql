-- Add migration script here
CREATE TABLE players (
    id INT UNSIGNED PRIMARY KEY,
    name TEXT NOT NULL
);

CREATE TABLE games (
    game_id INT UNSIGNED PRIMARY KEY,
    machine_id INT UNSIGNED NOT NULL,
    player_id INT UNSIGNED NOT NULL,
    started_at DATETIME NOT NULL,
    difficulty TEXT NOT NULL,
    score INT UNSIGNED NOT NULL,

    FOREIGN KEY (player_id) REFERENCES players (id)
);
