// Extremely basic runtime, courtesy of ChatGPT4

use std::future::Future;
use std::thread::JoinHandle;

pub fn spawn<F>(future: F) -> JoinHandle<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    std::thread::spawn(move || futures::executor::block_on(future))
}
