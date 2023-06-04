use std::ops::Deref;
use std::ops::DerefMut;

use scoped_tasks_prototype::{scope, vault};

async fn foobar() {
    scope(|bank| async move {
        let a = vault!(vec![1, 2, 3]);
        let x = vault!(0);

        let t1 = {
            let a = a.loan(&bank);
            tokio::spawn(async move {
                let a = a.deref();
                // We can borrow `a` here.
                println!("hello from the first scoped task: {:?}", a);
            })
        };

        let t2 = {
            let a = a.loan(&bank);
            let mut x = x.loan_mut(&bank);
            tokio::spawn(async move {
                let a = a.deref();
                let x = x.deref_mut();

                println!("hello from the second scoped task");
                // We can even mutably borrow `x` here,
                // because no other tasks are using it.
                *x += a[0] + a[2];
            })
        };

        t1.await.unwrap();
        t2.await.unwrap();
    })
    .await;
}

#[tokio::main]
async fn main() {
    let main_task = tokio::spawn(foobar());
    main_task.await.unwrap();
}
