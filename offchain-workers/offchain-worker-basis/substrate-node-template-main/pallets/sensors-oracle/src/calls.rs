use frame_support::pallet_macros::*;

#[pallet_section]
mod calls {

	use frame_system;
	use sp_std::vec::Vec;

	/// A public part of the pallet.
    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::call_index(0)]
        #[pallet::weight((0, Pays::No))]
        pub fn update_sensors_data(
            origin: OriginFor<T>,
            updated_data: Vec<SensorData>,
        ) -> DispatchResultWithPostInfo {
            // Retrieve sender of the transaction.
            let who = ensure_signed(origin)?;

            match Self::is_authority(&who) {
                true => {
                    for sensor in updated_data {
                        Self::add_sensor_data(sensor);
                    }
                }
                false => return Err(Error::<T>::NotAuthority.into()),
            }

            // Authorized OCWs don't need to pay fees
            Ok(Pays::No.into())
        }

        #[pallet::call_index(1)]
        pub fn add_authority(
            origin: OriginFor<T>,
            authority: T::AccountId,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;

            ensure!(
                !Self::is_authority(&authority),
                Error::<T>::AlreadyAuthority
            );

            let mut authorities = <Authorities<T>>::get();
            match authorities.try_push(authority.clone()) {
                Ok(()) => (),
                Err(_) => return Err(Error::<T>::TooManyAuthorities.into()),
            };

            Authorities::<T>::set(authorities);

            Self::deposit_event(Event::AuthorityAdded { authority });

            Ok(().into())
        }

        #[pallet::call_index(2)]
        pub fn remove_authority(
            origin: OriginFor<T>,
            authority: T::AccountId,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;

            ensure!(Self::is_authority(&authority), Error::<T>::NotAuthority);

            let mut authorities = <Authorities<T>>::get();
            match authorities.iter().position(|a| a == &authority) {
                Some(index) => authorities.swap_remove(index),
                None => return Err(Error::<T>::NotAuthority.into()),
            };

            Authorities::<T>::set(authorities);

            Self::deposit_event(Event::AuthorityAdded { authority });

            Ok(().into())
        }
    }
}