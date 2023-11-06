use frame_support::pallet_macros::*;

#[pallet_section]
mod events {
	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Event generated when a new authority is added.
		AuthorityAdded { authority: T::AccountId },
		/// Event generated when an authority is removed.
		AuthorityRemoved { authority: T::AccountId },
        /// Event generated when new sensor data is added.
        SensorDataAdded { id: u32, type_: SensorType },
	}
}