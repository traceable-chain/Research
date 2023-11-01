// This file is part of Substrate.

// Copyright (C) Parity Technologies (UK) Ltd. SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License"); you may not use this file except
// in compliance with the License. You may obtain a copy of the License at
//
//  http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software distributed under the License
// is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express
// or implied. See the License for the specific language governing permissions and limitations under
// the License.

//! # Price Oracle Offchain Worker Example Pallet
//!
//! The Price Oracle Offchain Worker Example: A simple pallet demonstrating concepts, APIs and
//! structures common to most offchain workers.
//!
//! Run `cargo doc --package pallet-example-offchain-worker-price-oracle --open` to view this
//! module's documentation.
//!
//! **This pallet serves as an example showcasing Substrate off-chain worker and is not meant to be
//! used in production.**
//!
//! ## Overview
//!
//! In this example we are going to build a very simplistic, naive and definitely NOT
//! production-ready oracle for BTC/USD price. The main goal is to showcase how to use off-chain
//! workers to fetch data from external sources via HTTP and feed it back on-chain.
//!
//! The OCW will be triggered after every block, fetch the current price and prepare either signed
//! or unsigned transaction to feed the result back on chain. The on-chain logic will simply
//! aggregate the results and store last `64` values to compute the average price.
//!
//! Only authorized keys are allowed to submit the price. The authorization key should be rotated.
//!
//! Here's an example of how a node admin can inject some keys into the keystore:
//!
//! ```bash
//! $ curl --location --request POST 'http://localhost:9944' \
//! --header 'Content-Type: application/json' \
//! --data-raw '{
//!     "jsonrpc": "2.0",
//!     "method": "author_insertKey",
//!     "params": [
//!	      "btc!",
//!       "bread tongue spell stadium clean grief coin rent spend total practice document",
//!       "0xb6a8b4b6bf796991065035093d3265e314c3fe89e75ccb623985e57b0c2e0c30"
//!     ],
//!     "id": 1
//! }'
//! ```
//!
//! Then make sure that the corresponding address
//! (`5GCCgshTQCfGkXy6kAkFDW1TZXAdsbCNZJ9Uz2c7ViBnwcVg`) has funds and is added to `Authorities` in
//! the runtime by adding it via `add_authority` extrinsic (from `root`).
//!
//! More complex management models and session based key rotations should be considered, but that’s
//! outside the scope of this example.

#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::traits::Get;
use frame_system::{
    self as system,
    offchain::{AppCrypto, CreateSignedTransaction, SendSignedTransaction, Signer},
};
use lite_json::json::JsonValue;
use sp_core::crypto::KeyTypeId;
use sp_runtime::offchain::{
    http,
    storage::{MutateStorageError, StorageRetrievalError, StorageValueRef},
    Duration,
};
use sp_std::vec::Vec;

use serde::{Deserialize, Deserializer, Serialize};

#[cfg(feature = "std")]
use serde::Serializer;

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

#[frame_support::pallet(dev_mode)]
pub mod pallet {
    use super::*;
    use core::fmt;
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;
    use serde::de::Error as SerdeError;
    use serde_json::error;
    /// This pallet's configuration trait
    #[pallet::config(with_default)]
    pub trait Config: CreateSignedTransaction<Call<Self>> + frame_system::Config {
        /// The identifier type for an offchain worker.
        #[pallet::no_default]
        type AuthorityId: AppCrypto<Self::Public, Self::Signature>;

        /// The overarching event type.
        #[pallet::no_default]
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// A grace period after we send transaction.
        ///
        /// To avoid sending too many transactions, we only attempt to send one every `GRACE_PERIOD`
        /// blocks. We use Local Storage to coordinate sending between distinct runs of this
        /// offchain worker.
        #[pallet::no_default]
        #[pallet::constant]
        type GracePeriod: Get<BlockNumberFor<Self>>;

        /// Maximum number of prices.
        #[pallet::constant]
        type MaxPrices: Get<u32>;

        /// Maximum number of authorities.
        #[pallet::constant]
        type MaxAuthorities: Get<u32>;
    }

