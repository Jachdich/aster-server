-- Your SQL goes here

CREATE TABLE user_groups (
    link_id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_uuid BigInt NOT NULL,
    group_uuid BigInt NOT NULL,
    FOREIGN KEY (user_uuid) REFERENCES users(uuid),
    FOREIGN KEY (group_uuid) REFERENCES groups(uuid)
);
