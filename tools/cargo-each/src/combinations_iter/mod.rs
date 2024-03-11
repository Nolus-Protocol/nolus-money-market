use std::{collections::BTreeSet, iter};

use anyhow::{Context, Result};
use cargo_metadata::Package;

use crate::{
    check,
    config::{Combination, Config, Set},
    either_iter::EitherIter,
    iter_or_else_iter::IterOrElseIter,
};

#[cfg(test)]
mod tests;

pub(crate) fn package_combinations<'r>(
    package: &'r Package,
    maybe_config: Option<&'r Config<'r>>,
    groups: Option<&'r BTreeSet<&'r str>>,
) -> Result<impl Iterator<Item = String> + 'r> {
    if let Some(config) = maybe_config {
        check::configuration(package, config)
            .context("Configuration checks failed!")
            .map(move |()| {
                Some(EitherIter::Left(configured_package_combinations(
                    package, config, groups,
                )))
            })
    } else {
        Ok(if groups.is_none() {
            Some(EitherIter::Right(build_combinations(
                package.features.keys().map(String::as_str),
            )))
        } else {
            eprintln!(
                r#"Package "{}" is not configured but groups are specified. Skipping over."#,
                package.name
            );

            None
        })
    }
    .map(Option::into_iter)
    .map(Iterator::flatten)
}

fn configured_package_combinations<'r>(
    package: &'r Package,
    config: &'r Config<'r>,
    groups: Option<&'r BTreeSet<&'r str>>,
) -> impl Iterator<Item = String> + 'r {
    let combinations = config.combinations.iter();

    let mut includes_empty = false;

    let iter = if let Some(groups) = groups {
        EitherIter::Left(
            combinations.filter(move |combination| combination.groups.is_superset(groups)),
        )
    } else {
        EitherIter::Right(combinations)
    }
    .flat_map(move |combination| {
        includes_empty = includes_empty | combination.always_on.is_empty()
            && !combination
                .sets
                .iter()
                .any(|set| config.sets[set].at_least_one);

        package_combination_variants(package, config, combination)
    });

    includes_empty.then(String::new).into_iter().chain(iter)
}

fn package_combination_variants<'r>(
    package: &'r Package,
    config: &'r Config<'r>,
    combination: &'r Combination<'r>,
) -> impl Iterator<Item = String> + 'r {
    let explicit_features = explicit_combination_features(config, combination);

    if combination.include_rest {
        EitherIter::Left(cross_join(
            combination_left_over_features(package, config, combination),
            explicit_features,
        ))
    } else {
        EitherIter::Right(explicit_features)
    }
}

fn explicit_combination_features<'r>(
    config: &'r Config<'r>,
    combination: &'r Combination<'r>,
) -> impl Iterator<Item = String> + 'r {
    combination_sets_variants(config, combination).map(|mut features| {
        combination.always_on.iter().copied().for_each(|feature| {
            if features.is_empty() {
                features = feature.to_string();
            } else {
                features.push(',');

                features.push_str(feature);
            }
        });

        features
    })
}

fn combination_sets_variants<'r>(
    config: &'r Config<'r>,
    combination: &'r Combination<'r>,
) -> impl Iterator<Item = String> + 'r {
    combination
        .sets
        .iter()
        .map(move |set| &config.sets[set])
        .filter(move |set| {
            set.mutually_exclusive && set.members.is_disjoint(&combination.always_on)
        })
        .map(move |set| {
            (!set.at_least_one)
                .then(String::new)
                .into_iter()
                .chain(from_set_members(set).map(String::from))
        })
        .fold(
            Box::new(non_exclusive_sets_variants(config, combination))
                as Box<dyn Iterator<Item = String> + 'r>,
            move |accumulator, exclusive_features| {
                Box::new(cross_join(exclusive_features, accumulator))
            },
        )
}

fn non_exclusive_sets_variants<'r>(
    config: &'r Config<'r>,
    combination: &'r Combination<'r>,
) -> impl Iterator<Item = String> + 'r {
    cross_join(
        optional_non_exclusive_sets_variants(config, combination),
        required_non_exclusive_sets_variants(config, combination),
    )
}

