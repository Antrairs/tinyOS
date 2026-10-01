#![no_std]
#![no_main]

use core::arch::{asm, global_asm};
use core::panic::PanicInfo;
use core::ptr::{read_volatile, write_volatile};

mod fat12;
use fat12::Fat12;

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
const IMAGE_SIZE: usize = include_bytes!("../build/fat12.img").len();
static mut IMAGE: [u8; IMAGE_SIZE] = *include_bytes!("../build/fat12.img");

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
        putchar(c);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn kernel_main() -> ! {
    puts("\nHello tinyOS!\n");

    let image = unsafe { &mut *core::ptr::addr_of_mut!(IMAGE) };
    let mut fs = Fat12::new(image);

    fs.list_root_name(|name| {
        for &c in name {
            putchar(c);
        }
        putchar(b'\n');
    });

    if fs.create_file(b"EMPTY   TXT") {
        puts("\nCreated EMPTY.TXT\n");
    } else {
        puts("\nCreate failed\n")
    }

    if fs.write_bytes(b"EMPTY   TXT", b"Hello, TinyOS!") {
        puts("\nWrite to EMPTY.TXT successful\n");
    } else {
        puts("\nWrite to EMPTY.TXT failed\n");
    }

    halt();
}

fn halt() -> ! {
    loop {
        unsafe {
            asm!("wfi");
        }
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    puts("\rKernel panic!\r");
    halt();
}
