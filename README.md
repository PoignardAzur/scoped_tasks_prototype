# scoped_tasks_prototype

A quick-and-dirty attempt to get scoped tasks in Rust.

This library tries to provide an interface similar to
[scoped threads](https://doc.rust-lang.org/stable/std/thread/fn.scope.html),
accessible from tasks. See
[Tyler Mandry's article for why this is non-trivial](https://tmandry.gitlab.io/blog/posts/2023-03-01-scoped-tasks/).
This crate explores the "Restricting Borrowing" option described in that post.

To be specific, this crate creates three concepts:

- A **Bank**, where your task is stored.
- **Vaults**, located inside the Bank.
- **Loans** which point to contents of the Vaults.

When you want to run scoped tasks, you call the `scope` function:

```rust
scope(|bank| async move {
    // code that will spawn tasks
})
```

This function accepts a callback which returns a promise (alas, no async
closures yet); that callback will be given a `Bank`, which represents _the
underlying memory the promise is stored in_.

Inside the callback, you can use the `vault!` macro to pin values to that
underlying storage (it's literally a slightly modified version of the standard
`pin!` macro):

```rust
let a = vault!(vec![1, 2, 3]);
let x = vault!(0);
```

Finally, you can use that vault and the bank to get loans. Loans are `'static`
values storing a reference to a vault, _and_ a shared reference to the bank. The
bank can't possibly be dropped until all loans are dropped, even if the parent
task is dropped, which means that a loan is always safe to deref:

```rust
let a = a.loan(&bank);
let mut x = x.loan_mut(&bank);
tokio::spawn(async move {
    let a = a.deref();
    let x = x.deref_mut();
    *x += a[0] + a[2];
})
```

Taken together, this means it's possible with some overhead to run scoped tasks
with a syntax similar to scoped threads:

```rust
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
```

## How safe is this?

:shrug_emoji:

This is very much at the proof-of-concept stage.

So far I've only tried to run examples in the repo, including the threads
example with MIRI, which reported no error. It does not immediately blow up my
computer, which is honestly better than I expected.

If people are interested, I would strongly encourage them to poke at this at the
seams and see if any parts of the crate are unsound. I have virtually no
experience whatsoever with unsafe, so I'm curious if I missed something.

More importantly, I'm hoping this serves as inspiration to the writers of async
runtimes for including similar concepts in their crates. Scoped tasks aren't
impossible, there's just a lot of design space we need to explore before they
can really become convenient to use. This is just one very early attempt.
