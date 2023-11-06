use frame_support::pallet_macros::*;

#[pallet_section]
mod config {

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
}
