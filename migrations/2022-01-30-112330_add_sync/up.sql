-- Your SQL goes here
create table sync_data (
    user_uuid BigInt PRIMARY KEY NOT NULL,
    uname text NOT NULL,
    pfp text NOT NULL
);

create table sync_servers (
    user_uuid BigInt NOT NULL,
    server_uuid BigInt NOT NULL,
    ip Text NOT NULL,
    port Integer NOT NULL,
    pfp Text,
    name Text,
    idx Integer NOT NULL,
    rowid Integer NOT NULL PRIMARY KEY
);
