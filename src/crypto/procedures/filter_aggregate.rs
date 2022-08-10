use crate::crypto::{primitives::multi::Signature, KeyCard, Statement};

struct Entry<'a> {
    index: usize,
    keycard: &'a KeyCard,
    signature: &'a Signature,
}

struct AggregationNode {
    signature: Option<Signature>,
    children: Option<(Box<AggregationNode>, Box<AggregationNode>)>,
}

pub fn filter_aggregate<'a, S, E>(statement: &S, entries: E) -> (Option<Signature>, Vec<usize>)
where
    S: Sync + Statement,
    E: IntoIterator<Item = (&'a KeyCard, &'a Signature)>,
{
    let entries = entries
        .into_iter()
        .enumerate()
        .map(|(index, (keycard, signature))| Entry {
            index,
            keycard,
            signature,
        })
        .collect::<Vec<_>>();

    let aggregation_root = aggregation_tree(entries.as_slice(), 32); // TODO: Add settings
    filter_recursion(statement, entries.as_ref(), Some(aggregation_root))
}

fn aggregation_tree(entries: &[Entry], chunk: usize) -> AggregationNode {
    if entries.len() > chunk {
        let mid = entries.len() / 2;
        let (left_entries, right_entries) = entries.split_at(mid);

        let (left_node, right_node) = rayon::join(
            || aggregation_tree(left_entries, chunk),
            || aggregation_tree(right_entries, chunk),
        );

        let signature = match (&left_node.signature, &right_node.signature) {
            (Some(left_multisignature), Some(right_multisignature)) => {
                Signature::aggregate([*left_multisignature, *right_multisignature]).ok()
            }
            _ => None,
        };

        AggregationNode {
            signature,
            children: Some((Box::new(left_node), Box::new(right_node))),
        }
    } else {
        AggregationNode {
            signature: Signature::aggregate(entries.iter().map(|entry| entry.signature).copied())
                .ok(),
            children: None,
        }
    }
}

fn filter_recursion<S>(
    statement: &S,
    entries: &[Entry],
    aggregation_node: Option<AggregationNode>,
) -> (Option<Signature>, Vec<usize>)
where
    S: Sync + Statement,
{
    let (signature, children) = match aggregation_node {
        Some(aggregation_node) => (aggregation_node.signature, aggregation_node.children),
        None => {
            let signatures = entries.iter().map(|entry| entry.signature).copied();
            (Signature::aggregate(signatures).ok(), None)
        }
    };

    let keycards = entries.iter().map(|entry| entry.keycard);
    let signature = signature.filter(|signature| signature.verify(keycards, statement).is_ok());

    if signature.is_some() {
        (signature, Vec::new())
    } else {
        filter_split(statement, entries, children)
    }
}

fn filter_split<S>(
    statement: &S,
    entries: &[Entry],
    aggregation_children: Option<(Box<AggregationNode>, Box<AggregationNode>)>,
) -> (Option<Signature>, Vec<usize>)
where
    S: Sync + Statement,
{
    if entries.len() == 1 {
        let index = entries.first().unwrap().index;
        (None, vec![index])
    } else {
        let mid = entries.len() / 2;
        let (left_entries, right_entries) = entries.split_at(mid);

        let (left_child, right_child) = match aggregation_children {
            Some((left_child, right_child)) => (Some(*left_child), Some(*right_child)),
            None => (None, None),
        };

        let ((left_signature, mut left_exceptions), (right_signature, mut right_exceptions)) =
            rayon::join(
                || filter_recursion(statement, left_entries, left_child),
                || filter_recursion(statement, right_entries, right_child),
            );

        let signature = match (left_signature, right_signature) {
            (Some(left_signature), Some(right_signature)) => {
                Signature::aggregate([left_signature, right_signature]).ok()
            }
            (Some(left_signature), None) => Some(left_signature),
            (None, Some(right_signature)) => Some(right_signature),
            (None, None) => None,
        };

        let mut exceptions = Vec::new();
        exceptions.append(&mut left_exceptions);
        exceptions.append(&mut right_exceptions);

        (signature, exceptions)
    }
}
