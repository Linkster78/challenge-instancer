CREATE TABLE IF NOT EXISTS challenge_instances (
    user_id       TEXT    NOT NULL,
    challenge_id  TEXT    NOT NULL,
    state         TEXT    NOT NULL,
    details       TEXT            ,
    start_time    INTEGER NOT NULL,
    PRIMARY KEY (user_id, challenge_id)
);