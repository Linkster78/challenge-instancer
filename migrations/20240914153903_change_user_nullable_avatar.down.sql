CREATE TABLE IF NOT EXISTS users_new (
    id             TEXT    NOT NULL PRIMARY KEY,
    username       TEXT    NOT NULL,
    display_name   TEXT    NOT NULL,
    avatar         TEXT    NOT NULL,
    creation_time  INTEGER NOT NULL,
    instance_count INTEGER NOT NULL DEFAULT 0
);

INSERT INTO users_new SELECT * FROM users;

DROP TABLE users;

ALTER TABLE users_new RENAME TO users;