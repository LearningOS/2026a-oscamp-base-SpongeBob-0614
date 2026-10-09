#!/usr/bin/env bash
# 一键安装第四章（上下文切换）所需环境；缺什么就尝试安装什么。
# 可由 check.sh 在跑第四章前自动调用，也可在仓库根目录手动执行：bash scripts/setup_riscv64.sh

set -e

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

TARGET="riscv64gc-unknown-linux-gnu"
SYSROOT="${RISCV64_SYSROOT:-/usr/riscv64-linux-gnu}"

# macOS 无 apt/dnf 与 riscv64-linux-gnu-gcc，交叉编译需用 Docker 或 cross
if [ "$(uname -s)" = "Darwin" ]; then
    echo -e "${YELLOW}当前为 macOS。第四章交叉编译到 riscv64 需在 Linux 下进行（或使用 cross）。${NC}"
    echo ""
    echo "可选方式："
    echo "  1) 在 Linux 机器或 Linux 虚拟机内克隆本仓库并执行 ./check.sh"
    echo "  2) 使用 cross（Docker）："
    echo "     cargo install cross"
    echo "     cross test --target $TARGET -p stack_coroutine -p green_threads"
    echo ""
    exit 1
fi

# 尝试安装 QEMU 用户态（Debian/Ubuntu / Fedora）
try_install_qemu() {
    if command -v apt-get >/dev/null 2>&1; then
        echo -e "${YELLOW}    尝试安装: sudo apt-get install -y qemu-user-static${NC}"
        sudo apt-get update -qq && sudo apt-get install -y qemu-user-static
        return $?
    fi
    if command -v dnf >/dev/null 2>&1; then
        echo -e "${YELLOW}    尝试安装: sudo dnf install -y qemu-user-static${NC}"
        sudo dnf install -y qemu-user-static
        return $?
    fi
    if command -v yum >/dev/null 2>&1; then
        echo -e "${YELLOW}    尝试安装: sudo yum install -y qemu-user-static${NC}"
        sudo yum install -y qemu-user-static
        return $?
    fi
    return 1
}

# 尝试安装 riscv64 交叉工具链（提供 linker 与 sysroot，Cargo 用 linker 才能正确链接）
try_install_cross_toolchain() {
    if command -v apt-get >/dev/null 2>&1; then
        echo -e "${YELLOW}    尝试安装: sudo apt-get install -y gcc-riscv64-linux-gnu${NC}"
        sudo apt-get update -qq && sudo apt-get install -y gcc-riscv64-linux-gnu
        return $?
    fi
    if command -v dnf >/dev/null 2>&1; then
        echo -e "${YELLOW}    尝试安装: sudo dnf install -y gcc-riscv64-linux-gnu${NC}"
        sudo dnf install -y gcc-riscv64-linux-gnu 2>/dev/null || true
        command -v riscv64-linux-gnu-gcc >/dev/null 2>&1 && return 0
        return 1
    fi
    if command -v yum >/dev/null 2>&1; then
        echo -e "${YELLOW}    尝试安装: sudo yum install -y gcc-riscv64-linux-gnu${NC}"
        sudo yum install -y gcc-riscv64-linux-gnu 2>/dev/null || true
        command -v riscv64-linux-gnu-gcc >/dev/null 2>&1 && return 0
        return 1
    fi
    return 1
}

echo "==> 1. 添加 Rust target: $TARGET"
rustup target add "$TARGET"
echo -e "${GREEN}    target 已就绪${NC}"

echo ""
echo "==> 2. 检查 riscv64 交叉 linker (riscv64-linux-gnu-gcc)"
echo "    若缺少会报错: linking with cc failed / incompatible with elf64-x86-64"
if command -v riscv64-linux-gnu-gcc >/dev/null 2>&1; then
    echo -e "${GREEN}    已安装: $(which riscv64-linux-gnu-gcc)${NC}"
else
    if try_install_cross_toolchain; then
        echo -e "${GREEN}    安装完成${NC}"
    else
        echo -e "${RED}    未找到且自动安装失败。交叉编译必须使用 riscv64 的 linker。${NC}"
        echo "    请手动安装，例如："
        echo "       Debian/Ubuntu: sudo apt-get install gcc-riscv64-linux-gnu"
        echo "       Fedora:        sudo dnf install gcc-riscv64-linux-gnu"
        exit 1
    fi
fi

echo ""
echo "==> 3. 检查 QEMU 用户态 (qemu-riscv64)"
if command -v qemu-riscv64 >/dev/null 2>&1; then
    echo -e "${GREEN}    已安装: $(which qemu-riscv64)${NC}"
else
    if try_install_qemu; then
        echo -e "${GREEN}    安装完成${NC}"
    else
        echo -e "${RED}    未找到且自动安装失败。请手动安装，例如：${NC}"
        echo "       Debian/Ubuntu: sudo apt-get install qemu-user-static"
        echo "       Fedora:        sudo dnf install qemu-user-static"
        exit 1
    fi
fi

echo ""
echo "==> 4. 检查 riscv64 根文件系统 (用于 QEMU 运行时的动态链接)"
if [ -d "$SYSROOT" ]; then
    echo -e "${GREEN}    已找到: $SYSROOT${NC}"
else
    if try_install_cross_toolchain; then
        [ -d "$SYSROOT" ] && echo -e "${GREEN}    安装完成${NC}" || echo -e "${YELLOW}    工具链已装，sysroot 路径可能不同，可设置 RISCV64_SYSROOT${NC}"
    else
        echo -e "${RED}    未找到: $SYSROOT。请手动安装 gcc-riscv64-linux-gnu 或设置 RISCV64_SYSROOT${NC}"
        exit 1
    fi
fi

echo ""
echo -e "${GREEN}环境就绪。可直接运行：${NC}"
echo "  ./check.sh                    # 检查全部练习（第四章会在 QEMU 下运行）"
echo "  cargo test -p stack_coroutine -p green_threads --target $TARGET   # 仅第四章"
echo ""
