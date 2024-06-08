CREATE TABLE IF NOT EXISTS users (
    id            TEXT    NOT NULL PRIMARY KEY,
    username      TEXT    NOT NULL,
    display_name  TEXT    NOT NULL,
    avatar        TEXT    NOT NULL,
    creation_time INTEGER NOT NULL
);