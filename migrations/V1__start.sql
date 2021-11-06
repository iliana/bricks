CREATE TABLE caches (
    kind TEXT,
    key BLOB,
    value BLOB,
    start_time TEXT,
    end_time TEXT,
    PRIMARY KEY (kind, key, start_time)
);

CREATE TABLE stats (
    game_id TEXT,
    team_id TEXT,
    player_id TEXT,
    stats_json TEXT,
    PRIMARY KEY (game_id, team_id, player_id)
);
