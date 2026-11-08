//! No decoder yet on this platform: 6.9e3b and 6.9e3c bring them.

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
