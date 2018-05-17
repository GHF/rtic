#![deny(unsafe_code)]
#![deny(warnings)]
#![feature(proc_macro)]
#![no_main]
#![no_std]

extern crate cortex_m_rtfm as rtfm;
extern crate panic_itm;
extern crate stm32f103xx;

use rtfm::app;

app! { //~ proc macro panicked
    device: stm32f103xx,

    resources: {
        static BUFFER: [u8; 16] = [0; 16];
    },

    init: {
        resources: [BUFFER],
    },

    tasks: {
        exti0: {
            interrupt: EXTI0,
            resources: [BUFFER],
            //~^ error: this resource is owned by `init` and can't be shared
        },
    },
}

fn init(_ctxt: init::Context) -> init::LateResources {
    init::LateResources {}
}

fn idle(_ctxt: idle::Context) -> ! {
    loop {}
}

fn exti0(_ctxt: exti0::Context) {}