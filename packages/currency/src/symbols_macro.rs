pub struct CurrencySymbols {
    pub bank: &'static str,
    pub dex: &'static str,
}

#[macro_export]
macro_rules! define_symbol {
    (
        $currency: ident {
            $([$($net: literal),+ $(,)?]: { $($body:tt)* }),+ $(,)?
        } $(,)?
    ) => {
        pub const $currency: $crate::symbols_macro::CurrencySymbols = {
            use $crate::symbols_macro::CurrencySymbols;

            $(
                #[cfg(any($(net_name = $net),+))]
                { CurrencySymbols { $($body)* } }
            )+
            #[cfg(all($($(not(net_name = $net)),+),+))]
            { compile_error!(concat!("No symbols defined for network with name \"", env!("NET_NAME"), "\"!")) }
        };
    };
}
