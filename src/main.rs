#![no_std]
#![no_main]

use core::arch::{asm, global_asm};
use core::panic::PanicInfo;
use core::ptr::{read_volatile, write_volatile};

mod fat12;
use fat12::{Fat12, to_83};

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

fn getchar() -> u8 {
    const LSR: usize = 5;
    const RX_READY: u8 = 1;

    unsafe {
        while read_volatile((UART0 + LSR) as *const u8) & RX_READY == 0 {}
        read_volatile(UART0 as *const u8)
    }
}

fn read_line(buf: &mut [u8]) -> usize {
    let mut len = 0;

    loop {
        let c = getchar();

        match c {
            b'\r' => {
                putchar(b'\n');
                return len;
            }
            127 => {
                if len > 0 {
                    len -= 1;
                    puts("\x08 \x08"); // 光标左移 擦掉字符 再左移
                }
            }
            _ => {
                // 可显示字符范围
                if c >= 32 && c <= 126 && len < buf.len() {
                    buf[len] = c;
                    len += 1;
                    putchar(c);
                }
            }
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn kernel_main() -> ! {
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

    let mut fs = Fat12::new(&mut disk);

    let mut line = [0u8; 128];
    let mut current_dir = 0;

    loop {
        puts("tinyOS> ");

        let len = read_line(&mut line);
        let input = &line[..len];

        if input.is_empty() {
            continue;
        }

        let space = input.iter().position(|&c| c == b' ');

        let (cmd, arg) = match space {
            Some(pos) => {
                let mut arg_start = pos + 1;

                while arg_start < input.len() && input[arg_start] == b' ' {
                    arg_start += 1;
                }
                (&input[..pos], &input[arg_start..])
            }
            None => (input, &[][..]),
        };

        match cmd {
            b"help" => {
                puts("help   - show commands\n");
                puts("dir    - list directory\n");
                puts("touch  - create file\n");
                puts("mkdir  - create directory\n");
            }
            b"dir" => {
                fs.list_dir(current_dir, |name| {
                    for &c in name {
                        putchar(c);
                    }
                    putchar(b'\n');
                });
            }
            b"touch" => {
                if arg.is_empty() {
                    puts("Usage: touch <filename>\n");
                    continue;
                }

                let name = match to_83(arg) {
                    Some(name) => name,
                    None => {
                        puts("Invalid filename\n");
                        continue;
                    }
                };

                if fs.create_file(current_dir, &name) {
                    puts("File created\n");
                } else {
                    puts("Create failed\n");
                }
            }
            b"mkdir" => {
                if arg.is_empty() {
                    puts("Usage: mkdir <dirname>\n");
                    continue;
                }

                let name = match to_83(arg) {
                    Some(name) => name,
                    None => {
                        puts("Invalid directory name\n");
                        continue;
                    }
                };

                if fs.mkdir(current_dir, &name) {
                    puts("Directory created\n");
                } else {
                    puts("Create failed\n");
                }
            }
            _ => {
                puts("Unknown command\n");
            }
        }
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
    puts("\nKernel panic!\n");
    halt();
}
