#![no_std]
#![no_main]

use core::arch::{asm, global_asm};
use core::panic::PanicInfo;
use core::ptr::{read_volatile, write_volatile};

global_asm!(
    r#"
    .section .text.entry
    .global _start

_start:
    la sp, __stack_top

    call kernel_main

1:
    wfi
    j 1b
"#
);

const UART0: usize = 0x1000_0000;

fn putchar(c: u8) {
    const LSR: usize = 5;
    const TX_EMPTY: u8 = 1 << 5;

    unsafe {
        while read_volatile((UART0 + LSR) as *const u8) & TX_EMPTY == 0 {}

        write_volatile(UART0 as *mut u8, c);
    }
}

fn puts(s: &str) {
    for c in s.bytes() {
        if c == b'\n' {
            putchar(b'\r');
        }
        putchar(c);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn kernel_main() -> ! {
    puts("\nHello tinyOS!\n");

    loop {
        unsafe {
            asm!("wfi");
        }
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    puts("\nKernel panic!\n");

    loop {
        unsafe {
            asm!("wfi");
        }
    }
}