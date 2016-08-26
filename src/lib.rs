use std::thread;

use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{channel, Sender, Receiver};

// TODO: Box<FnOnce>
type Task = Box<FnMut() + Send>;

struct Stats {
    threads: AtomicUsize,
    highwater: AtomicUsize,
}

impl Stats {
    fn new() -> Stats {
        Stats {
            threads: AtomicUsize::new(1),
            highwater: AtomicUsize::new(1),
        }
    }
}

struct ThreadGuard<'a> {
    chan: &'a Arc<Mutex<Receiver<Task>>>,
    stats: &'a Arc<Stats>,
    active: bool,
}

impl<'a> ThreadGuard<'a> {
    fn new(chan: &'a Arc<Mutex<Receiver<Task>>>, stats: &'a Arc<Stats>) -> ThreadGuard<'a> {
        ThreadGuard {
            chan: chan,
            stats: stats,
            active: true,
        }
    }

    fn release(&mut self) {
        self.active = false;
    }
}

impl<'a> Drop for ThreadGuard<'a> {
    fn drop(&mut self) {
        if self.active {
            self.stats.threads.fetch_sub(1, Ordering::SeqCst);
            ThreadPool::add(self.chan.clone(), self.stats.clone());
        }
    }
}

pub struct ThreadPool {
    chan: Sender<Task>,
    tasks: Arc<Mutex<Receiver<Task>>>,
    stats: Arc<Stats>,
}

impl ThreadPool {
    pub fn new() -> ThreadPool {
        let (send, recv) = channel();
        let recv = Arc::new(Mutex::new(recv));
        let sx = Arc::new(Stats::new());

        ThreadPool::add(recv.clone(), sx.clone());

        ThreadPool {
            chan: send,
            tasks: recv,
            stats: sx,
        }
    }

    // Execute task, blocking until a thread is available
    pub fn exec(&self, task: Task) {

    }

    //pub fn spawn() {}

    fn add(chan: Arc<Mutex<Receiver<Task>>>, stats: Arc<Stats>) {

        thread::spawn(move || {
            let mut guard = ThreadGuard::new(&chan, &stats);

            loop {
                let chan = chan.lock().unwrap();
                let message = chan.recv();

                let mut task = match message {
                    Ok(task) => task,
                    Err(..) => break,
                };

                stats.threads.fetch_add(1, Ordering::SeqCst);
                task();
                stats.threads.fetch_sub(1, Ordering::SeqCst);
            }

            guard.release();
        });
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
    }
}
