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