    /// Events for the pallet.
    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Event generated when new price is accepted to contribute to the average.
        NewPrice {
            price: u32,
            maybe_who: Option<T::AccountId>,
        },
        /// Event generated when a new authority is added.
        AuthorityAdded { authority: T::AccountId },
        /// Event generated when an authority is removed.
        AuthorityRemoved { authority: T::AccountId },
    }

    /// A vector of recently submitted prices.
    ///
    /// This is used to calculate average price, should have bounded size.
    #[pallet::storage]
    #[pallet::getter(fn prices)]
    pub(super) type Prices<T: Config> = StorageValue<_, BoundedVec<u32, T::MaxPrices>, ValueQuery>;

    pub(super) type SensorIdOf = u32;

    #[derive(
        Clone,
        Copy,
        Encode,
        Decode,
        Eq,
        PartialEq,
        RuntimeDebug,
        MaxEncodedLen,
        TypeInfo,
        Serialize,
        Deserialize,
    )]
    pub enum SensorType {
        Humidity = 0,
        Temperature = 1,
        Pressure = 2,
        Digital = 3,
    }

    #[derive(
        Clone,
        Copy,
        Encode,
        Decode,
        Eq,
        PartialEq,
        RuntimeDebug,
        MaxEncodedLen,
        TypeInfo,
        Serialize,
        Deserialize,
    )]
    pub struct Geolocation {
        pub lat: u32,
        pub lon: u32,
    }

    #[derive(
        Clone,
        Copy,
        Encode,
        Decode,
        Eq,
        PartialEq,
        RuntimeDebug,
        MaxEncodedLen,
        TypeInfo,
        Serialize,
        Deserialize,
    )]
    pub enum SensorValue {
        Number(u32),
        Bool(bool),
    }

    fn de_string_to_sensor_type<'de, D>(de: D) -> Result<SensorType, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s: &str = Deserialize::deserialize(de)?;
        match s {
            "Humidity" => Ok(SensorType::Humidity),
            "Pressure" => Ok(SensorType::Pressure),
            "Temperature" => Ok(SensorType::Temperature),
            "Digital" => Ok(SensorType::Digital),
            _ => Err(D::Error::custom("Unexpected sensor type")),
        }
    }

    fn de_string_to_geolocation<'de, D>(de: D) -> Result<Geolocation, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s: Geolocation = Deserialize::deserialize(de)?;
        // let geolocation: Geolocation = serde_json::from_slice(&s).map_err(|e| {
        // 	log::info!("\n\n\n ERROR: {:?}\n\n\n", e);
        // 	Ok(Geolocation {lat: 0, lon: 0})
        // })?;
        // geolocation
        Ok(s)
    }

    fn de_string_to_sensor_value<'de, D>(de: D) -> Result<SensorValue, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s: &str = Deserialize::deserialize(de)?;
        match s {
            "true" => Ok(SensorValue::Bool(true)),
            "false" => Ok(SensorValue::Bool(false)),
            value => match value.parse::<u32>() {
                Ok(x) => Ok(SensorValue::Number(x)),
                // TODO: check later if this should be zero or not
                _ => Ok(SensorValue::Number(0)),
            },
            _ => Err(D::Error::custom("Unexpected sensor value")),
        }
    }

    #[derive(
        Clone,
        Copy,
        Encode,
        Decode,
        Eq,
        PartialEq,
        RuntimeDebug,
        MaxEncodedLen,
        TypeInfo,
        Serialize,
        Deserialize,
    )]
    pub struct SensorData {
        pub id: u32,
        #[serde(deserialize_with = "de_string_to_sensor_type")]
        pub type_: SensorType,
        #[serde(deserialize_with = "de_string_to_geolocation")]
        pub geolocation: Geolocation,
        #[serde(deserialize_with = "de_string_to_sensor_value")]
        pub value: SensorValue,
        pub timestamp: u64,
    }

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

    #[pallet::error]
    pub enum Error<T> {
        NotAuthority,
        AlreadyAuthority,
        TooManyAuthorities,
        DeserializeError,
        FailedSignedTransaction,
    }

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

    /// Container for different types that implement [`DefaultConfig`]` of this pallet.
    pub mod config_preludes {
        // This will help use not need to disambiguate anything when using `derive_impl`.
        use super::*;

        /// A type providing default configurations for this pallet in testing environment.
        pub struct TestDefaultConfig;

        #[frame_support::register_default_impl(TestDefaultConfig)]
        impl DefaultConfig for TestDefaultConfig {
            type MaxPrices = frame_support::traits::ConstU32<64>;
            type MaxAuthorities = frame_support::traits::ConstU32<64>;
        }
    }
}

impl<T: Config> Pallet<T> {
    fn is_authority(who: &T::AccountId) -> bool {
        <Authorities<T>>::get().contains(who)
    }

    /// Fetch current price and return the result in cents.
    fn get_sensors_data() -> Result<Vec<SensorData>, http::Error> {
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

        // Create a str slice from the body.
        let body_str = sp_std::str::from_utf8(&body).map_err(|_| {
            log::warn!("No UTF8 body");
            http::Error::Unknown
        })?;

        log::info!("\n\n\n\nBody: {:?}\n\n\n\n", body_str);

        let sensors_data: Vec<SensorData> = serde_json::from_slice(&body).map_err(|e| {
            log::info!("\n\n\n ERROR: {:?}\n\n\n", e);
            http::Error::DeadlineReached
        })?;

        log::info!("Sensors Data: {:?}", sensors_data.clone());

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
            .map_err(|_| http::Error::DeadlineReached)?;

        Ok(sensors_data)
    }

    fn add_sensor_data(sensor: SensorData) {
        let id = sensor.id;
        let type_ = sensor.type_;
        <Sensors<T>>::insert(id, type_, sensor)
    }
}
