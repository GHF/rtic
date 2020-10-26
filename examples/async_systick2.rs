//! examples/async_task2
#![no_main]
#![no_std]
#![feature(const_fn)]
#![feature(type_alias_impl_trait)]

// use core::cell::Cell;
// use core::cell::UnsafeCell;
use core::future::Future;
use core::mem;
// use core::mem::MaybeUninit;
use core::pin::Pin;
// use core::ptr;
// use core::ptr::NonNull;
// use core::sync::atomic::{AtomicPtr, AtomicU32, Ordering};
use core::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

use cortex_m_semihosting::{debug, hprintln};
use panic_semihosting as _;
use rtic::Mutex;

#[rtic::app(device = lm3s6965, dispatchers = [SSI0])]
mod app {
    use crate::*;

    #[resources]
    struct Resources {
        systick: Systick,
    }

    #[init]
    fn init(cx: init::Context) -> init::LateResources {
        hprintln!("init").ok();
        foo::spawn().unwrap();
        init::LateResources {
            systick: Systick {
                syst: cx.core.SYST,
                state: State::Done,
                queue: BinaryHeap::new(),
                // waker: None,
            },
        }
    }

    #[idle]
    fn idle(_: idle::Context) -> ! {
        // debug::exit(debug::EXIT_SUCCESS);
        loop {
            hprintln!("idle").ok();
            cortex_m::asm::wfi(); // put the MCU in sleep mode until interrupt occurs
        }
    }

    #[task(resources = [systick])]
    fn foo(mut cx: foo::Context) {
        // BEGIN BOILERPLATE
        type F = impl Future + 'static;
        fn create(cx: foo::Context<'static>) -> F {
            task(cx)
        }

        static mut TASK: Task<F> = Task::new();

        hprintln!("foo trampoline").ok();
        unsafe {
            match TASK {
                Task::Idle | Task::Done(_) => {
                    hprintln!("foo spawn task").ok();
                    TASK.spawn(|| create(mem::transmute(cx)));
                }
                _ => {}
            };

            hprintln!("foo trampoline poll").ok();
            TASK.poll(|| {
                let _ = foo::spawn();
            });

            match TASK {
                Task::Done(ref r) => {
                    hprintln!("foo trampoline done").ok();
                    // hprintln!("r = {:?}", mem::transmute::<_, &u32>(r)).ok();
                }
                _ => {
                    hprintln!("foo trampoline running").ok();
                }
            }
        }
        // END BOILERPLATE

        async fn task(mut cx: foo::Context<'static>) {
            hprintln!("foo task").ok();

            hprintln!("delay long time").ok();
            timer_delay(&mut cx.resources.systick, 5000000).await;
            hprintln!("foo task resumed").ok();

            hprintln!("delay short time").ok();
            timer_delay(&mut cx.resources.systick, 1000000).await;
            hprintln!("foo task resumed").ok();
        }
    }

    // RTIC task bound to the HW SysTick interrupt
    #[task(binds = SysTick, resources = [systick], priority = 2)]
    fn systic(mut cx: systic::Context) {
        hprintln!("systic interrupt").ok();
        cx.resources.systick.lock(|s| {
            s.syst.disable_interrupt();
            s.state = State::Done;
            s.queue.pop().map(|w| w.waker.wake());
            if let Some(w) = s.queue.peek() {
                s.syst.set_reload(w.time);
            } else {
                s.syst.disable_interrupt();
            }
        });
    }
}

//=============
// Waker

static WAKER_VTABLE: RawWakerVTable =
    RawWakerVTable::new(waker_clone, waker_wake, waker_wake, waker_drop);

unsafe fn waker_clone(p: *const ()) -> RawWaker {
    RawWaker::new(p, &WAKER_VTABLE)
}

unsafe fn waker_wake(p: *const ()) {
    let f: fn() = mem::transmute(p);
    f();
}

unsafe fn waker_drop(_: *const ()) {
    // nop
}

//============
// Task

enum Task<F: Future + 'static> {
    Idle,
    Running(F),
    Done(F::Output),
}

impl<F: Future + 'static> Task<F> {
    const fn new() -> Self {
        Self::Idle
    }

    fn spawn(&mut self, future: impl FnOnce() -> F) {
        *self = Task::Running(future());
    }

    unsafe fn poll(&mut self, wake: fn()) {
        match self {
            Task::Idle => {}
            Task::Running(future) => {
                let future = Pin::new_unchecked(future);
                let waker_data: *const () = mem::transmute(wake);
                let waker = Waker::from_raw(RawWaker::new(waker_data, &WAKER_VTABLE));
                let mut cx = Context::from_waker(&waker);

                match future.poll(&mut cx) {
                    Poll::Ready(r) => *self = Task::Done(r),
                    Poll::Pending => {}
                };
            }
            Task::Done(_) => {}
        }
    }
}

//=============
// Timer
// Later we want a proper queue

//use core::cmp::{Ord, Ordering, PartialOrd};
use core::cmp::Ordering;
use heapless::binary_heap::{BinaryHeap, Max};
use heapless::consts::U8;

pub enum State {
    Started,
    Done,
}

struct Timeout {
    time: u32,
    waker: Waker,
}

impl Ord for Timeout {
    fn cmp(&self, other: &Self) -> Ordering {
        self.time.cmp(&other.time)
    }
}

impl PartialOrd for Timeout {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(&other))
    }
}

impl PartialEq for Timeout {
    fn eq(&self, other: &Self) -> bool {
        self.time == other.time
    }
}

impl Eq for Timeout {}

pub struct Systick {
    syst: cortex_m::peripheral::SYST,
    state: State,
    queue: BinaryHeap<Timeout, U8, Max>,
    // waker: Option<Waker>,
}

//=============
// Timer
// Later we want a proper queue

pub struct Timer<'a, T: Mutex<T = Systick>> {
    request: Option<u32>,
    systick: &'a mut T,
}

impl<'a, T: Mutex<T = Systick>> Future for Timer<'a, T> {
    type Output = ();
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let Self { request, systick } = &mut *self;
        systick.lock(|s| {
            // enqueue a new request
            request.take().map(|t| {
                s.syst.set_reload(t);
                s.syst.enable_counter();
                s.syst.enable_interrupt();
                s.state = State::Started;
                s.queue.push(Timeout {
                    time: t,
                    waker: cx.waker().clone(),
                });
            });

            match s.state {
                State::Done => Poll::Ready(()),
                State::Started => {
                    // s.waker = Some(cx.waker().clone());
                    Poll::Pending
                }
            }
        })
    }
}

fn timer_delay<'a, T: Mutex<T = Systick>>(systick: &'a mut T, t: u32) -> Timer<'a, T> {
    hprintln!("timer_delay {}", t);
    Timer {
        request: Some(t),
        systick,
    }
}