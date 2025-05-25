use std::marker::PhantomData;

use serde::de::{SeqAccess, Visitor};
// See aster updates discord, pinned messages
use serde::ser::SerializeSeq;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::helper::Uuid;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Perm {
    Allow,
    Deny,
    Default,
}

#[derive(Clone, PartialEq, Debug)]
pub struct Permissions {
    pub modify_channels: Perm,
    pub modify_icon_name: Perm,
    pub modify_groups: Perm,
    pub modify_user_groups: Perm,
    pub ban_users: Perm,
    pub send_messages: Perm,
    pub read_messages: Perm,
    pub manage_messages: Perm,
    pub join_voice: Perm,
}

#[derive(Clone, PartialEq, Eq, Debug, Hash, Serialize, Deserialize)]
pub enum PermableEntity {
    User(Uuid),
    Group(Uuid),
}

impl From<u8> for Perm {
    fn from(value: u8) -> Self {
        match value {
            0 => Perm::Default,
            1 => Perm::Allow,
            2 => Perm::Deny,
            _ => unreachable!(),
        }
    }
}

impl From<Perm> for u8 {
    fn from(value: Perm) -> Self {
        match value {
            Perm::Default => 0,
            Perm::Allow => 1,
            Perm::Deny => 2,
        }
    }
}

fn byte_to_perms(b: u8) -> [Perm; 4] {
    [
        ((b >> 0) & 0b11).into(),
        ((b >> 2) & 0b11).into(),
        ((b >> 4) & 0b11).into(),
        ((b >> 6) & 0b11).into(),
    ]
}

fn perms_to_byte(p: [Perm; 4]) -> u8 {
    (Into::<u8>::into(p[0]) << 0)
        | (Into::<u8>::into(p[1]) << 2)
        | (Into::<u8>::into(p[2]) << 4)
        | (Into::<u8>::into(p[3]) << 6)
}

impl From<&[u8]> for Permissions {
    fn from(value: &[u8]) -> Self {
        let perms: Vec<Perm> = value.iter().map(|x| byte_to_perms(*x)).flatten().collect();
        let get_perm = |idx| {
            if idx < perms.len() {
                perms[idx]
            } else {
                Perm::Default // any perms not specified are assumed to be "default"
            }
        };
        Permissions {
            modify_channels: get_perm(0),
            modify_icon_name: get_perm(1),
            modify_groups: get_perm(2),
            modify_user_groups: get_perm(3),
            ban_users: get_perm(4),
            send_messages: get_perm(5),
            read_messages: get_perm(6),
            manage_messages: get_perm(7),
            join_voice: get_perm(8),
        }
    }
}

const PERM_N_BYTES: usize = 3;

impl From<&Permissions> for Box<[u8]> {
    fn from(value: &Permissions) -> Self {
        Box::new([
            perms_to_byte([
                value.modify_channels,
                value.modify_icon_name,
                value.modify_groups,
                value.modify_user_groups,
            ]),
            perms_to_byte([
                value.ban_users,
                value.send_messages,
                value.read_messages,
                value.manage_messages,
            ]),
            perms_to_byte([
                value.join_voice,
                Perm::Default,
                Perm::Default,
                Perm::Default,
            ]),
        ])
    }
}

impl From<Permissions> for Box<[u8]> {
    fn from(value: Permissions) -> Self {
        (&value).into()
    }
}

impl Serialize for Permissions {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let bytes: Box<[u8]> = self.into();
        let mut seq = serializer.serialize_seq(Some(bytes.len()))?;
        for b in bytes {
            seq.serialize_element(&b)?;
        }
        seq.end()
    }
}

struct ArrayVisitor<A> {
    marker: PhantomData<A>,
}

impl<A> ArrayVisitor<A> {
    fn new() -> Self {
        ArrayVisitor {
            marker: PhantomData,
        }
    }
}

impl<'de> Visitor<'de> for ArrayVisitor<Vec<u8>> {
    type Value = Vec<u8>;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str(&format!(
            "an array of length at most {} (or is it really??? perms)",
            PERM_N_BYTES
        ))
    }

    #[inline]
    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut perms = vec![];

        while let Some(v) = seq.next_element()? {
            perms.push(v);
        }

        Ok(perms)
    }
}

impl<'a> Deserialize<'a> for Permissions {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'a>,
    {
        let arr = deserializer.deserialize_seq(ArrayVisitor::new())?;
        Ok(arr.as_slice().into())
    }
}
