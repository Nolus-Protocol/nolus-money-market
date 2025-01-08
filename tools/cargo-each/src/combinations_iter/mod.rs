use std::iter;

use anyhow::{Context, Result};
use cargo_metadata::Package;

use either::Either;

use crate::{
    check,
    config::{Combination, Config, FeatureGroup},
    iter_or_else_iter::IterOrElseIter,
    subcommands::Tags,
};

#[cfg(test)]
mod tests;

pub(crate) fn package_combinations<'r>(
    package: &'r Package,
    maybe_config: Option<&'r Config<'r>>,
    tags: Tags<'r>,
) -> Result<impl Iterator<Item = String> + 'r> {
    if let Some(config) = maybe_config {
        check::configuration(package, config)
            .context("Configuration checks failed!")
            .map(move |()| {
                Some(Either::Left(configured_package_combinations(
                    package, config, tags,
                )))
            })
    } else {
        Ok(if tags.is_none() {
            Some(Either::Right(build_combinations(
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
    tags: Tags<'r>,
) -> impl Iterator<Item = String> + 'r {
    let combinations = config.combinations.iter();

    let mut includes_empty = false;

    let iter = if let Some(tags) = tags {
        Either::Left(combinations.filter(move |combination| combination.tags.is_superset(tags)))
    } else {
        Either::Right(combinations)
    }
    .flat_map(move |combination| {
        includes_empty = includes_empty | combination.always_on.is_empty()
            && !combination
                .feature_groups
                .iter()
                .any(|feature_group| config.feature_groups[feature_group].at_least_one);

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
        Either::Left(cross_join(
            combination_left_over_features(package, config, combination),
            explicit_features,
        ))
    } else {
        Either::Right(explicit_features)
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
        .feature_groups
        .iter()
        .map(move |feature_group| &config.feature_groups[feature_group])
        .filter(move |feature_group| {
            feature_group.mutually_exclusive
                && feature_group.members.is_disjoint(&combination.always_on)
        })
        .map(move |feature_group| {
            (!feature_group.at_least_one)
                .then(String::new)
                .into_iter()
                .chain(from_group_members(feature_group).map(String::from))
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
        .feature_groups
        .iter()
        .map(|feature_group| &config.feature_groups[feature_group])
        .filter(|feature_group| !feature_group.mutually_exclusive && feature_group.at_least_one)
        .map(|feature_group| {
            build_combinations_with_at_least_one(from_group_members_disjoint_from_always_on(
                combination,
                feature_group,
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
            .feature_groups
            .iter()
            .map(|feature_group| &config.feature_groups[feature_group])
            .filter(|feature_group| {
                !(feature_group.mutually_exclusive || feature_group.at_least_one)
            })
            .flat_map(|feature_group| feature_group.members.iter())
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
                    || combination.feature_groups.iter().any(|feature_group| {
                        config.feature_groups[feature_group]
                            .members
                            .contains(feature)
                    }))
            }),
    )
}

fn from_group_members_disjoint_from_always_on<'r>(
    combination: &'r Combination<'r>,
    feature_group: &'r FeatureGroup<'r>,
) -> impl Iterator<Item = &'r str> + Clone + 'r {
    from_group_members(feature_group).filter(|member| !combination.always_on.contains(member))
}

fn from_group_members<'r>(
    feature_group: &'r FeatureGroup<'r>,
) -> impl Iterator<Item = &'r str> + Clone + 'r {
    feature_group.members.iter().copied()
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
                Either::Left(cloned)
            } else {
                let right_set_element = right_set_element.as_ref().to_string();

                let back_iter = Some(right_set_element.clone()).into_iter();

                Either::Right(IterOrElseIter::new(
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
