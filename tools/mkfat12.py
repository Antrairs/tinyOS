import struct
import sys

SECTOR_SIZE = 512
TOTAL_SECTORS = 2880  # 1.44 MB disk
IMAGE_SIZE = TOTAL_SECTORS * SECTOR_SIZE

# 创建全零磁盘镜像
image = bytearray(IMAGE_SIZE)
boot = memoryview(image)[0:SECTOR_SIZE]

# BPB 前面的跳转指令和 OEM 名称
boot[0:3] = b'\xeb\x3c\x90'
boot[3:11] = b'TINYOS  '

# BPB：多字节整数使用小端序
struct.pack_into("<H", boot, 11, 512)    # 每扇区字节数
boot[13] = 1                             # 每簇扇区数
struct.pack_into("<H", boot, 14, 1)      # 保留扇区数
boot[16] = 2                             # FAT 副本数
struct.pack_into("<H", boot, 17, 224)    # 根目录项数
struct.pack_into("<H", boot, 19, 2880)   # 总扇区数
boot[21] = 0xF0                          # 1.44 MB 软盘媒体描述符
struct.pack_into("<H", boot, 22, 9)      # 每份 FAT 的扇区数
struct.pack_into("<H", boot, 24, 18)     # 每磁道扇区数
struct.pack_into("<H", boot, 26, 2)      # 磁头数
struct.pack_into("<I", boot, 28, 0)      # 隐藏扇区数
struct.pack_into("<I", boot, 32, 0)      # 32 位总扇区数，这里用 16 位字段

# FAT12
boot[36] = 0x00                          # 软盘驱动器号
boot[37] = 0
boot[38] = 0x29                          # 扩展引导签名
struct.pack_into("<I", boot, 39, 0x12345678)
boot[43:54] = b"TINYOS DISK"             # 11 字节卷标
boot[54:62] = b"FAT12   "                # 8 字节类型提示

# 引导扇区签名
boot[510:512] = b"\x55\xAA"

image_path = sys.argv[1] if len(sys.argv) > 1 else "build/data.img"

# 计算各区域在镜像中的字节偏移
fat_size = 9 * SECTOR_SIZE
fat1_start = 1 * SECTOR_SIZE
fat2_start = 10 * SECTOR_SIZE

# FAT12 软盘的前两个 FAT 表项是保留项
fat_header = b"\xF0\xFF\xFF"
image[fat1_start:fat1_start + 3] = fat_header
image[fat2_start:fat2_start + 3] = fat_header

with open(image_path, "wb") as f:
    f.write(image)

print(f"Created {image_path} ({len(image)} bytes)")