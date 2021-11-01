//! examples/lockall_destruct.rs

#![deny(unsafe_code)]
#![deny(warnings)]
#![no_main]
#![no_std]

use panic_semihosting as _;

#[rtic::app(device = lm3s6965, dispatchers = [GPIOA, GPIOB, GPIOC])]
mod app {
    use cortex_m_semihosting::{debug, hprintln};

    #[shared]
    struct Shared {
        a: u32,
        b: i64,
    }

    #[local]
    struct Local {}

    #[init]
    fn init(_: init::Context) -> (Shared, Local, init::Monotonics) {
        foo::spawn().unwrap();

        (Shared { a: 1, b: 2 }, Local {}, init::Monotonics())
    }

    // when omitted priority is assumed to be `1`
    #[task(shared = [a, b])]
    fn foo(mut c: foo::Context) {
        c.shared.lock(|foo::Shared { a, b }| {
            hprintln!("foo: a = {}, b = {}", a, b).ok();
            **a += 1;
            bar::spawn().unwrap();
            baz::spawn().unwrap();
        });
        debug::exit(debug::EXIT_SUCCESS); // Exit QEMU simulator
    }

    #[task(priority = 2, shared = [a])]
    fn bar(mut c: bar::Context) {
        // the higher priority task does still need a critical section
        let a = c.shared.lock(|bar::Shared { a }| {
            **a += 1;
            // *s.b += 1; `b` not accessible
            **a
        });

        hprintln!("bar: a = {}", a).unwrap();
    }

    #[task(priority = 3)]
    fn baz(_: baz::Context) {
        hprintln!("baz").unwrap();
    }
}