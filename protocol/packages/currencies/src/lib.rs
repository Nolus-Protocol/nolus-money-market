#[cfg(not(any(feature = "astroport", feature = "osmosis")))]
compile_error!("No protocol selected!");

#[cfg(not(any(feature = "net_dev", feature = "net_test", feature = "net_main")))]
compile_error!("No net selected!");

mod currency_macro;
mod lease;
mod lpn;
mod native;
mod payment;
mod symbols_macro;

#[cfg(feature = "testing")]
pub mod test;

#[cfg(test)]
mod test_impl;

pub use lease::LeaseGroup;
pub use lpn::Lpns;
pub use native::{Native, Nls};
pub use payment::PaymentGroup;

use self::{currency_macro::define_currency, symbols_macro::define_symbol};
