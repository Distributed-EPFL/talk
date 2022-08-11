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

    if !entries.is_empty() {
        let aggregation_root = aggregation_tree(entries.as_slice(), 32); // TODO: Add settings
        filter_recursion(statement, entries.as_ref(), Some(aggregation_root))
    } else {
        (None, Vec::new())
    }
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
            // (TODO) Remark: Normally this case happens when `entries.len() < chunk`. However, this could also
            // happen because aggregation previously failed for `chunk`. In this case, aggregation is bound to
            // fail again. This case should probably be handled to save useless aggregations.
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
                Signature::aggregate([left_signature, right_signature]).ok() // This is guaranteed to succeed
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

#[cfg(test)]
mod tests {
    use super::*;

    use crate::crypto::KeyChain;

    use serde::Serialize;

    use std::iter;

    #[derive(Serialize)]
    enum TestStatement {
        Correct,
        Incorrect,
    }

    impl Statement for TestStatement {
        type Header = ();
        const HEADER: Self::Header = ();
    }

    #[test]
    fn empty() {
        let (signature, exceptions) = filter_aggregate(&TestStatement::Correct, vec![]);

        assert!(signature.is_none());
        assert!(exceptions.is_empty());
    }

    #[test]
    fn correct() {
        let keychains = iter::repeat_with(KeyChain::random)
            .take(128)
            .collect::<Vec<_>>();

        let keycards = keychains.iter().map(KeyChain::keycard).collect::<Vec<_>>();

        let signatures = keychains
            .iter()
            .map(|keychain| keychain.multisign(&TestStatement::Correct).unwrap())
            .collect::<Vec<_>>();

        let (signature, exceptions) = filter_aggregate(
            &TestStatement::Correct,
            keycards.iter().zip(signatures.iter()),
        );

        let reference_signature = Signature::aggregate(signatures).unwrap();

        assert_eq!(signature.unwrap(), reference_signature);
        assert!(exceptions.is_empty());
    }

    #[test]
    fn one_incorrect() {
        let keychains = iter::repeat_with(KeyChain::random)
            .take(128)
            .collect::<Vec<_>>();

        let keycards = keychains.iter().map(KeyChain::keycard).collect::<Vec<_>>();

        let mut signatures = keychains
            .iter()
            .map(|keychain| keychain.multisign(&TestStatement::Correct).unwrap())
            .collect::<Vec<_>>();

        *signatures.get_mut(33).unwrap() = keychains
            .get(33)
            .unwrap()
            .multisign(&TestStatement::Incorrect)
            .unwrap();

        let (signature, exceptions) = filter_aggregate(
            &TestStatement::Correct,
            keycards.iter().zip(signatures.iter()),
        );

        let reference_signature = Signature::aggregate(
            (&signatures[..33])
                .iter()
                .copied()
                .chain((&signatures[34..]).iter().copied()),
        )
        .unwrap();

        assert_eq!(signature.unwrap(), reference_signature);
        assert_eq!(exceptions, vec![33]);
    }

    #[test]
    fn multiple_incorrect() {
        let keychains = iter::repeat_with(KeyChain::random)
            .take(128)
            .collect::<Vec<_>>();

        let keycards = keychains.iter().map(KeyChain::keycard).collect::<Vec<_>>();

        let signatures = keychains
            .iter()
            .enumerate()
            .map(|(index, keychain)| {
                keychain
                    .multisign(if index % 3 == 0 {
                        &TestStatement::Incorrect
                    } else {
                        &TestStatement::Correct
                    })
                    .unwrap()
            })
            .collect::<Vec<_>>();

        let (signature, exceptions) = filter_aggregate(
            &TestStatement::Correct,
            keycards.iter().zip(signatures.iter()),
        );

        let reference_signature =
            Signature::aggregate(signatures.into_iter().enumerate().filter_map(
                |(index, signature)| {
                    if index % 3 == 0 {
                        None
                    } else {
                        Some(signature)
                    }
                },
            ))
            .unwrap();

        let reference_exceptions = (0..128)
            .into_iter()
            .filter(|index| index % 3 == 0)
            .collect::<Vec<_>>();

        assert_eq!(signature.unwrap(), reference_signature);
        assert_eq!(exceptions, reference_exceptions);
    }

    #[test]
    fn incorrect() {
        let keychains = iter::repeat_with(KeyChain::random)
            .take(128)
            .collect::<Vec<_>>();

        let keycards = keychains.iter().map(KeyChain::keycard).collect::<Vec<_>>();

        let signatures = keychains
            .iter()
            .map(|keychain| keychain.multisign(&TestStatement::Incorrect).unwrap())
            .collect::<Vec<_>>();

        let (signature, exceptions) = filter_aggregate(
            &TestStatement::Correct,
            keycards.iter().zip(signatures.iter()),
        );

        assert!(signature.is_none());
        assert_eq!(exceptions, (0..128).into_iter().collect::<Vec<_>>());
    }
}
