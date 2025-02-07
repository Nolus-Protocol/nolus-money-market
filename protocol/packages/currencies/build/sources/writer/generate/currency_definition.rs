use std::{borrow::Cow, iter};

use anyhow::{anyhow, Context as _, Result};
use either::Either;

use crate::currencies_tree::{self, CurrenciesTree};

use super::{super::super::generator, GeneratedSourceEntry};

pub(super) type GenerateEntryResult<'dex_currency, MaybeVisit, CurrencyDefinition> = Result<
    GeneratedSourceEntry<Either<MaybeVisit, iter::Empty<&'dex_currency str>>, CurrencyDefinition>,
>;

pub(super) struct CurrencyDefinition<
    'currencies_tree,
    'parents_of,
    'parent,
    'children_of,
    'child,
    'generator,
    Generator,
> {
    currencies_tree: &'currencies_tree CurrenciesTree<'parents_of, 'parent, 'children_of, 'child>,
    generator: &'generator Generator,
    visited_group: &'static str,
    visit_function: &'static str,
    matcher_parameter: &'static str,
    visitor_parameter: &'static str,
}

impl<'currencies_tree, 'parents_of, 'parent, 'children_of, 'child, 'generator, Generator>
    CurrencyDefinition<
        'currencies_tree,
        'parents_of,
        'parent,
        'children_of,
        'child,
        'generator,
        Generator,
    >
{
    #[inline]
    pub const fn new(
        currencies_tree: &'currencies_tree CurrenciesTree<
            'parents_of,
            'parent,
            'children_of,
            'child,
        >,
        generator: &'generator Generator,
        visited_group: &'static str,
        visit_function: &'static str,
        matcher_parameter: &'static str,
        visitor_parameter: &'static str,
    ) -> Self {
        Self {
            currencies_tree,
            generator,
            visited_group,
            visit_function,
            matcher_parameter,
            visitor_parameter,
        }
    }

    #[inline]
    pub const fn visited_group(&self) -> &'static str {
        self.visited_group
    }

    #[inline]
    pub const fn visit_function(&self) -> &'static str {
        self.visit_function
    }

    #[inline]
    pub const fn matcher_parameter(&self) -> &'static str {
        self.matcher_parameter
    }

    #[inline]
    pub const fn visitor_parameter(&self) -> &'static str {
        self.visitor_parameter
    }
}

impl<
        'dex_currencies,
        'definition,
        'dex_currency_ticker,
        'dex_currency_definition,
        'currencies_tree,
        'generator,
        Generator,
    > CurrencyDefinition<'currencies_tree, '_, '_, '_, '_, 'generator, Generator>
where
    'dex_currencies: 'definition,
    'dex_currency_ticker: 'dex_currencies,
    'dex_currency_definition: 'dex_currencies,
    Generator: generator::Resolver<'dex_currencies, 'definition>
        + generator::MaybeVisit
        + generator::PairsGroup<'dex_currencies, 'dex_currency_ticker, 'dex_currency_definition>
        + generator::InPoolWith<'dex_currencies, 'dex_currency_ticker, 'dex_currency_definition>,
{
    pub(super) fn generate_entry<'r>(
        &self,
        ticker: &'r str,
    ) -> GenerateEntryResult<
        'dex_currencies,
        impl IntoIterator<Item = &'dex_currencies str> + use<'dex_currencies, Generator>,
        impl Iterator<Item = Cow<'r, str>>
            + use<
                'r,
                'dex_currencies,
                'dex_currency_ticker,
                'dex_currency_definition,
                'currencies_tree,
                'generator,
                Generator,
            >,
    >
    where
        'definition: 'r,
    {
        let parents = self.currencies_tree.parents(ticker);

        let children = self.currencies_tree.children(ticker);

        if [children.as_ref(), parents.as_ref()]
            .into_iter()
            .any(|paired_with| paired_with.contains(ticker))
        {
            Err(anyhow!("Currency cannot be in a pool with itself!"))
        } else {
            self.generate_entry_unchecked(ticker, children, parents)
        }
    }

    fn generate_entry_unchecked<'r, 'children, 'child, 'parents, 'parent>(
        &self,
        ticker: &'r str,
        children: &'children currencies_tree::Children<'child>,
        parents: &'parents currencies_tree::Parents<'parent>,
    ) -> GenerateEntryResult<
        'dex_currencies,
        impl IntoIterator<Item = &'dex_currencies str> + use<'dex_currencies, Generator>,
        impl Iterator<Item = Cow<'r, str>>
            + use<
                'r,
                'dex_currencies,
                'dex_currency_ticker,
                'dex_currency_definition,
                'currencies_tree,
                'children,
                'parents,
                'generator,
                Generator,
            >,
    >
    where
        'definition: 'r,
    {
        self.generator
            .resolve(ticker)
            .context("Failed to generate currency definition sources!")
            .and_then(|resolved| {
                self.generator
                    .pairs_group(resolved.name(), parents)
                    .and_then(|pairs_group| {
                        self.generator
                            .in_pool_with(resolved.name(), children)
                            .map(|in_pool_with| GeneratedSourceEntry {
                                maybe_visit: if <Generator as generator::MaybeVisit>::GENERATE {
                                    Either::Left([
                                        self.visit_function,
                                        "::<_, self::definitions::",
                                        resolved.name(),
                                        ", ",
                                        self.visited_group,
                                        ", _>(",
                                        self.matcher_parameter,
                                        ", ",
                                        self.visitor_parameter,
                                        ")",
                                    ])
                                } else {
                                    Either::Right(iter::empty())
                                },
                                currency_definition: currency_definition(
                                    resolved.name(),
                                    ticker,
                                    resolved.definition(),
                                )
                                .chain(pairs_group.chain(in_pool_with).map(Cow::Borrowed)),
                            })
                    })
            })
    }
}

fn currency_definition<'r>(
    name: &'r str,
    ticker: &'r str,
    currency: &'r topology::CurrencyDefinition,
) -> impl Iterator<Item = Cow<'r, str>> + use<'r> {
    [
        r#"
    #[derive(
        Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize,
        serde::Deserialize,
    )]
    #[serde(deny_unknown_fields, rename_all = "snake_case")]
    pub struct "#,
        name,
        r#"(currency::CurrencyDTO<super::super::Group>);

    impl currency::CurrencyDef for "#,
        name,
        r#" {
        type Group = super::super::Group;

        fn dto() -> &'static currency::CurrencyDTO<Self::Group> {
            const {
                &currency::CurrencyDTO::new(
                    const {
                        &currency::Definition::new(
                            ""#,
        ticker,
        r#"",
                            // "#,
        currency.host().path(),
        r#"
                            ""#,
        currency.host().symbol(),
        r#"",
                            // "#,
        currency.dex().path(),
        r#"
                            ""#,
        currency.dex().symbol(),
        r#"",
                            "#,
    ]
    .into_iter()
    .map(Cow::Borrowed)
    .chain(iter::once(Cow::Owned(
        currency.decimal_digits().to_string(),
    )))
    .chain(iter::once(
        const {
            Cow::Borrowed(
                r#",
                        )
                    },
                )
            }
        }
    }
"#,
            )
        },
    ))
}
