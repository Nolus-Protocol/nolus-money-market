use serde::{de, de::SeqAccess, ser::SerializeSeq, Deserialize, Deserializer, Serialize, Serializer, de::Visitor};
use finance::currency::SymbolOwned;
use trees::{Tree, tr};
use std::ops::{Deref, DerefMut};

#[derive(Clone, PartialEq, Eq)]
pub struct TreeStore(pub trees::Tree<SymbolOwned>);

impl Deref for TreeStore {
    type Target = Tree<SymbolOwned>;
    fn deref(&self) -> &Self::Target {
       &self.0 
    }
}

impl DerefMut for TreeStore {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Serialize for TreeStore {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
            TreeStoreRef(&self.0).serialize(serializer)
    }
}

struct TreeStoreRef<'b>(pub &'b trees::Node<SymbolOwned>);

impl<'b> Serialize for TreeStoreRef<'b> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let len = self.0.degree() + 1;
        if len == 1 {
            self.0.data().serialize(serializer)
        } else {
            let mut seq = serializer.serialize_seq(Some(len))?;
            seq.serialize_element(self.0.data())?;
            for child in self.0.iter() {
                seq.serialize_element(&TreeStoreRef(child))?;
            }
            seq.end()
        }
    }
}

impl<'de> Deserialize<'de> for TreeStore {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct TreeVisitor;

        impl<'de> Visitor<'de> for TreeVisitor {
            type Value = TreeStore;

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
                where
                    E: de::Error, {
                // can't use visit_string with deserialize_any to move value
                Ok(TreeStore(Tree::new(v.into())))
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let mut root = if let Some(root) = seq.next_element()? {
                    Ok(tr(root))
                } else {
                    Err(de::Error::custom("no root element"))
                }?;

                while let Some(leaf) = seq.next_element::<TreeStore>()? {
                    root.push_back(leaf.0);
                }
                Ok(TreeStore(root))
            }

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("format: [root,[node, leaf], leaf]")
            }
        }

        deserializer.deserialize_any(TreeVisitor)
    }
}
