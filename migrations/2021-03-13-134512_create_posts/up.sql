-- Your SQL goes here
CREATE TABLE channels (
    uuid BigInt PRIMARY KEY NOT NULL,
    name text NOT NULL
);

CREATE TABLE messages (
    uuid BigInt PRIMARY KEY NOT NULL,
    content text NOT NULL,
    author_uuid BigInt NOT NULL,
    channel_uuid BigInt NOT NULL,
    date integer NOT NULL,
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
