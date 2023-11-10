#![cfg_attr(not(feature = "std"), no_std)]

mod calls;
mod config;
mod errors;
mod events;
pub mod types;

use crate::types::*;

use crate::pallet::{Authorities, Sensors};

use frame_support::{pallet_macros::*, pallet_prelude::*};
use frame_system::{
    self as system,
    offchain::{AppCrypto, CreateSignedTransaction, SendSignedTransaction, Signer},
    pallet_prelude::*,
};
use sp_core::crypto::KeyTypeId;
use sp_runtime::offchain::{http, Duration};
use sp_std::vec::Vec;

#[cfg(test)]
mod tests;

/// Defines application identifier for crypto keys of this module.
///
/// Every module that deals with signatures needs to declare its unique identifier for its crypto
/// keys.
///
/// When offchain worker is signing transactions it's going to request keys of type `KeyTypeId` from
/// the keystore and use the ones it finds to sign the transaction. The keys can be inserted
/// manually via RPC (see `author_insertKey`).
pub const KEY_TYPE: KeyTypeId = KeyTypeId(*b"sens");

/// Based on the above `KeyTypeId` we need to generate a pallet-specific crypto type wrappers. We
/// can use from supported crypto kinds (`sr25519`, `ed25519` and `ecdsa`) and augment the types
/// with this pallet-specific identifier.
pub mod crypto {
    use super::KEY_TYPE;
    use sp_core::sr25519::Signature as Sr25519Signature;
    use sp_runtime::{
        app_crypto::{app_crypto, sr25519},
        traits::Verify,
        MultiSignature, MultiSigner,
    };
    app_crypto!(sr25519, KEY_TYPE);

    pub struct TestAuthId;

    impl frame_system::offchain::AppCrypto<MultiSigner, MultiSignature> for TestAuthId {
        type RuntimeAppPublic = Public;
        type GenericSignature = sp_core::sr25519::Signature;
        type GenericPublic = sp_core::sr25519::Public;
    }

    // implemented for mock runtime in test
    impl frame_system::offchain::AppCrypto<<Sr25519Signature as Verify>::Signer, Sr25519Signature>
        for TestAuthId
    {
        type RuntimeAppPublic = Public;
        type GenericSignature = sp_core::sr25519::Signature;
        type GenericPublic = sp_core::sr25519::Public;
    }
}

pub use pallet::*;

#[import_section(events::events)]
#[import_section(errors::errors)]
#[import_section(config::config)]
#[import_section(calls::calls)]
#[frame_support::pallet]
pub mod pallet {
    use super::*;

    use core::fmt;
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;
    use serde_json::error;

    /// A double storage map with the sensors data.
    #[pallet::storage]
    #[pallet::getter(fn sensors)]
    pub(super) type Sensors<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        SensorIdOf,
        Blake2_128Concat,
        SensorType,
        SensorData,
        OptionQuery,
    >;

    /// Authorities allowed to submit the price.
    #[pallet::storage]
    #[pallet::getter(fn authorities)]
    pub(super) type Authorities<T: Config> =
        StorageValue<_, BoundedVec<T::AccountId, T::MaxAuthorities>, ValueQuery>;

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn offchain_worker(block_number: BlockNumberFor<T>) {
            match Self::get_sensors_data() {
                Ok(_) => log::info!("Sensors data updated..."),
                Err(_) => log::error!("Failed to update sensors data..."),
            }
        }
    }

    /// Container for different types that implement [`DefaultConfig`]` of this pallet.
    pub mod config_preludes {
        // This will help use not need to disambiguate anything when using `derive_impl`.
        use super::*;

        /// A type providing default configurations for this pallet in testing environment.
        pub struct TestDefaultConfig;

        #[frame_support::register_default_impl(TestDefaultConfig)]
        impl DefaultConfig for TestDefaultConfig {
            type MaxAuthorities = frame_support::traits::ConstU32<64>;
        }
    }
}

impl<T: Config> Pallet<T> {
    pub fn is_authority(who: &T::AccountId) -> bool {
        <Authorities<T>>::get().contains(who)
    }

    /// Fetch current price and return the result in cents.
    pub fn get_sensors_data() -> Result<Vec<SensorData>, http::Error> {
        // We want to keep the offchain worker execution time reasonable, so we set a hard-coded
        // deadline to 2s to complete the external call. You can also wait indefinitely for the
        // response, however you may still get a timeout coming from the host machine.
        let deadline = sp_io::offchain::timestamp().add(Duration::from_millis(2_000));
        // Initiate an external HTTP GET request. This is using high-level wrappers from
        // `sp_runtime`, for the low-level calls that you can find in `sp_io`. The API is trying to
        // be similar to `request`, but since we are running in a custom WASM execution environment
        // we can't simply import the library here.
        let request = http::Request::get("https://sensors-api.vercel.app/api/v1/sensors");
        // We set the deadline for sending of the request, note that awaiting response can have a
        // separate deadline. Next we send the request, before that it's also possible to alter
        // request headers or stream body content in case of non-GET requests.
        let pending = request
            .deadline(deadline)
            .send()
            .map_err(|_| http::Error::IoError)?;

        // The request is already being processed by the host, we are free to do anything else in
        // the worker (we can send multiple concurrent requests too). At some point however we
        // probably want to check the response though, so we can block current thread and wait for
        // it to finish. Note that since the request is being driven by the host, we don't have to
        // wait for the request to have it complete, we will just not read the response.
        let response = pending
            .try_wait(deadline)
            .map_err(|_| http::Error::DeadlineReached)??;

        // Let's check the status code before we proceed to reading the response.
        if response.code != 200 {
            log::warn!("Unexpected status code: {}", response.code);
            return Err(http::Error::Unknown);
        }

        // Next we want to fully read the response body and collect it to a vector of bytes. Note
        // that the return object allows you to read the body in chunks as well with a way to
        // control the deadline.
        let body = response.body().collect::<Vec<u8>>();

        let sensors_data: Vec<SensorData> = serde_json::from_slice(&body).map_err(|_| {
            log::warn!("No sensors data found");
            http::Error::Unknown
        })?;

        let signer = Signer::<T, T::AuthorityId>::any_account();

        signer
            .send_signed_transaction(|account| {
                log::info!("Account, {:?}, {:?}", account.id, account.public);
                Call::<T>::update_sensors_data {
                    updated_data: sensors_data.clone(),
                }
            })
            .ok_or(http::Error::DeadlineReached)?
            .1
            .map_err(|_| http::Error::Unknown)?;

        Ok(sensors_data)
    }

    pub fn add_sensor_data(sensor: SensorData) {
        let id = sensor.id;
        let type_ = sensor.type_;
        <Sensors<T>>::insert(id, type_, sensor);
        Self::deposit_event(Event::SensorDataAdded { id, type_ })
    }
}
