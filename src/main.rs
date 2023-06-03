#[macro_use]
mod bank;
mod runtime;

use std::ops::Deref;
use std::ops::DerefMut;
use std::pin::pin;

async fn foobar() {
    bank::scope(|bank| async move {
        let a = vault!(vec![1, 2, 3]);
        let x = vault!(0);

        let t1 = {
            let a = a.loan(&bank);
            runtime::spawn(async move {
                let a = a.deref();
                // We can borrow `a` here.
                println!("hello from the first scoped task: {:?}", a);
            })
        };

        let t2 = {
            let a = a.loan(&bank);
            let mut x = x.loan_mut(&bank);
            runtime::spawn(async move {
                let a = a.deref();
                let x = x.deref_mut();

                println!("hello from the second scoped task");
                // We can even mutably borrow `x` here,
                // because no other tasks are using it.
                *x += a[0] + a[2];
            })
        };

        // I'm actually cheating and using threads.
        // I don't know how to write an executor =(
        // The basic principle should generalize.
        t1.join().unwrap();
        t2.join().unwrap();

        // Adding at least one actual await point,
        // otherwise the magic doesn't work.
        async {}.await;
    })
    .await;
}

fn main() {
    //let main_task = runtime::spawn(foobar());
    //main_task.join().unwrap();

    let main_task = pin!(foobar());
    futures::executor::block_on(main_task);
}
