// See aster updates discord, pinned messages
use serde::{Deserialize, Serialize, Serializer};
use serde::ser::SerializeSeq;

use crate::helper::Uuid;
use crate::models::{Group, User};

#[derive(Clone, PartialEq, Debug)]
pub enum Perm {
    Allow,
    Deny,
    Default,
}

#[derive(Clone, PartialEq, Debug)]
pub struct ServerPerms {
    modify_channels: Perm,
    modify_icon_name: Perm,
    modify_groups: Perm,
    modify_user_groups: Perm,
    ban_users: Perm,
    channel_perms: ChannelPerms,
}

#[derive(Clone, PartialEq, Debug)]
pub struct ChannelPerms {
    send_messages: Perm,
    read_messages: Perm,
    manage_messages: Perm,
    join_voice: Perm,
}


#[derive(Clone, PartialEq, Eq, Debug, Hash)]
pub enum PermableEntity {
    User(Uuid),
    Group(Uuid),
}

impl Serialize for ChannelPerms {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> 
        where S: Serializer {
        let mut seq = serializer.serialize_seq(Some(2))?;
        // seq.serialize_element(self.0.start())?;
        // seq.serialize_element(self.0.end())?;
        seq.end()
    }
}

// struct ArrayVisitor<A> {
//     marker: PhantomData<A>,
// }

// impl<A> ArrayVisitor<A> {
//     fn new() -> Self {
//         ArrayVisitor {
//             marker: PhantomData,
//         }
//     }
// }
// impl<'de, T> Visitor<'de> for ArrayVisitor<[T; 2]>
// where
//     T: Deserialize<'de> + Copy,
// {
//     type Value = [T; 2];

//     fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
//         formatter.write_str(concat!("an array of length ", 2))
//     }

//     #[inline]
//     fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
//     where
//         A: SeqAccess<'de>,
//     {
//         Ok([
//             match seq.next_element()? {
//                 Some(val) => val,
//                 None => return Err(Error::invalid_length(0, &self)),
//             },
//             match seq.next_element()? {
//                 Some(val) => val,
//                 None => return Err(Error::invalid_length(1, &self)),
//             },
//         ])
//     }
// }


// impl<'a, T> Deserialize<'a> for Range<T> where T: Deserialize<'a> + Copy {
//     fn deserialize<D>(deserializer: D) -> Result<Self, D::Error> 
//         where D: Deserializer<'a> {
//         let arr = deserializer.deserialize_tuple(2, ArrayVisitor::<[T; 2]>::new())?;
//         Ok(Range{0:arr[0]..=arr[1]})
//     }
// }
