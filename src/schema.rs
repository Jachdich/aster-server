use diesel::{allow_tables_to_appear_in_same_query, joinable, table};

table! {
    channels (uuid) {
        uuid -> BigInt,
        name -> Text,
    }
}

table! {
    emojis (uuid) {
        uuid -> BigInt,
        name -> Text,
        data -> Text,
    }
}

table! {
    groups (uuid) {
        uuid -> BigInt,
        permissions -> BigInt,
        name -> Text,
        colour -> Integer,
    }
}

table! {
    messages (uuid) {
        uuid -> BigInt,
        content -> Text,
        author_uuid -> BigInt,
        channel_uuid -> BigInt,
        date -> Integer,
        rowid -> Integer,
    }
}

table! {
    sync_data (user_uuid) {
        user_uuid -> BigInt,
        uname -> Text,
        pfp -> Text,
    }
}

table! {
    sync_servers (rowid) {
        user_uuid -> BigInt,
        server_uuid -> BigInt,
        ip -> Text,
        port -> Integer,
        pfp -> Nullable<Text>,
        name -> Nullable<Text>,
        idx -> Integer,
        rowid -> Integer,
    }
}

table! {
    user_groups (link_id) {
        link_id -> Nullable<Integer>,
        user_uuid -> BigInt,
        group_uuid -> BigInt,
    }
}

table! {
    users (uuid) {
        uuid -> BigInt,
        name -> Text,
        pfp -> Text,
        group_uuid -> BigInt,
    }
}

joinable!(messages -> channels (channel_uuid));
joinable!(messages -> users (author_uuid));
joinable!(user_groups -> groups (group_uuid));
joinable!(user_groups -> users (user_uuid));
joinable!(users -> groups (group_uuid));

allow_tables_to_appear_in_same_query!(
    channels,
    emojis,
    groups,
    messages,
    sync_data,
    sync_servers,
    user_groups,
    users,
);
