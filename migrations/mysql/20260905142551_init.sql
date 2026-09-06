-- Add migration script here
CREATE TABLE players (
    id INT UNSIGNED PRIMARY KEY AUTO_INCREMENT,
    name TEXT NOT NULL
);

CREATE TABLE games (
    game_id INT UNSIGNED PRIMARY KEY AUTO_INCREMENT,
    machine_id INT UNSIGNED NOT NULL,
    player_id INT UNSIGNED NOT NULL,
    started_at DATETIME NOT NULL,
    difficulty TEXT NOT NULL,
    score INT UNSIGNED NULL,
    status ENUM('running','finished') NOT NULL,

    FOREIGN KEY (player_id) REFERENCES players (id),
    CHECK (
        (status = 'running' AND score IS NULL)
        OR
        (status = 'finished' AND score IS NOT NULL)
    )
);
