use std::collections::btree_map::{BTreeMap, Entry};

use serde::{Deserialize, Serialize};

use crate::{
    node::{Node, NodeIndex},
    tree::{Nodes, Tree},
};

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct HumanReadableTree<T> {
    root: HrtNode<T>,
}

impl<T> HumanReadableTree<T> {
    pub fn into_tree(self) -> Tree<T> {
        Tree {
            nodes: self
                .root
                .flatten(Tree::<T>::ROOT_PARENT, Tree::<T>::ROOT_INDEX),
        }
    }

    pub fn from_tree(mut tree: Tree<T>) -> Self {
        let root_index = Tree::<T>::ROOT_INDEX.into();

        let mut child_nodes: BTreeMap<usize, Vec<HrtNode<T>>> = BTreeMap::new();

        let mut indexes_mapping = (0..tree.nodes.len()).collect::<Vec<usize>>();

        while let Some((node, index_mapping)) = tree
            .nodes
            .iter()
            .enumerate()
            .filter(|&(index, _)| index != root_index)
            .max_by_key(|(_, node)| node.parent_index())
            .map(|(index, _)| index)
            .map(|index| (tree.nodes.remove(index), indexes_mapping.remove(index)))
        {
            let parent_index = node.parent_index().into();

            let node = Self::convert_node_to_hrt(&mut child_nodes, node, index_mapping);

            match child_nodes.entry(parent_index) {
                Entry::Vacant(entry) => _ = entry.insert(vec![node]),
                Entry::Occupied(entry) => entry.into_mut().push(node),
            }
        }

        let root =
            Self::convert_node_to_hrt(&mut child_nodes, tree.nodes.remove(root_index), root_index);

        debug_assert_eq!(indexes_mapping, [root_index]);

        debug_assert!(child_nodes.is_empty());

        Self { root }
    }

    fn convert_node_to_hrt(
        child_nodes: &mut BTreeMap<usize, Vec<HrtNode<T>>>,
        node: Node<T>,
        index_mapping: usize,
    ) -> HrtNode<T> {
        let value = node.into_value();

        #[expect(if_let_rescope)]
        // TODO remove once stop linting with the 'rust-2024-compatibility' group
        if let Some(mut children) = child_nodes.remove(&index_mapping) {
            children.reverse();

            HrtNode::Branch { value, children }
        } else {
            HrtNode::Leaf { value }
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(untagged, from = "HrtNodeStruct<T>")]
pub enum HrtNode<T> {
    Leaf { value: T },
    Branch { value: T, children: Vec<HrtNode<T>> },
}

impl<T> From<HrtNodeStruct<T>> for HrtNode<T> {
    fn from(HrtNodeStruct { value, children }: HrtNodeStruct<T>) -> Self {
        if children.is_empty() {
            HrtNode::Leaf { value }
        } else {
            HrtNode::Branch { value, children }
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HrtNodeStruct<T> {
    value: T,
    #[serde(default = "Vec::new")]
    children: Vec<HrtNode<T>>,
}

impl<T> HrtNode<T> {
    fn flatten(self, parent: NodeIndex, this: NodeIndex) -> Nodes<T> {
        match self {
            HrtNode::Leaf { value } => {
                vec![Node::new(parent, value)]
            }
            HrtNode::Branch { value, children } => {
                children
                    .into_iter()
                    .fold(vec![Node::new(parent, value)], |mut nodes, node| {
                        #[expect(if_let_rescope)]
                        // TODO remove once stop linting with the 'rust-2024-compatibility' group
                        if let Self::Leaf { value } = node {
                            nodes.push(Node::new(this, value));
                        } else {
                            nodes.append(
                                &mut node.flatten(
                                    this,
                                    this + NodeIndex::try_from(nodes.len())
                                        .expect("Tree contains too many elements!"),
                                ),
                            );
                        }

                        nodes
                    })
            }
        }
    }
}
