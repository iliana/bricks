CREATE TABLE caches (
    kind TEXT,
    key BLOB,
    value BLOB,
    start_time TEXT,
    end_time TEXT,
    PRIMARY KEY (kind, key, start_time)
);
