CREATE TABLE IF NOT EXISTS challenge_instances (
    user_id       TEXT    NOT NULL,
    challenge_id  TEXT    NOT NULL,
    state         TEXT    NOT NULL,
    start_time    INTEGER NOT NULL,
    PRIMARY KEY (user_id, challenge_id)
);

CREATE TABLE IF NOT EXISTS instance_details (
    user_id       TEXT    NOT NULL,
    challenge_id  TEXT    NOT NULL,
    detail        TEXT    NOT NULL,
    FOREIGN KEY (user_id, challenge_id)
        REFERENCES challenge_instances (user_id, challenge_id)
);