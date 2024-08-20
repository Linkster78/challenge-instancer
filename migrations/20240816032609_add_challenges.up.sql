CREATE TABLE IF NOT EXISTS challenge_instances (
    user_id       TEXT    NOT NULL,
    challenge_id  TEXT    NOT NULL,
    state         TEXT    NOT NULL,
    details       TEXT            ,
    stop_time     INTEGER         ,
    PRIMARY KEY (user_id, challenge_id)
);