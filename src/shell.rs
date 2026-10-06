use core::ptr::{read_volatile, write_volatile};

use crate::fat12::{Fat12, to_83};

const UART0: usize = 0x1000_0000;

fn putchar(c: u8) {
    const LSR: usize = 5;
    const TX_EMPTY: u8 = 1 << 5;

    unsafe {
        while read_volatile((UART0 + LSR) as *const u8) & TX_EMPTY == 0 {}
        write_volatile(UART0 as *mut u8, c);
    }
}

pub fn puts(s: &str) {
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


pub fn run(fs: &mut Fat12<'_>) {
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
                puts("cd     - change directory\n");
                puts("write  - write file\n");
                puts("type   - show file content\n");
                puts("format - format FAT12 disk\n");
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
            b"cd" => {
                if arg.is_empty() {
                    puts("Usage: cd <dirname>\n");
                    continue;
                }

                if arg == b".." {
                    if current_dir != 0 {
                        match fs.find_dir(current_dir, b"..         ") {
                            Some(parent) => {
                                current_dir = parent;
                            }
                            None => {
                                puts("Parent directory not found\n");
                            }
                        }
                    }
                    continue;
                }

                if arg == b"." {
                    continue;
                }

                // cd 普通目录
                let name = match to_83(arg) {
                    Some(name) => name,
                    None => {
                        puts("Invalid directory name\n");
                        continue;
                    }
                };
                match fs.find_dir(current_dir, &name) {
                    Some(cluster) => {
                        current_dir = cluster;
                    }
                    None => {
                        puts("Directory not found\n");
                    }
                }
            }
            b"write" => {
                let space = match arg.iter().position(|&b| b == b' ') {
                    Some(pos) => pos,
                    None => {
                        puts("Usage: write <filename> <text>\n");
                        continue;
                    }
                };

                let filename = &arg[..space];
                let mut text_start = space + 1;

                // 跳过文件名和正文之间多余的空格
                while text_start < arg.len() && arg[text_start] == b' ' {
                    text_start += 1;
                }

                let text = &arg[text_start..];

                let name = match to_83(filename) {
                    Some(name) => name,
                    None => {
                        puts("Invalid filename\n");
                        continue;
                    }
                };

                if fs.write_bytes(current_dir, &name, text) {
                    puts("Write OK\n");
                } else {
                    puts("Write failed\n");
                }
            }
            b"type" => {
                if arg.is_empty() {
                    puts("Usage: type <filename>\n");
                    continue;
                }

                let name = match to_83(arg) {
                    Some(name) => name,
                    None => {
                        puts("Invalid filename\n");
                        continue;
                    }
                };

                let mut buf = [0u8; 4096];
                match fs.read_bytes(current_dir, &name, &mut buf) {
                    Some(size) => {
                        for &c in &buf[..size] {
                            putchar(c);
                        }
                        putchar(b'\n');
                    }
                    None => {
                        puts("Read failed\n");
                    }
                }
            }
            b"format" => {
                fs.format();
                current_dir = 0;
                puts("Format OK\n");
            }
            _ => {
                puts("Unknown command\n");
            }
        }
    }
}