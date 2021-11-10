CREATE TABLE caches (
    kind TEXT,
    key BLOB,
    value BLOB,
    start_time TEXT,
    end_time TEXT,
    PRIMARY KEY (kind, key, start_time)
);

CREATE TABLE game_debug (
    game_id BLOB,
    error TEXT,
    log_json_zst BLOB,
    PRIMARY KEY (game_id)
);

CREATE TABLE game_stats (
    game_id BLOB,
    sim TEXT,
    season INTEGER,
    day INTEGER,
    away BLOB,
    home BLOB,
    stats_json_zst BLOB,
    PRIMARY KEY (game_id)
);

CREATE TABLE player_stats (
    game_id BLOB,
    team_id BLOB,
    player_id BLOB,
    sim TEXT,
    season INTEGER,
    day INTEGER,
    stats_json TEXT,
    PRIMARY KEY (game_id, team_id, player_id)
);
