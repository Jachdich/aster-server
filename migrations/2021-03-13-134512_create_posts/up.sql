-- Your SQL goes here
CREATE TABLE channels (
    uuid integer PRIMARY KEY NOT NULL,
    name text NOT NULL
);

CREATE TABLE messages (
    uuid integer PRIMARY KEY NOT NULL,
    content text NOT NULL,
    author_uuid integer NOT NULL,
    channel_uuid integer NOT NULL,
    date integer NOT NULL,
    FOREIGN KEY (author_uuid) REFERENCES users(uuid),
    FOREIGN KEY (channel_uuid) REFERENCES channels(uuid)
);

CREATE TABLE users (
    uuid integer PRIMARY KEY NOT NULL,
    name text NOT NULL,
    pfp text NOT NULL,
    group_uuid integer NOT NULL,
    FOREIGN KEY (group_uuid) REFERENCES groups(uuid)
);

CREATE TABLE groups (
    uuid integer PRIMARY KEY NOT NULL,
    permissions integer NOT NULL,
    name text NOT NULL,
    colour integer NOT NULL
);
