use std::fmt::Debug;

use serde::Deserialize;

#[derive(Clone, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(transparent)]
pub struct Secret<T>(pub T);

impl<T> Debug for Secret<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Secret")
    }
}

impl<T> From<T> for Secret<T> {
    fn from(value: T) -> Self {
        Self(value)
    }
}
