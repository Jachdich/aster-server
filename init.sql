CREATE TABLE channels (
    uuid BigInt PRIMARY KEY NOT NULL,
    name text NOT NULL
);
CREATE TABLE messages (
    uuid BigInt PRIMARY KEY NOT NULL,
    content text NOT NULL,
    author_uuid BigInt NOT NULL,
    channel_uuid BigInt NOT NULL,
    date integer NOT NULL, edited Integer not null default 0,
    FOREIGN KEY (author_uuid) REFERENCES users(uuid),
    FOREIGN KEY (channel_uuid) REFERENCES channels(uuid)
);
CREATE TABLE users (
    uuid BigInt PRIMARY KEY NOT NULL,
    name text NOT NULL,
    pfp text NOT NULL,
    group_uuid BigInt NOT NULL,
    FOREIGN KEY (group_uuid) REFERENCES groups(uuid)
);
CREATE TABLE groups (
    uuid BigInt PRIMARY KEY NOT NULL,
    permissions BigInt NOT NULL,
    name text NOT NULL,
    colour integer NOT NULL
);
CREATE TABLE sqlite_sequence(name,seq);
CREATE TABLE user_groups (
    link_id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_uuid BigInt NOT NULL,
    group_uuid BigInt NOT NULL,
    FOREIGN KEY (user_uuid) REFERENCES users(uuid),
    FOREIGN KEY (group_uuid) REFERENCES groups(uuid)
);
CREATE TABLE sync_data (
    user_uuid BigInt PRIMARY KEY NOT NULL,
    uname text NOT NULL,
    pfp text NOT NULL
);
CREATE TABLE emojis (
    uuid BigInt PRIMARY KEY NOT NULL,
    name text NOT NULL,
    data text NOT NULL
);
CREATE TABLE sync_servers (
    user_uuid BigInt NOT NULL,
    uuid BigInt,
    uname Text NOT NULL,
    ip Text NOT NULL,
    port Integer NOT NULL,
    pfp Text,
    name Text,
    idx Integer NOT NULL,
    rowid Integer NOT NULL PRIMARY KEY
);
