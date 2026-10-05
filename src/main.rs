#![no_std]
#![no_main]

use core::arch::{asm, global_asm};
use core::panic::PanicInfo;
use core::ptr::{read_volatile, write_volatile};

mod fat12;
use fat12::Fat12;

mod block;
use block::{BlockDevice, SECTOR_SIZE};

mod virtio;
use crate::virtio::VirtioBlock;

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
        putchar(c);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn kernel_main() -> ! {
    puts("\nHello tinyOS!\n");

    let base = match virtio::find_block_device() {
        Some(base) => base,
        None => {
            puts("VirtIO block not found!\n");
            halt();
        }
    };

    let mut disk = match VirtioBlock::new(base) {
        Some(disk) => disk,
        None => {
            puts("VirtIO init failed!\n");
            halt();
        }
    };

    puts("VirtIO block initialized!\n");

    let mut sector0 = [0u8; SECTOR_SIZE];

    disk.read_sector(0, &mut sector0);

    if sector0[510] == 0x55 && sector0[511] == 0xAA {
        puts("Sector 0 read OK!\n");
    } else {
        puts("Sector 0 read failed!\n");
    }

    let mut fs = Fat12::new(&mut disk);

    puts("FAT12 mounted!\n");
    puts("Root directory:\n");

    let dir_name = b"TEST       ";
    if fs.mkdir(0, dir_name) {
        puts("mkdir TEST OK!\n");
    }

    let dir = match fs.find_dir(0, dir_name) {
        Some(cluster) => cluster,
        None => {
            puts("Find TEST failed!\n");
            halt();
        }
    };

    let sub_name = b"SUB        ";

    if fs.mkdir(dir, sub_name) {
        puts("mkdir SUB OK!\n");
    }

    let sub_dir = match fs.find_dir(dir, sub_name) {
        Some(cluster) => cluster,
        None => {
            puts("Find SUB failed!\n");
            halt();
        }
    };

    puts("List TEST dir:\n");
    fs.list_dir(dir, |name| {
        for &c in name {
            putchar(c);
        }
        putchar(b'\n');
    });

    puts("List sub dir:\n");
    fs.list_dir(sub_dir, |name| {
        for &c in name {
            putchar(c);
        }
        putchar(b'\n');
    });

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
    puts("\nKernel panic!\n");
    halt();
}
