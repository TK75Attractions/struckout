CREATE TABLE players (
    id INT UNSIGNED PRIMARY KEY AUTO_INCREMENT,
    name VARCHAR(32) UNIQUE NOT NULL
);

CREATE TABLE games (
    game_id INT UNSIGNED PRIMARY KEY AUTO_INCREMENT,
    machine_id_1 INT UNSIGNED NOT NULL,
    player_id_1 INT UNSIGNED NOT NULL,
    machine_id_2 INT UNSIGNED,
    player_id_2 INT UNSIGNED,
    started_at DATETIME NOT NULL,
    difficulty TEXT NOT NULL,
    score INT UNSIGNED NOT NULL,
    mode ENUM('single', 'multi') NOT NULL,

    FOREIGN KEY (player_id_1) REFERENCES players (id),
    FOREIGN KEY (player_id_2) REFERENCES players (id),
    CHECK (
        (mode = 'single' AND machine_id_2 IS NULL AND player_id_2 IS NULL)
        OR
        (mode = 'multi' AND machine_id_2 IS NOT NULL AND player_id_2 IS NOT NULL)
    )
);
