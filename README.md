# 基础阶段 - Rust 进阶 & OS 入门

基于 [LearningOS 2026s 课程仓库](https://github.com/LearningOS/2026s-oscamp-base-2026s-oscamp-base-exercise-oscamp-base-experiment)，由 [LearningOS](https://github.com/LearningOS) 统一分配学员仓库、运行真实测试并上传 OpenCamp。

**24 项练习 · 总分：100**

## 领取与克隆

1. 加入 [OpenCamp 秋冬季训练营](https://opencamp.cn/os2edu/camp/2026fall)，并绑定自己的 GitHub 账号。
2. 点击[领取作业仓库](https://github.com/LearningOS/2026a-enroll/issues/new?template=base.yml)，点击 **Create** 提交申请；等待机器人回复，然后接受仓库邀请。
3. 克隆回复中的作业仓库，在 `main` 分支完成实验。

```sh
git clone https://github.com/LearningOS/2026a-oscamp-base-YOUR_GITHUB_LOGIN.git
cd 2026a-oscamp-base-YOUR_GITHUB_LOGIN
```

将 `YOUR_GITHUB_LOGIN` 替换为自己的 GitHub 登录名。两条命令依次下载作业仓库并进入目录。OpenCamp 绑定、领取和推送应使用同一个 GitHub 账号。

## 环境配置

本课程学习 Rust 并发、异步编程、`no_std` 开发与操作系统核心概念。系统调用和汇编练习需要 Linux；Windows 可使用 WSL2 的 Ubuntu 环境。第 4 模块使用 RISC-V，其余模块在 Linux 主机上运行。

先安装 [Rust 工具链管理器 rustup](https://rustup.rs/)。在 Ubuntu 中安装系统编译工具和 RISC-V 用户态模拟环境：

```sh
sudo apt-get update
sudo apt-get install -y build-essential gcc-riscv64-linux-gnu qemu-user
```

第一条更新软件包索引，第二条安装本机编译器、RISC-V 交叉编译器及 QEMU 用户态模拟器。Rust 版本由仓库的 `rust-toolchain.toml` 指定；仓库同时配置了 RISC-V 编译目标、链接器与运行器。

```sh
cargo build -p oscamp-cli
cargo run -p oscamp-cli -- watch
```

先构建本仓库的练习工具，再进入交互模式；`-p oscamp-cli` 选择工具包，`-- watch` 将 `watch` 参数交给练习工具。

也可以在自己作业仓库中选择 **Code → Codespaces → Create codespace on main**，等待开发环境初始化后执行同一练习命令。Codespaces 使用自己账号的可用额度。

## 模块与知识点

共 **6 个模块、24 项练习**。建议依次阅读各模块及题目文件中的说明，再实现 `src/lib.rs` 内的 `todo!()`。

### 1. 同步并发：`exercises/01_concurrency_sync/`

| 练习目录 | 主要知识点 |
| --- | --- |
| `01_thread_spawn` | `thread::spawn`、`move` 闭包、`join` |
| `02_mutex_counter` | `Arc<Mutex<T>>` 与共享状态 |
| `03_channel` | `mpsc::channel` 与多生产者通信 |
| `04_process_pipe` | `Command`、`Stdio::piped()` 与进程管道 |

### 2. no_std 开发：`exercises/02_no_std_dev/`

| 练习目录 | 主要知识点 |
| --- | --- |
| `01_mem_primitives` | `memcpy`、`memset`、`memmove`、`strlen`、`strcmp` |
| `02_bump_allocator` | `GlobalAlloc`、Bump 分配器、CAS 并发保护 |
| `03_free_list_allocator` | 空闲链表、侵入式链表、首次适配策略 |
| `04_syscall_wrapper` | 系统调用 ABI、内联汇编与跨架构差异 |
| `05_fd_table` | 文件描述符表、`Arc<dyn File>`、描述符复用 |

### 3. 操作系统并发：`exercises/03_os_concurrency/`

| 练习目录 | 主要知识点 |
| --- | --- |
| `01_atomic_counter` | `AtomicU64`、`fetch_add`、CAS 循环 |
| `02_atomic_ordering` | 内存顺序、Release-Acquire、`OnceCell` |
| `03_spinlock` | 自旋锁、`compare_exchange`、`spin_loop` |
| `04_spinlock_guard` | RAII、`Deref`、`DerefMut`、`Drop` |
| `05_rwlock` | 手写读写锁与写者优先策略 |

### 4. 上下文切换：`exercises/04_context_switch/`

| 练习目录 | 主要知识点 |
| --- | --- |
| `01_stack_coroutine` | 被调用者保存寄存器、栈帧、上下文切换 |
| `02_green_threads` | 绿色线程、协作式调度与 yield |

该模块仅支持 RISC-V。使用练习工具或 `./check.sh` 会选择对应目标；详细说明见[上下文切换模块](exercises/04_context_switch/)。

### 5. 异步编程：`exercises/05_async_programming/`

| 练习目录 | 主要知识点 |
| --- | --- |
| `01_basic_future` | `Future`、`Poll`、`Waker` |
| `02_tokio_tasks` | `tokio::spawn`、`JoinHandle` 与并发任务 |
| `03_async_channel` | 异步生产者消费者与 `tokio::sync::mpsc` |
| `04_select_timeout` | `tokio::select!`、超时控制、竞争执行 |

### 6. 页表：`exercises/06_page_table/`

| 练习目录 | 主要知识点 |
| --- | --- |
| `01_pte_flags` | SV39 页表项位布局与标志位 |
| `02_page_table_walk` | VPN、页内偏移、地址转换与缺页 |
| `03_multi_level_pt` | 三级页表遍历与 2 MB 大页 |
| `04_tlb_sim` | TLB 查询、FIFO 替换、按页或 ASID 刷新 |

## 练习与本地测试

运行 `cargo run -p oscamp-cli -- watch`，打开当前题目的 `src/lib.rs`，阅读说明并补全代码。保存后工具会重新测试，通过后跳转到下一题。

| 交互按键 | 作用 |
| --- | --- |
| `h` | 查看当前题目提示 |
| `l` | 查看全部题目和进度 |
| `n` / `p` | 下一题 / 上一题 |
| `r` / Enter | 重新测试 |
| `q` / Esc | 退出 |

也可以在仓库根目录按需运行：

| 命令 | 作用 |
| --- | --- |
| `cargo run -p oscamp-cli -- list` | 查看完成状态 |
| `cargo run -p oscamp-cli -- check` | 检查全部练习，并为 RISC-V 题目选择对应目标 |
| `cargo run -p oscamp-cli -- run thread_spawn` | 测试指定练习 |
| `cargo run -p oscamp-cli -- hint thread_spawn` | 查看指定练习的提示 |
| `cargo run -p oscamp-cli -- help` | 查看工具帮助 |
| `cargo test -p thread_spawn` | 直接执行该包的测试 |
| `cargo test -p thread_spawn -- --nocapture` | 同时显示测试中的标准输出 |

包名与目录名不一定相同，例如 `03_async_channel` 的包名是 `async_channel_ex`。跨架构的全量检查使用上表中的 `check` 命令。

## 提交实验

完成一部分练习后，检查并保存改动：

```sh
git diff
git add exercises
git commit -m "Complete base exercises"
git push origin main
```

依次检查差异、暂存练习代码、创建提交并推送到 `main`。也可以使用已登录的 GitHub Desktop 或编辑器完成提交与推送。

## 计分规则


沿用往期各项练习的权重，每项全部通过才获得该项分数。每次提交重新计算全部练习，上传本次实际总分；不是按提交次数累加。部分完成也会上传测得的分数。编译或测试未通过、超时的练习记 0 分；环境准备失败、缺少结果或结果不完整时不上传。

| 练习 | 分值 |
| --- | ---: |
| `thread_spawn` | 3 |
| `mutex_counter` | 4 |
| `channel` | 4 |
| `process_pipe` | 4 |
| `mem_primitives` | 4 |
| `bump_allocator` | 5 |
| `free_list_allocator` | 5 |
| `syscall_wrapper` | 5 |
| `fd_table` | 5 |
| `atomic_counter` | 4 |
| `atomic_ordering` | 4 |
| `spinlock` | 4 |
| `spinlock_guard` | 4 |
| `rwlock` | 5 |
| `stack_coroutine` | 6 |
| `green_threads` | 6 |
| `basic_future` | 4 |
| `tokio_tasks` | 3 |
| `async_channel_ex` | 3 |
| `select_timeout` | 4 |
| `pte_flags` | 3 |
| `page_table_walk` | 4 |
| `multi_level_pt` | 4 |
| `tlb_sim` | 3 |

Actions 中测试作业变红表示还有未完成练习；单独的 **Save measured score and upload to OpenCamp** 作业显示成绩同步是否成功。日志出现 `OpenCamp accepted the score (result=1).` 才表示接口接受成绩。

成绩明细同时保存在运行附件和学员仓库的 `gh-pages` 分支的成绩文件，无需启用 GitHub Pages。

## 查看成绩与处理问题

在自己仓库的 **Actions** 打开本次运行，查看各题日志与分数，再到 [OpenCamp 秋冬季训练营](https://opencamp.cn/os2edu/camp/2026fall) 核对自己的成绩。只有分配的学员账号触发的运行上传该账号成绩；维护者代推只测试。

如果上传失败，先确认已加入训练营，并绑定领取仓库的 GitHub 账号，再把 Actions 链接和错误信息交给助教。修复后可以重跑工作流。环境安装失败或运行取消时，不会上传不完整结果。

## 许可证

按 [MIT 许可证](LICENSE) 分发。
