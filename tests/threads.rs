use std::future::Future;
use std::ops::Deref;
use std::ops::DerefMut;
use std::pin::pin;
use std::thread::JoinHandle;

use scoped_tasks_prototype::{scope, vault};

// Writing an example with threads, because miri can't run
// the tokio example.

pub fn spawn<F>(future: F) -> JoinHandle<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    // I'm actually cheating and using threads.
    // I don't know how to write an executor =(
    // The basic principle should generalize.
    std::thread::spawn(move || futures::executor::block_on(future))
}

async fn foobar() {
    scope(|bank| async move {
        let a = vault!(vec![1, 2, 3]);
        let x = vault!(0);

        let t1 = {
            let a = a.loan(&bank);
            spawn(async move {
                let a = a.deref();
                // We can borrow `a` here.
                println!("hello from the first scoped task: {:?}", a);
            })
        };

        let t2 = {
            let a = a.loan(&bank);
            let mut x = x.loan_mut(&bank);
            spawn(async move {
                let a = a.deref();
                let x = x.deref_mut();

                println!("hello from the second scoped task");
                // We can even mutably borrow `x` here,
                // because no other tasks are using it.
                *x += a[0] + a[2];
            })
        };

        t1.join().unwrap();
        t2.join().unwrap();

        // Adding at least one actual await point,
        // otherwise the magic doesn't work.
        async {}.await;
    })
    .await;
}

fn main() {
    let main_task = pin!(foobar());
    futures::executor::block_on(main_task);
}
