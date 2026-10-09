#!/bin/bash
# OSCamp Exercise Checker
# Checks each exercise's test status locally (no scoring — scoring runs in CI).

set -e

GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

PASS=0
FAIL=0
SKIP=0

# Repository root (where this script lives)
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Module 4 uses riscv64 target
RISCV64_TARGET="riscv64gc-unknown-linux-gnu"
RISCV64_SYSROOT="${RISCV64_SYSROOT:-/usr/riscv64-linux-gnu}"

# Ensure riscv64 cross-compilation environment is ready (Linux only)
ensure_riscv64_ready() {
    local arch
    arch=$(uname -m)
    if [ "$arch" = "riscv64" ]; then
        return 0
    fi
    if [ "$(uname -s)" = "Darwin" ]; then
        return 0
    fi

    if [ -d "${REPO_ROOT}/scripts" ]; then
        chmod +x "${REPO_ROOT}/scripts/setup_riscv64.sh" "${REPO_ROOT}/.cargo/run_riscv64.sh" 2>/dev/null || true
        echo -e "  ${YELLOW}[Module 4] Preparing riscv64 environment (target / QEMU / sysroot)...${NC}"
        (cd "$REPO_ROOT" && bash scripts/setup_riscv64.sh) || exit 1
    fi
    export CARGO_TARGET_RISCV64GC_UNKNOWN_LINUX_GNU_RUNNER="bash ${REPO_ROOT}/.cargo/run_riscv64.sh"

    if ! command -v qemu-riscv64 >/dev/null 2>&1; then
        echo -e "${RED}Error: qemu-riscv64 not found. Cannot run Module 4 tests on non-riscv64 host.${NC}" >&2
        echo "Install QEMU user-mode, e.g.:" >&2
        echo "  Debian/Ubuntu: sudo apt-get install qemu-user-static" >&2
        echo "  Fedora:        sudo dnf install qemu-user-static" >&2
        exit 1
    fi

    if [ ! -d "$RISCV64_SYSROOT" ]; then
        echo -e "${RED}Error: riscv64 sysroot not found: ${RISCV64_SYSROOT}${NC}" >&2
        echo "Install riscv64 cross toolchain, e.g.:" >&2
        echo "  Debian/Ubuntu: sudo apt-get install gcc-riscv64-linux-gnu" >&2
        echo "Or set: export RISCV64_SYSROOT=/path/to/riscv64/sysroot" >&2
        exit 1
    fi
}

# Exercise list: "module:package:name"
exercises=(
    # Module 1: Concurrency (Synchronous)
    "01_concurrency_sync:thread_spawn:Thread Creation"
    "01_concurrency_sync:mutex_counter:Mutex Shared State"
    "01_concurrency_sync:channel:Channel Communication"
    "01_concurrency_sync:process_pipe:Process Pipes"
    # Module 2: no_std Development
    "02_no_std_dev:mem_primitives:Memory Primitives"
    "02_no_std_dev:bump_allocator:Bump Allocator"
    "02_no_std_dev:free_list_allocator:Free-List Allocator"
    "02_no_std_dev:syscall_wrapper:Syscall Wrapper"
    "02_no_std_dev:fd_table:File Descriptor Table"
    # Module 3: OS Concurrency Advanced
    "03_os_concurrency:atomic_counter:Atomic Counter"
    "03_os_concurrency:atomic_ordering:Memory Ordering"
    "03_os_concurrency:spinlock:Spinlock"
    "03_os_concurrency:spinlock_guard:RAII Spinlock Guard"
    "03_os_concurrency:rwlock:Read-Write Lock"
    # Module 4: Context Switching
    "04_context_switch:stack_coroutine:Stackful Coroutine"
    "04_context_switch:green_threads:Green Threads"
    # Module 5: Async Programming
    "05_async_programming:basic_future:Manual Future"
    "05_async_programming:tokio_tasks:Tokio Tasks"
    "05_async_programming:async_channel_ex:Async Channel"
    "05_async_programming:select_timeout:Select/Timeout"
    # Module 6: Page Tables
    "06_page_table:pte_flags:PTE Flags"
    "06_page_table:page_table_walk:Page Table Walk"
    "06_page_table:multi_level_pt:SV39 Multi-Level PT"
    "06_page_table:tlb_sim:TLB Simulation"
)

echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}   OSCamp Exercise Checker${NC}"
echo -e "${BLUE}========================================${NC}"
echo ""

current_module=""
riscv64_ready=0

for entry in "${exercises[@]}"; do
    IFS=':' read -r module package desc <<< "$entry"

    if [ "$module" != "$current_module" ]; then
        current_module="$module"
        echo -e "\n${YELLOW}[$module]${NC}"
        # Prepare riscv64 environment before Module 4
        if [ "$module" = "04_context_switch" ] && [ "$riscv64_ready" -eq 0 ]; then
            ensure_riscv64_ready
            riscv64_ready=1
        fi
    fi

    printf "  %-25s %-20s " "$desc" "($package)"

    if [ "$module" = "04_context_switch" ]; then
        if [ "$(uname -s)" = "Darwin" ]; then
            echo -e "${YELLOW}SKIP (macOS)${NC}"
            ((SKIP++))
        elif cargo test -p "$package" --target "$RISCV64_TARGET" --quiet -- --nocapture 2>/dev/null; then
            echo -e "${GREEN}PASS${NC}"
            ((PASS++))
        else
            echo -e "${RED}FAIL${NC}"
            ((FAIL++))
        fi
    else
        if cargo test -p "$package" --quiet 2>/dev/null; then
            echo -e "${GREEN}PASS${NC}"
            ((PASS++))
        else
            echo -e "${RED}FAIL${NC}"
            ((FAIL++))
        fi
    fi
done

echo ""
echo -e "${BLUE}========================================${NC}"
TOTAL=$((PASS + FAIL + SKIP))
echo -e "  Passed: ${GREEN}$PASS${NC} / Failed: ${RED}$FAIL${NC} / Skipped: ${YELLOW}$SKIP${NC} / Total: $TOTAL"
echo -e "  Progress: $PASS/$TOTAL"
echo -e "${BLUE}========================================${NC}"

if [ $FAIL -eq 0 ]; then
    echo -e "\n${GREEN}Congratulations! All exercises passed!${NC}"
    exit 0
else
    echo -e "\n${YELLOW}$FAIL exercise(s) remaining. Keep going!${NC}"
    exit 1
fi
