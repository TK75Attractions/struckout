#![allow(dead_code)]

use i_slint_core::model::{ModelChangeListener, ModelChangeListenerContainer};
use slint::{ModelNotify, ModelTracker};
use std::{cell::RefCell, fmt::Debug, pin::Pin, rc::Rc};

/// Observable data.
///
/// # Example
/// ```
/// # use std::rc::Rc;
/// # use std::cell::Cell;
/// use tsukuyomidmx::Observable;
///
/// let count = Observable::new(0);
/// let double = Rc::new(Cell::new(0));
/// count.subscribe({
///     let double = Rc::clone(&double);
///     move |c| double.set(c * 2)
/// });
///
/// count.set(2);
/// assert_eq!(double.get(), 4);
///
/// count.update(|v| v + 3);
/// assert_eq!(double.get(), 10);
/// ```
#[derive(Debug)]
pub struct State<T: 'static + Debug>(Rc<RefCell<StateInner<T>>>);

#[derive(derive_more::Debug)]
struct StateInner<T: 'static + Debug> {
    data: T,
    #[debug(skip)]
    peer_containers: Vec<Pin<Box<ModelChangeListenerContainer<Peer<T>>>>>,
    // TODO: slintのシステムを使う必要あるか？
    #[debug(skip)]
    notify: ModelNotify,
}

impl<T> State<T>
where
    T: Debug + 'static,
{
    pub fn new(data: T) -> Self {
        Self(Rc::new(RefCell::new(StateInner {
            data: data,
            peer_containers: Vec::new(),
            notify: ModelNotify::default(),
        })))
    }

    /// Do something with current value. If `T` implements `Copy`,
    /// use [`Observable::get()`] instead.
    pub fn with<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&T) -> R,
    {
        f(&self.0.borrow().data)
    }

    /// Sets the new value and notifies to subscribers.
    pub fn set(&self, val: T) {
        self.0.borrow_mut().data = val;
        self.0.borrow().notify.row_changed(0);
    }

    /// Updates value based on current value and notifies to subscribers.
    pub fn update<F>(&self, f: F)
    where
        F: FnOnce(&T) -> T,
    {
        let new = f(&self.0.borrow().data);
        self.0.borrow_mut().data = new;
        self.0.borrow().notify.row_changed(0);
    }

    pub fn subscribe<F>(&self, f: F)
    where
        F: FnMut(&T) + 'static,
    {
        let container = Box::pin(ModelChangeListenerContainer::new(Peer {
            val: Self(Rc::clone(&self.0)),
            f: RefCell::new(Box::new(f)),
        }));

        self.0
            .borrow()
            .notify
            .attach_peer(container.as_ref().model_peer());
        self.0.borrow_mut().peer_containers.push(container);
    }
}

impl<T> State<T>
where
    T: 'static + Copy + Debug,
{
    pub fn get(&self) -> T {
        self.0.borrow().data
    }
}

impl<T: Debug> Clone for State<T> {
    /// This is cheap clone (same as [`Rc::clone()`][std::rc::Rc])
    fn clone(&self) -> Self {
        Self(Rc::clone(&self.0))
    }
}

// TODO: こういうDebug実装をしてデバッグの役に立つか怪しい
#[derive(derive_more::Debug)]
struct Peer<T: 'static + Debug> {
    val: State<T>,
    #[debug(skip)]
    f: RefCell<Box<dyn FnMut(&T) + 'static>>,
}

impl<T> ModelChangeListener for Peer<T>
where
    T: Debug + 'static,
{
    fn row_added(self: Pin<&Self>, _index: usize, _count: usize) {
        unimplemented!("row_added() is never called");
    }

    fn row_changed(self: Pin<&Self>, row: usize) {
        debug_assert_eq!(0, row);
        tracing::trace!(?self, row, "row change was notified");
        (self.f.borrow_mut())(&self.val.0.borrow().data)
    }

    fn row_removed(self: Pin<&Self>, _index: usize, _count: usize) {
        unimplemented!("row_removed() is never called");
    }

    fn reset(self: Pin<&Self>) {
        unimplemented!("reset() is never called");
    }
}
