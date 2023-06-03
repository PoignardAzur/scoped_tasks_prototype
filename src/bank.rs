use std::any::Any;
use std::cell::UnsafeCell;
use std::future::Future;
use std::mem::MaybeUninit;
use std::pin::Pin;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::sync::Arc;

#[derive(Clone)]
pub struct Bank {
    pub(crate) storage: Arc<dyn Any>,
}

pub struct Vault<'a, T> {
    pub(crate) inner: &'a mut VaultInner<T>,
}

pub(crate) struct VaultInner<T> {
    pub value: T,
    pub loan_count: AtomicUsize,
    pub loan_mut_count: AtomicUsize,
}

pub struct Loan<T> {
    _bank: Arc<dyn Any>,
    value: *const VaultInner<T>,
}

pub struct LoanMut<T> {
    _bank: Arc<dyn Any>,
    value: *mut VaultInner<T>,
}

// ---

macro_rules! vault {
    (
        $value:expr
    ) => {
        &mut $crate::bank::Vault {
            inner: &mut $crate::bank::VaultInner {
                value: $value,
                loan_count: 0.into(),
                loan_mut_count: 0.into(),
            },
        }
    };
}

// ---

impl Bank {
    fn new<T: Any>(storage: Arc<T>) -> Self {
        Self { storage }
    }
}

impl<'a, T> Vault<'a, T> {
    #[cfg(FALSE)]
    #[allow(unused)]
    pub fn new(value: T) -> Self {
        Self {
            value,
            loan_count: 0.into(),
            loan_mut_count: 0.into(),
        }
    }

    fn check_bank(&self, bank: &Bank) {
        // Make sure that current adress is within the payload of the bank,
        // eg that self with stay valid as long as the bank isn't dropped.
        let payload_ref = bank.storage.as_ref() as &dyn Any;
        let payload_start = payload_ref as *const dyn Any as *const u8 as usize;
        let payload_end = payload_start + std::mem::size_of_val(payload_ref);
        let self_start = self as *const Self as *const u8 as usize;
        assert!(payload_start <= self_start);
        assert!(self_start < payload_end);
    }

    pub fn loan(&mut self, bank: &Bank) -> Loan<T> {
        self.check_bank(bank);

        assert_eq!(self.inner.loan_mut_count.load(Ordering::SeqCst), 0);
        self.inner.loan_count.fetch_add(1, Ordering::SeqCst);

        Loan {
            _bank: bank.storage.clone(),
            value: self.inner as *const VaultInner<T>,
        }
    }

    pub fn loan_mut(&mut self, bank: &Bank) -> LoanMut<T> {
        self.check_bank(bank);

        assert_eq!(self.inner.loan_count.load(Ordering::SeqCst), 0);
        assert_eq!(self.inner.loan_mut_count.load(Ordering::SeqCst), 0);
        self.inner.loan_mut_count.fetch_add(1, Ordering::SeqCst);

        LoanMut {
            _bank: bank.storage.clone(),
            value: self.inner as *mut VaultInner<T>,
        }
    }
}

impl<T> std::ops::Deref for Loan<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        // SAFETY: Loan can only be created in such a way that self.value is valid
        // as long as there's at least one copy of the Arc<dyn Any> of BankInner.
        unsafe { &(*self.value).value }
    }
}

impl<T> std::ops::Deref for LoanMut<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        // SAFETY: Loan can only be created in such a way that self.value is valid
        // as long as there's at least one copy of the Arc<dyn Any> of BankInner.
        unsafe { &(*self.value).value }
    }
}

impl<T> std::ops::DerefMut for LoanMut<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        // SAFETY: LoanMut can only be created in such a way that self.value is valid
        // as long as there's at least one copy of the Arc<dyn Any> of BankInner.
        unsafe { &mut (*self.value).value }
    }
}

impl<T> Clone for Loan<T> {
    fn clone(&self) -> Self {
        // SAFETY: Loan can only be created in such a way that self.value is valid
        // as long as there's at least one copy of the Arc<dyn Any> of BankInner.
        unsafe {
            (*self.value).loan_count.fetch_add(1, Ordering::SeqCst);
        }
        Self {
            _bank: self._bank.clone(),
            value: self.value,
        }
    }
}

impl<T> Drop for Loan<T> {
    fn drop(&mut self) {
        // SAFETY: Loan can only be created in such a way that self.value is valid
        // as long as there's at least one copy of the Arc<dyn Any> of BankInner.
        unsafe {
            (*self.value).loan_count.fetch_sub(1, Ordering::SeqCst);
        }
    }
}

impl<T> Drop for LoanMut<T> {
    fn drop(&mut self) {
        // SAFETY: LoanMut can only be created in such a way that self.value is valid
        // as long as there's at least one copy of the Arc<dyn Any> of BankInner.
        unsafe {
            (*self.value).loan_mut_count.fetch_sub(1, Ordering::SeqCst);
        }
    }
}

// SAFETY: I have no idea what I'm doing.
unsafe impl Send for Bank {}
unsafe impl Sync for Bank {}
unsafe impl<T> Send for Loan<T> {}
unsafe impl<T> Sync for Loan<T> {}
unsafe impl<T> Send for LoanMut<T> {}
unsafe impl<T> Sync for LoanMut<T> {}

struct BankBuilder<F> {
    inner: Arc<UnsafeCell<MaybeUninit<F>>>,
}

impl<F: Future + 'static> BankBuilder<F> {
    fn new() -> Self {
        Self {
            inner: Arc::new(UnsafeCell::new(MaybeUninit::uninit())),
        }
    }
}

// SAFETY: Probably fine?
unsafe impl<F> Send for BankBuilder<F> {}
unsafe impl<F> Sync for BankBuilder<F> {}

pub async fn scope<F: Future + 'static>(callback: impl FnOnce(Bank) -> F) -> F::Output {
    let bank_holder = BankBuilder::new();
    let future = callback(Bank::new(bank_holder.inner.clone()));

    // SAFETY: The unsafe cell is shared with the Bank instance given to `callback`,
    // which may share it with Loans. However, neither the Bank nor the Loans can
    // read anything inside the Arc, they only get the pointer range.
    unsafe { bank_holder.inner.get().write(MaybeUninit::new(future)) };

    // SAFETY: Proooobably safe? Oh boy.
    // - Pin::new_unchecked: As stated above, the Bank and Loans can't access the
    // contents of the Arc, so they can't move the future out of it.
    // - UnsafeCell::get: The Bank and Loans can't access the contents of the Arc,
    // so the concents of the cell will only be accessed through Future::poll.
    // - MaybeUninit::assume_init_mut: We used MaybeUninit::new above, and we never
    // change it until it's dropped
    let future = unsafe { Pin::new_unchecked((&mut *bank_holder.inner.get()).assume_init_mut()) };
    future.await
}
