use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, Serialize, Deserialize, Default)]
pub enum MaybeLimited<T: Copy> {
    Limited(T),
    #[default]
    Unlimited,
}
pub use MaybeLimited::{Limited, Unlimited};

impl<T: Copy> MaybeLimited<T> {
    pub fn map<R: Copy>(self, op: impl FnOnce(T) -> R) -> MaybeLimited<R> {
        if let Limited(x) = self {
            Limited(op(x))
        } else {
            Unlimited
        }
    }
}
