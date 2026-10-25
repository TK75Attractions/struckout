-- Running games are held in memory and are never part of the persistent history.
DELETE FROM games WHERE status = 'running';
ALTER TABLE games
    DROP CHECK games_chk_1,
    DROP COLUMN status,
    MODIFY COLUMN score INT UNSIGNED NOT NULL;
