use std::iter;

use anyhow::Result;

use either::Either;
use topology::HostCurrency;

use crate::{
    protocol::Protocol,
    sources::resolved_currency::{CurrentModule, ResolvedCurrency},
    subtype_lifetime::SubtypeLifetime,
};

use super::DexCurrencies;

pub(super) fn pairs_group<'r, 'dex_currencies, 'child, Parents>(
    current_module: CurrentModule,
    protocol: &Protocol,
    host_currency: &HostCurrency,
    dex_currencies: &'dex_currencies DexCurrencies<'_, '_>,
    name: &'r str,
    mut parents: Parents,
) -> Result<impl Iterator<Item = &'r str> + use<'r, Parents>>
where
    'dex_currencies: 'r,
    Parents: Iterator<Item = &'child str>,
{
    let matcher;

    if let Some(ticker) = parents.next() {
        matcher = "matcher";

        PairsGroupTemplate::new(
            current_module,
            protocol,
            host_currency,
            dex_currencies,
            matcher,
            "visitor",
        )
        .apply(ticker, parents)
        .map(Either::Left)
    } else {
        matcher = "_";

        Ok(Either::Right(iter::once("currency::visit_noone(visitor)")))
    }
    .map(|sources| {
        [
            r#"
    impl currency::PairsGroup for "#,
            name,
            r#" {
        type CommonGroup = crate::payment::Group;

        fn maybe_visit<M, V>(
            "#,
            matcher,
            r#": &M,
            visitor: V,
        ) -> currency::MaybePairsVisitorResult<V>
        where
            M: currency::Matcher,
            V: currency::PairsVisitor<Pivot = Self>,
        {
            "#,
        ]
        .into_iter()
        .chain(sources.map(SubtypeLifetime::subtype))
        .chain(iter::once(
            r#"
        }
    }
"#,
        ))
    })
}

struct PairsGroupTemplate<
    'protocol,
    'host_currency,
    'dex_currencies,
    'dex_currency_ticker,
    'dex_currency_definition,
> {
    current_module: CurrentModule,
    protocol: &'protocol Protocol,
    host_currency: &'host_currency HostCurrency,
    dex_currencies: &'dex_currencies DexCurrencies<'dex_currency_ticker, 'dex_currency_definition>,
    matcher_parameter_name: &'static str,
    visitor_parameter_name: &'static str,
}

impl<
        'protocol,
        'host_currency,
        'dex_currencies,
        'dex_currency_ticker,
        'dex_currency_definition,
    >
    PairsGroupTemplate<
        'protocol,
        'host_currency,
        'dex_currencies,
        'dex_currency_ticker,
        'dex_currency_definition,
    >
{
    #[inline]
    const fn new(
        current_module: CurrentModule,
        protocol: &'protocol Protocol,
        host_currency: &'host_currency HostCurrency,
        dex_currencies: &'dex_currencies DexCurrencies<
            'dex_currency_ticker,
            'dex_currency_definition,
        >,
        matcher_parameter_name: &'static str,
        visitor_parameter_name: &'static str,
    ) -> Self {
        Self {
            current_module,
            protocol,
            host_currency,
            dex_currencies,
            matcher_parameter_name,
            visitor_parameter_name,
        }
    }
}

impl<'dex_currencies> PairsGroupTemplate<'_, '_, 'dex_currencies, '_, '_> {
    fn apply<'child, Children>(
        &self,
        ticker: &str,
        children: Children,
    ) -> Result<impl Iterator<Item = &'dex_currencies str> + use<'dex_currencies, Children>>
    where
        Children: Iterator<Item = &'child str>,
    {
        const PAIRS_GROUP_ENTRIES_PREPEND: &str = "use currency::maybe_visit_buddy as visit;

            ";

        let visit_entry_template = VisitEntryTemplate::new(
            self.current_module,
            self.protocol,
            self.host_currency,
            self.dex_currencies,
            "visit",
            self.matcher_parameter_name,
            self.visitor_parameter_name,
        );

        visit_entry_template.apply(ticker).and_then(|first_entry| {
            children
                .map(|ticker| {
                    visit_entry_template.apply(ticker).map(|entry| {
                        [
                            "
                .or_else(|",
                            self.visitor_parameter_name,
                            "| ",
                        ]
                        .into_iter()
                        .chain(entry)
                        .chain(iter::once(")"))
                    })
                })
                .collect::<Result<_, _>>()
                .map(Vec::into_iter)
                .map(Iterator::flatten)
                .map(move |rest_of_entries| {
                    iter::once(PAIRS_GROUP_ENTRIES_PREPEND)
                        .chain(first_entry)
                        .chain(rest_of_entries)
                })
        })
    }
}

struct VisitEntryTemplate<
    'protocol,
    'host_currency,
    'dex_currencies,
    'dex_currency_ticker,
    'dex_currency_definition,
> {
    current_module: CurrentModule,
    protocol: &'protocol Protocol,
    host_currency: &'host_currency HostCurrency,
    dex_currencies: &'dex_currencies DexCurrencies<'dex_currency_ticker, 'dex_currency_definition>,
    visit_function: &'static str,
    matcher_parameter_name: &'static str,
    visitor_parameter_name: &'static str,
}

impl<
        'protocol,
        'host_currency,
        'dex_currencies,
        'dex_currency_ticker,
        'dex_currency_definition,
    >
    VisitEntryTemplate<
        'protocol,
        'host_currency,
        'dex_currencies,
        'dex_currency_ticker,
        'dex_currency_definition,
    >
{
    #[inline]
    const fn new(
        current_module: CurrentModule,
        protocol: &'protocol Protocol,
        host_currency: &'host_currency HostCurrency,
        dex_currencies: &'dex_currencies DexCurrencies<
            'dex_currency_ticker,
            'dex_currency_definition,
        >,
        visit_function: &'static str,
        matcher_parameter_name: &'static str,
        visitor_parameter_name: &'static str,
    ) -> Self {
        Self {
            current_module,
            protocol,
            host_currency,
            dex_currencies,
            visit_function,
            matcher_parameter_name,
            visitor_parameter_name,
        }
    }
}

impl<'dex_currencies> VisitEntryTemplate<'_, '_, 'dex_currencies, '_, '_> {
    fn apply(&self, ticker: &str) -> Result<impl IntoIterator<Item = &'dex_currencies str>> {
        ResolvedCurrency::resolve(
            self.current_module,
            self.protocol,
            self.host_currency,
            self.dex_currencies,
            ticker,
        )
        .map(|resolved| {
            [
                self.visit_function,
                "::<",
                resolved.module(),
                "::",
                resolved.name(),
                ", _, _>(",
                self.matcher_parameter_name,
                ", ",
                self.visitor_parameter_name,
                ")",
            ]
        })
    }
}
