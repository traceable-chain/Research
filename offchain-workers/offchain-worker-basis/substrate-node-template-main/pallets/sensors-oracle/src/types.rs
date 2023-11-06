use codec::{Decode, Encode, MaxEncodedLen};
use frame_system::{self as system, pallet_prelude::BlockNumberFor, Config};
use frame_support::sp_runtime::RuntimeDebug;
use scale_info::TypeInfo;
use serde::de::Error as SerdeError;
use serde::{Deserialize, Deserializer, Serialize};

#[cfg(feature = "std")]
use serde::Serializer;

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
        _ => Err(SerdeError::custom("Error decoding sensor type.")),
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
        _ => Err(SerdeError::custom("Error decoding sensor value.")),
    }
}
