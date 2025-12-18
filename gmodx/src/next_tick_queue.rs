use crate::lua::{self, State};
use std::sync::{Arc, Mutex, mpsc};

type Callback = Box<dyn FnOnce(&lua::State) + Send>;

#[derive(Clone)]
pub struct NextTickQueue {
    tx: mpsc::Sender<Callback>,
    rx: Arc<Mutex<mpsc::Receiver<Callback>>>,
}

impl NextTickQueue {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel::<Callback>();
        let rx = Arc::new(Mutex::new(rx));

        let rx_clone = rx.clone();
        std::thread::spawn(move || {
            loop {
                // Block until something arrives
                let Ok(cb) = rx_clone.lock().unwrap().recv() else {
                    break;
                };

                let Some(l) = lua::lock() else {
                    break;
                };

                cb(&l);
                for cb in rx_clone.lock().unwrap().try_iter().take(19) {
                    cb(&l);
                }
            }
        });

        Self { tx, rx }
    }

    pub fn queue<F>(&self, f: F)
    where
        F: FnOnce(&State) + Send + 'static,
    {
        let _ = self.tx.send(Box::new(f));
    }

    pub fn flush(&self, l: &State) {
        for cb in self.rx.lock().unwrap().try_iter().take(20) {
            cb(l);
        }
    }
}
