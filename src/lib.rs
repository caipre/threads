use std::thread::{self, JoinHandle};

use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{channel, Sender, Receiver};

// TODO: Box<FnOnce>
type Task = Box<FnMut() + Send + 'static>;

#[derive(Debug)]
struct Stats {
    threads: AtomicUsize,
    highwater: AtomicUsize,
}

impl Stats {
    fn new() -> Stats {
        Stats {
            threads: AtomicUsize::new(0),
            highwater: AtomicUsize::new(0),
        }
    }
}

struct ThreadGuard<'a> {
    chan: &'a Arc<Mutex<Receiver<Task>>>,
    stats: &'a Arc<Stats>,
    cap: &'a Arc<AtomicUsize>,
    active: bool,
}

impl<'a> ThreadGuard<'a> {
    fn new(chan: &'a Arc<Mutex<Receiver<Task>>>,
           stats: &'a Arc<Stats>,
           cap: &'a Arc<AtomicUsize>) -> ThreadGuard<'a> {
        ThreadGuard {
            chan: chan,
            stats: stats,
            cap: cap,
            active: true,
        }
    }

    fn release(mut self) {
        self.active = false;
    }
}

impl<'a> Drop for ThreadGuard<'a> {
    fn drop(&mut self) {
        if self.active {
            self.stats.threads.fetch_sub(1, Ordering::SeqCst);
            ThreadPool::worker(self.chan.clone(), self.stats.clone(), self.cap.clone());
        }
    }
}

#[derive(Debug)]
pub struct ThreadPool {
    chan: Sender<Task>,
    tasks: Arc<Mutex<Receiver<Task>>>,
    stats: Arc<Stats>,
    cap: Arc<AtomicUsize>,
}

impl ThreadPool {
    pub fn new() -> ThreadPool {
        let (send, recv) = channel();
        let recv = Arc::new(Mutex::new(recv));
        let stats = Arc::new(Stats::new());
        let cap = Arc::new(AtomicUsize::new(1));

        ThreadPool::worker(recv.clone(), stats.clone(), cap.clone());

        ThreadPool {
            chan: send,
            tasks: recv,
            stats: stats,
            cap: cap,
        }
    }

    pub fn with_capacity(n: usize) -> ThreadPool {
        let mut pool = ThreadPool::new();
        pool.cap = Arc::new(AtomicUsize::new(n));
        for _ in 1..n {
            ThreadPool::worker(pool.tasks.clone(), pool.stats.clone(), pool.cap.clone());
        }
        pool
    }

    pub fn len(&self) -> usize {
        self.stats.threads.load(Ordering::Relaxed)
    }

    pub fn resize(mut self, n: usize) -> ThreadPool {
        self.cap = Arc::new(AtomicUsize::new(n));
        self
    }

    pub fn exec<F>(&self, task: F)
        where F: FnMut() + Send + 'static
    {
        self.chan.send(Box::new(move || task()));
    }

    fn worker(chan: Arc<Mutex<Receiver<Task>>>, stats: Arc<Stats>, cap: Arc<AtomicUsize>) -> JoinHandle<()> {
        thread::spawn(move || {
            let guard = ThreadGuard::new(&chan, &stats, &cap);
            stats.threads.fetch_add(1, Ordering::SeqCst);

            loop {
                if cap.load(Ordering::SeqCst) < stats.threads.load(Ordering::SeqCst) { break; }

                let message = {
                    let chan = chan.lock().unwrap();
                    chan.recv()
                };

                let mut task = match message {
                    Ok(task) => task,
                    Err(..) => break,
                };

                task();
            }

            stats.threads.fetch_sub(1, Ordering::SeqCst);
            guard.release();
        })
    }
}

mod test {
    use std::sync::mpsc::channel;
    use super::ThreadPool;

    const NWORKERS: usize = 10;

    #[test]
    fn test_worker_creation() {
        let pool = ThreadPool::with_capacity(NWORKERS);
        assert_eq!(pool.len(), NWORKERS);
    }

    #[test]
    fn test_task_completion() {
        let pool = ThreadPool::with_capacity(NWORKERS);
        let (send, recv) = channel();
        for _ in 0..NWORKERS {
            let send = send.clone();
            pool.exec(move || send.send(1).unwrap() );
        }
        assert_eq!(recv.recv().iter().take(NWORKERS).fold(0, |a, b| a + b), NWORKERS);
    }
}