fn required_non_exclusive_sets_variants<'r>(
    config: &'r Config<'r>,
    combination: &'r Combination<'r>,
) -> impl Iterator<Item = String> + 'r {
    let mut combination_variants = combination
        .sets
        .iter()
        .map(|set| &config.sets[set])
        .filter(|set| !set.mutually_exclusive && set.at_least_one)
        .map(|set| {
            build_combinations_with_at_least_one(from_set_members_disjoint_from_always_on(
                combination,
                set,
            ))
        });

    combination_variants
        .next()
        .map(|first_variant| {
            combination_variants.fold(
                Box::new(first_variant) as Box<dyn Iterator<Item = String>>,
                |accumulator, variant| Box::new(cross_join(variant, accumulator)),
            )
        })
        .into_iter()
        .flatten()
}

fn optional_non_exclusive_sets_variants<'r>(
    config: &'r Config<'r>,
    combination: &'r Combination<'r>,
) -> impl Iterator<Item = String> + Clone + 'r {
    build_combinations(
        combination
            .sets
            .iter()
            .map(|set| &config.sets[set])
            .filter(|set| !(set.mutually_exclusive || set.at_least_one))
            .flat_map(|set| set.members.iter())
            .copied(),
    )
}

fn combination_left_over_features<'r>(
    package: &'r Package,
    config: &'r Config<'r>,
    combination: &'r Combination<'r>,
) -> impl Iterator<Item = String> + Clone + 'r {
    build_combinations(
        package
            .features
            .keys()
            .map(String::as_str)
            .filter(|feature| {
                !(combination.always_on.contains(feature)
                    || combination
                        .sets
                        .iter()
                        .any(|set| config.sets[set].members.contains(feature)))
            }),
    )
}

fn from_set_members_disjoint_from_always_on<'r>(
    combination: &'r Combination<'r>,
    set: &'r Set<'r>,
) -> impl Iterator<Item = &'r str> + Clone + 'r {
    from_set_members(set).filter(|member| !combination.always_on.contains(member))
}

fn from_set_members<'r>(set: &'r Set<'r>) -> impl Iterator<Item = &'r str> + Clone + 'r {
    set.members.iter().copied()
}

fn cross_join<'r, LeftIter, RightIter>(
    left_set: LeftIter,
    right_set: RightIter,
) -> impl Iterator<Item = String> + 'r
where
    LeftIter: Iterator<Item = String> + Clone + 'r,
    RightIter: Iterator + 'r,
    RightIter::Item: AsRef<str> + 'r,
{
    let cloned = left_set.clone();

    IterOrElseIter::new(
        right_set.flat_map(move |right_set_element| {
            let cloned = left_set.clone();

            if right_set_element.as_ref().is_empty() {
                EitherIter::Left(cloned)
            } else {
                let right_set_element = right_set_element.as_ref().to_string();

                let back_iter = Some(right_set_element.clone()).into_iter();

                EitherIter::Right(IterOrElseIter::new(
                    cloned.map(move |mut features_set| {
                        if features_set.is_empty() {
                            right_set_element.clone()
                        } else {
                            features_set.push(',');

                            features_set.push_str(&right_set_element);

                            features_set
                        }
                    }),
                    back_iter,
                ))
            }
        }),
        cloned,
    )
}

fn build_combinations<'r, I>(iter: I) -> impl Iterator<Item = String> + Clone + 'r
where
    I: Iterator<Item = &'r str> + Clone + 'r,
{
    Some(String::new())
        .into_iter()
        .chain(build_combinations_with_at_least_one(iter))
}

fn build_combinations_with_at_least_one<'r, I>(iter: I) -> impl Iterator<Item = String> + Clone + 'r
where
    I: Iterator<Item = &'r str> + Clone + 'r,
{
    let mut stack = Vec::with_capacity({
        let (min, max) = iter.size_hint();

        max.map_or(min, |max| max.min(min << 1)) + 1
    });

    stack.push((String::new(), iter));

    iter::from_fn(move || {
        while let Some((buffer, iter)) = stack.last_mut() {
            if let Some(next_feature) = iter.next() {
                let mut buffer = buffer.clone();

                let iter = iter.clone();

                buffer.push_str(next_feature);

                let output = buffer.clone();

                buffer.push(',');

                stack.push((buffer, iter));

                return Some(output);
            }

            stack.pop();
        }

        None
    })
}
