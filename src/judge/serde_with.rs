use bytesize::ByteSize;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use tokio::time::Duration;

use super::MaybeLimited;
#[allow(dead_code)]
pub mod ser {
    use super::*;

    pub fn duration_to_secs<S>(
        duration: &Duration,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_f64(duration.as_secs_f64())
    }

    pub fn bytesize_to_kib<S>(
        bytesize: &ByteSize,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u64(bytesize.as_kib() as u64)
    }

    pub fn option_bytesize_to_option_kib<S>(
        bytesize: &Option<ByteSize>,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        bytesize
            .as_ref()
            .map(|b| b.as_kib() as u64)
            .serialize(serializer)
    }

    pub fn mb_lim_duration_to_mb_lim_secs<S>(
        value: &MaybeLimited<Duration>,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Если MaybeLimited реализует as_ref() + map() (как Option)
        value.map(|d| d.as_secs_f64()).serialize(serializer)
    }

    pub fn mb_lim_bytesize_to_mb_lim_kib<S>(
        value: &MaybeLimited<ByteSize>,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        value.map(|b| b.as_kib() as u64).serialize(serializer)
    }
}

#[allow(dead_code)]
pub mod de {
    use super::*;

    pub fn duration_from_secs<'de, D>(deserializer: D) -> std::result::Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        f64::deserialize(deserializer).map(Duration::from_secs_f64)
    }

    pub fn bytesize_from_kib<'de, D>(deserializer: D) -> std::result::Result<ByteSize, D::Error>
    where
        D: Deserializer<'de>,
    {
        u64::deserialize(deserializer).map(ByteSize::kib)
    }

    pub fn option_bytesize_from_option_kib<'de, D>(
        deserializer: D,
    ) -> std::result::Result<Option<ByteSize>, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(Option::<u64>::deserialize(deserializer)?.map(ByteSize::kib))
    }

    pub fn mb_lim_duration_from_mb_lim_secs<'de, D>(
        deserializer: D,
    ) -> std::result::Result<MaybeLimited<Duration>, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(MaybeLimited::<f64>::deserialize(deserializer)?.map(Duration::from_secs_f64))
    }

    pub fn mb_lim_bytesize_from_mb_lim_kib<'de, D>(
        deserializer: D,
    ) -> std::result::Result<MaybeLimited<ByteSize>, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(MaybeLimited::<u64>::deserialize(deserializer)?.map(ByteSize::kib))
    }
}
