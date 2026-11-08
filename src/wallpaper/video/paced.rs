//! A decoder thread for frameworks that hand out frames on demand: paced by frame times,
//! paused on request, restarted to loop or to refit.

use std::{
    sync::{Arc, Condvar, Mutex},
    time::{Duration, Instant},
};

use anyhow::Result;

use super::{Event, Fit, Frame};

/// One platform's reader: frames from `from` seconds, at `size` or the source size.
pub trait Decoder {
    fn start(&mut self, size: Option<(u32, u32)>, from: f64) -> Result<()>;
    /// The next frame and its time in seconds, or `None` at the end.
    fn next(&mut self) -> Result<Option<(f64, image::RgbaImage)>>;
}

struct State {
    playing: bool,
    fit: Fit,
    refit: bool,
    stop: bool,
}

type Control = Arc<(Mutex<State>, Condvar)>;

pub struct Player {
    control: Control,
    thread: Option<std::thread::JoinHandle<()>>,
}

fn run(decoder: &mut impl Decoder, control: &Control, send: &dyn Fn(Event)) -> Result<()> {
    let (mut native, mut from) = (None, 0.0);
    loop {
        let fit = control.0.lock().expect("state lock").fit;
        decoder.start(native.map(|n| fit.scaled(n)), from)?;
        let mut clock: Option<(Instant, f64)> = None;
        loop {
            let Some((at, image)) = decoder.next()? else {
                from = 0.0;
                break;
            };
            let dims = image.dimensions();
            let first = native.is_none();
            let source = *native.get_or_insert(dims);
            let image = if fit.sigma > 0.0 {
                image::imageops::fast_blur(&image, fit.frame_sigma(dims.0, source))
            } else {
                image
            };
            let (state, wake) = &**control;
            let mut state = state.lock().expect("state lock");
            let (start, zero) = *clock.get_or_insert((Instant::now(), at));
            let due = start + Duration::from_secs_f64((at - zero).max(0.0));
            while !state.stop && !state.refit && state.playing && Instant::now() < due {
                state = wake
                    .wait_timeout(state, due - Instant::now())
                    .expect("state lock")
                    .0;
            }
            if state.stop {
                return Ok(());
            }
            send(Event::Frame(Frame(image)));
            let paused = !state.playing;
            while !state.stop && !state.refit && !state.playing {
                state = wake.wait(state).expect("state lock");
            }
            if state.stop {
                return Ok(());
            }
            if paused {
                clock = Some((Instant::now(), at));
            }
            if first || std::mem::take(&mut state.refit) {
                from = at;
                break;
            }
        }
    }
}

impl Player {
    /// `make` runs on the decoder thread, which owns every object the framework hands out.
    pub fn spawn<D: Decoder>(
        fit: Fit,
        send: impl Fn(Event) + Send + Sync + 'static,
        make: impl FnOnce() -> Result<D> + Send + 'static,
    ) -> Self {
        let state = State {
            playing: false,
            fit,
            refit: false,
            stop: false,
        };
        let control: Control = Arc::new((Mutex::new(state), Condvar::new()));
        let shared = control.clone();
        let thread = std::thread::spawn(move || {
            let result = make().and_then(|mut decoder| run(&mut decoder, &shared, &send));
            if let Err(error) = result {
                send(Event::Failed(format!("{error:#}")));
            }
        });
        Self {
            control,
            thread: Some(thread),
        }
    }

    pub fn play(&self, playing: bool) -> Result<()> {
        self.control.0.lock().expect("state lock").playing = playing;
        self.control.1.notify_all();
        Ok(())
    }

    pub fn fit(&self, fit: Fit) {
        let mut state = self.control.0.lock().expect("state lock");
        if state.fit != fit {
            state.fit = fit;
            state.refit = true;
            self.control.1.notify_all();
        }
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        self.control.0.lock().expect("state lock").stop = true;
        self.control.1.notify_all();
        if let Some(thread) = self.thread.take() {
            thread.join().expect("decoder thread panicked");
        }
    }
}
