use frame_support::pallet_macros::*;

/// Need to add the errors here.
#[pallet_section]
mod errors {
	#[pallet::error]
	pub enum Error<T> {
		ExampleError,
	}
}
