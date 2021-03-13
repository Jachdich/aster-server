table! {
    channels (uuid) {
        uuid -> Integer,
        name -> Text,
    }
}

table! {
    groups (uuid) {
        uuid -> Integer,
        permissions -> Integer,
        name -> Text,
        colour -> Integer,
    }
}

table! {
    messages (uuid) {
        uuid -> Integer,
        content -> Text,
        author_uuid -> Integer,
        channel_uuid -> Integer,
        date -> Integer,
    }
}

table! {
    users (uuid) {
        uuid -> Integer,
        name -> Text,
        pfp -> Text,
        group_uuid -> Integer,
    }
}

joinable!(messages -> channels (channel_uuid));
joinable!(messages -> users (author_uuid));
joinable!(users -> groups (group_uuid));

allow_tables_to_appear_in_same_query!(
    channels,
    groups,
    messages,
    users,
);
