//! No decoder yet on Windows: 6.9e3c brings it.

use anyhow::{Result, bail};

use super::{Event, Fit};

pub struct Player;

impl Player {
    pub fn open(_: &[u8], _: Fit, _: impl Fn(Event) + Send + Sync + 'static) -> Result<Self> {
        bail!("no video decoder on this platform")
    }

    pub fn play(&self, _: bool) -> Result<()> {
        unreachable!("never opened")
    }

    pub fn fit(&self, _: Fit) {
        unreachable!("never opened")
    }
}
