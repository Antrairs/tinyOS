<div align="center">
    <h1>TinyOS</h1>
    <p><strong>A tiny RISC-V learning OS with a hand-built FAT12 filesystem and responsive terminal file browser.</strong></p>
</div>

TinyOS 是一个使用 Rust 编写的 RISC-V 裸机学习项目，运行在 QEMU virt 虚拟机上。

项目通过 VirtIO 块设备访问一个经典的 1.44 MB FAT12 磁盘镜像，并从零开始实现了 FAT12 文件系统。

在文件系统之上，tinyOS 还提供了一个交互式 Shell，以及使用 ANSI 控制序列实现的响应式全屏文件浏览器，可浏览目录、查看文件大小并直接预览文本文件。

![文件浏览器](docs/tui1.png)

![文件查看器](docs/tui2.png)

## 快速开始

推荐在 Ubuntu / Debian 下运行本项目。

克隆本仓库：

```bash
git clone https://github.com/Antrairs/tinyOS.git
```

安装 qemu 模拟器：

```bash
sudo apt install -y qemu-system-misc
```

安装 Rust（若已安装可跳过）：

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
. "$HOME/.cargo/env"
```

安装 RISC-V 裸机目标：

```bash
rustup target add riscv64gc-unknown-none-elf
```

首次运行时需要准备 1.44 MB 磁盘：

```bash
mkdir -p build
dd if=/dev/zero of=build/fat12.img bs=512 count=2880
```

启动 TinyOS：

```bash
cargo run
```

首次使用全零镜像时，在 `tinyOS>` 提示符下执行：

```bash
format
```

后续启动无需再次格式化。

退出 QEMU 按 Ctrl+A，松开后再按 X。


## 支持命令

Shell 当前提供：

| Command | Usage | Description |
|---|---|---|
| `help` | `help` | 显示帮助 |
| `browse` | `browse` | 打开全屏文件浏览器 |
| `dir` | `dir` | 列出当前目录 |
| `touch` | `touch FILE` | 创建文件 |
| `mkdir` | `mkdir DIR` | 创建目录 |
| `cd` | `cd DIR` | 切换目录 |
| `write` | `write FILE TEXT` | 写入文件 |
| `type` | `type FILE` | 输出文件内容 |
| `format` | `format` | 格式化 FAT12 磁盘 |

tinyOS 的 `format` 命令可以从全零 1.44 MB 镜像直接创建 FAT12 文件系统。生成后的镜像可以在宿主机检查：

```bash
fsck.fat -vn build/fat12.img
```
若未安装该工具运行下面的命令安装：

```bash
sudo apt install -y dosfstools
```

## TUI 文件浏览器

在受支持的终端中执行 `browse` 即可进入全屏文件浏览器。

浏览器支持方向键选择、目录进入与返回、文件大小显示以及文本文件预览。

## FAT12 极简设计

TinyOS 使用经典 1.44 MB FAT12 布局：

```text
Sector 0       Boot Sector / BPB
Sector 1-9     FAT #1
Sector 10-18   FAT #2
Sector 19-32   Root Directory
Sector 33-2879 Data Area
```

但是做了一定程度的简化。

核心参数为：

```text
512 bytes / sector
1 sector / cluster
2 FAT copies
224 root directory entries
2880 sectors
```
当文件内容超过一个 cluster 时，会继续分配空闲 cluster 并链接到 FAT chain；覆盖已有文件时，则释放原有 chain 后重新分配。

子目录使用普通 data cluster 保存，并创建 `.` 和 `..` 两个特殊目录项。FAT12 根目录则保留其经典的固定 224-entry 布局。

## 技术取舍

tinyOS 以理解文件系统和裸机程序的工作方式为目标。当前优先完成从磁盘读写到文件操作、再到交互界面的完整流程，逐步扩展功能。作为一个以学习为主的项目，我有意在控制复杂度。

为将注意力聚集在文件系统本身，设备层使用 VirtIO。`BlockDevice` trait 将 FAT12 与具体硬件隔离，使文件系统只处理 sector read/write。

文件名目前只支持 FAT 8.3。这样可以简化文件命名和显示，把重点留在 FAT12 的结构上。

TUI 直接使用 ANSI 终端控制序列，实现颜色、方向键、光标控制和响应式布局。

## 实现进度

| Area | Status | Notes |
|---|---|---|
| RISC-V bare-metal boot | ✅ | QEMU `virt` + OpenSBI |
| UART Shell | ✅ | 命令输入输出 |
| VirtIO block device | ✅ | 持久化 sector I/O |
| FAT12 BPB / format | ✅ | 可从全零磁盘创建 FAT12 |
| Packed 12-bit FAT | ✅ | 读取、修改、双 FAT 同步 |
| Cluster allocation | ✅ | 分配、链接、释放 |
| File read/write | ✅ | 支持 multi-cluster |
| Root directory | ✅ | 标准 224-entry root |
| Subdirectories | ✅ | `mkdir`、`.`、`..`、嵌套导航 |
| FAT 8.3 filenames | ✅ | 创建与转换 |
| `fsck.fat` validation | ✅ | 标准工具可识别 |
| Full-screen TUI browser | ✅ | 目录浏览、滚动、方向键 |
| Responsive layout | ✅ | Terminal size query + fallback |
| File viewer | ✅ | 小型文本文件预览 |


