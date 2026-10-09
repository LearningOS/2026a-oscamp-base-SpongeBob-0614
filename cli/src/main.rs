use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{self, Clear, ClearType},
};
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use serde::Deserialize;
use std::io::{self, Write};
use std::path::Path;
use std::process::Command;
use std::sync::mpsc;
use std::time::Duration;

const GREEN: &str = "\x1b[32m";
const RED: &str = "\x1b[31m";
const YELLOW: &str = "\x1b[33m";
const BLUE: &str = "\x1b[34m";
const CYAN: &str = "\x1b[36m";
const BOLD: &str = "\x1b[1m";
const DIM: &str = "\x1b[2m";
const RESET: &str = "\x1b[0m";

#[derive(Debug, Deserialize, Clone)]
struct Exercise {
    name: String,
    package: String,
    path: String,
    module: String,
    description: String,
    hint: String,
}

#[derive(Debug, Deserialize)]
struct Config {
    exercise: Vec<Exercise>,
}

struct TestResult {
    passed: bool,
    output: String,
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let exercises = load_exercises();

    match args.get(1).map(String::as_str) {
        None | Some("watch") => watch_mode(&exercises),
        Some("list") => list_mode(&exercises),
        Some("check") => check_mode(&exercises),
        Some("run") => run_mode(&exercises, args.get(2)),
        Some("hint") => hint_mode(&exercises, args.get(2)),
        Some("help" | "--help" | "-h") => print_usage(),
        Some(other) => {
            eprintln!("Unknown command: {other}");
            print_usage();
            std::process::exit(1);
        }
    }
}

fn load_exercises() -> Vec<Exercise> {
    for path in ["exercises.toml", "../exercises.toml"] {
        if let Ok(content) = std::fs::read_to_string(path) {
            let config: Config = toml::from_str(&content).expect("exercises.toml format error");
            return config.exercise;
        }
    }
    eprintln!(
        "{RED}Error:{RESET} Could not find exercises.toml, please run in project root directory"
    );
    std::process::exit(1);
}

/// 第四章（上下文切换）需用 riscv64 target，在 x86 上通过 .cargo/config 的 runner 用 QEMU 运行。
const RISCV64_TARGET: &str = "riscv64gc-unknown-linux-gnu";
const RISCV64_PACKAGES: &[&str] = &["stack_coroutine", "green_threads"];

fn need_riscv64_target(package: &str) -> bool {
    RISCV64_PACKAGES.contains(&package)
}

fn test_exercise(ex: &Exercise) -> TestResult {
    let mut args = vec!["test", "-p", &ex.package];
    if need_riscv64_target(&ex.package) {
        args.extend(["--target", RISCV64_TARGET]);
    }
    if need_riscv64_target(&ex.package) {
        args.extend(["--", "--color=always", "--nocapture"]);
    } else {
        args.extend(["--", "--color=always"]);
    }

    let output = Command::new("cargo")
        .args(&args)
        .output()
        .expect("Failed to run cargo test");

    TestResult {
        passed: output.status.success(),
        output: format!(
            "{}{}",
            String::from_utf8_lossy(&output.stderr),
            String::from_utf8_lossy(&output.stdout)
        ),
    }
}

fn test_quiet(ex: &Exercise) -> bool {
    let mut args = vec!["test", "-p", &ex.package];
    if need_riscv64_target(&ex.package) {
        args.extend(["--target", RISCV64_TARGET]);
    }
    args.push("--quiet");
    if need_riscv64_target(&ex.package) {
        args.extend(["--", "--nocapture"]);
    }

    Command::new("cargo")
        .args(&args)
        .stderr(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// In raw-mode, \n must be \r\n
fn rprint(out: &mut impl Write, s: &str) {
    let s = s.replace("\r\n", "\n").replace('\n', "\r\n");
    write!(out, "{s}").unwrap();
}

fn rprintln(out: &mut impl Write, s: &str) {
    rprint(out, s);
    write!(out, "\r\n").unwrap();
}

// ─────────────────────── watch mode ───────────────────────

fn watch_mode(exercises: &[Exercise]) {
    let total = exercises.len();
    let mut stdout = io::stdout();

    println!("{BOLD}{BLUE}OS Camp{RESET} - Scanning exercise progress...\n");

    let mut done = vec![false; total];
    let mut current = total;
    for (i, ex) in exercises.iter().enumerate() {
        print!("  [{:2}/{total}] Checking {:<25}\r", i + 1, ex.package);
        stdout.flush().unwrap();
        if test_quiet(ex) {
            done[i] = true;
        } else if current == total {
            current = i;
        }
    }

    fn count_done(done: &[bool]) -> usize {
        done.iter().filter(|&&d| d).count()
    }

    if current == total {
        println!("\n\n  {BOLD}{GREEN}🎉 Congratulations! All {total} exercises passed!{RESET}");
        return;
    }

    terminal::enable_raw_mode().expect("Failed to enable terminal raw mode");

    let (fs_tx, fs_rx) = mpsc::channel::<()>();
    let mut watcher = RecommendedWatcher::new(
        move |res: Result<notify::Event, notify::Error>| {
            if let Ok(ev) = res {
                if matches!(ev.kind, EventKind::Modify(_) | EventKind::Create(_)) {
                    let _ = fs_tx.send(());
                }
            }
        },
        notify::Config::default(),
    )
    .expect("Failed to create file watcher");
    watcher
        .watch(Path::new("exercises"), RecursiveMode::Recursive)
        .ok();

    let mut needs_retest = true;
    let mut last_result: Option<TestResult> = None;
    let mut show_hint = false;
    let mut show_list = false;

    loop {
        // ── run test ──
        if needs_retest {
            show_hint = false;
            show_list = false;

            execute!(stdout, Clear(ClearType::All), cursor::MoveTo(0, 0)).unwrap();
            render_header(&mut stdout, exercises, current, count_done(&done));
            rprintln(&mut stdout, "");
            rprintln(
                &mut stdout,
                &format!(
                    "  {YELLOW}⏳ Testing {}...{RESET}",
                    exercises[current].package
                ),
            );
            stdout.flush().unwrap();

            let result = test_exercise(&exercises[current]);

            execute!(stdout, Clear(ClearType::All), cursor::MoveTo(0, 0)).unwrap();

            if result.passed {
                done[current] = true;
                render_header(&mut stdout, exercises, current, count_done(&done));
                rprintln(
                    &mut stdout,
                    &format!(
                        "\n  {BOLD}{GREEN}✅ Exercise '{}' passed!{RESET}",
                        exercises[current].name
                    ),
                );

                if let Some(next) = find_next_incomplete(&done, current) {
                    current = next;
                    rprintln(
                        &mut stdout,
                        &format!("\n  ➡  Auto-jump: {CYAN}{}{RESET}", exercises[current].name),
                    );
                    stdout.flush().unwrap();
                    std::thread::sleep(Duration::from_millis(800));
                    last_result = Some(result);
                    needs_retest = true;
                    continue;
                } else {
                    rprintln(&mut stdout, "");
                    rprintln(
                        &mut stdout,
                        &format!("  {BOLD}{GREEN}🎉 Congratulations! All {total} exercises passed!{RESET}"),
                    );
                    rprintln(&mut stdout, &format!("\n  Press {BOLD}q{RESET} to quit"));
                    stdout.flush().unwrap();
                    wait_for_quit();
                    break;
                }
            } else {
                render_header(&mut stdout, exercises, current, count_done(&done));
                render_failure(&mut stdout, &result);
            }

            render_controls(&mut stdout);
            stdout.flush().unwrap();
            last_result = Some(result);
            needs_retest = false;
        }

        // ── event loop ──
        if event::poll(Duration::from_millis(200)).unwrap() {
            if let Event::Key(key) = event::read().unwrap() {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => break,
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => break,
                    KeyCode::Char('h') => {
                        show_hint = !show_hint;
                        full_redraw(
                            &mut stdout,
                            exercises,
                            current,
                            count_done(&done),
                            &last_result,
                            show_hint,
                            false,
                            &done,
                        );
                    }
                    KeyCode::Char('l') => {
                        show_list = !show_list;
                        full_redraw(
                            &mut stdout,
                            exercises,
                            current,
                            count_done(&done),
                            &last_result,
                            show_hint,
                            show_list,
                            &done,
                        );
                    }
                    KeyCode::Char('n') => {
                        current = (current + 1) % total;
                        needs_retest = true;
                    }
                    KeyCode::Char('p') => {
                        current = if current > 0 { current - 1 } else { total - 1 };
                        needs_retest = true;
                    }
                    KeyCode::Char('r') | KeyCode::Enter => {
                        needs_retest = true;
                    }
                    _ => {}
                }
            }
        }

        if fs_rx.try_recv().is_ok() {
            while fs_rx.try_recv().is_ok() {}
            std::thread::sleep(Duration::from_millis(300));
            while fs_rx.try_recv().is_ok() {}
            needs_retest = true;
        }
    }

    terminal::disable_raw_mode().unwrap();
    execute!(stdout, cursor::MoveTo(0, 0), Clear(ClearType::All)).unwrap();
    println!("Goodbye! Keep up the good work 💪");
}

fn find_next_incomplete(done: &[bool], current: usize) -> Option<usize> {
    let n = done.len();
    for i in 1..=n {
        let idx = (current + i) % n;
        if !done[idx] {
            return Some(idx);
        }
    }
    None
}

fn wait_for_quit() {
    loop {
        if event::poll(Duration::from_millis(200)).unwrap() {
            if let Event::Key(key) = event::read().unwrap() {
                if matches!(key.code, KeyCode::Char('q') | KeyCode::Esc)
                    || (key.code == KeyCode::Char('c')
                        && key.modifiers.contains(KeyModifiers::CONTROL))
                {
                    return;
                }
            }
        }
    }
}

// ─────────────────────── rendering ───────────────────────

fn progress_bar(done: usize, total: usize, width: usize) -> String {
    let filled = if total > 0 { done * width / total } else { 0 };
    let empty = width - filled;
    let pct = if total > 0 { done * 100 / total } else { 0 };
    format!(
        "{GREEN}{}{}  {done}/{total} ({pct}%){RESET}",
        "█".repeat(filled),
        "░".repeat(empty),
    )
}

fn render_header(out: &mut impl Write, exercises: &[Exercise], current: usize, done: usize) {
    let total = exercises.len();
    let ex = &exercises[current];
    let bar = progress_bar(done, total, 20);

    rprintln(
        out,
        &format!("{BOLD}{BLUE}─── OS Camp ─── Rust & OS Advanced Experiments ───{RESET}"),
    );
    rprintln(out, &format!("  Progress: {bar}"));
    rprintln(out, "");
    rprintln(
        out,
        &format!(
            "  {BOLD}▶ Exercise {}/{total}: {}{RESET}",
            current + 1,
            ex.name
        ),
    );
    rprintln(out, &format!("    {DIM}Module:{RESET} {}", ex.module));
    rprintln(out, &format!("    {CYAN}{}{RESET}", ex.description));
    rprintln(out, &format!("    {DIM}📄 {}{RESET}", ex.path));
}

fn render_failure(out: &mut impl Write, result: &TestResult) {
    rprintln(out, &format!("\n  {BOLD}{RED}❌ Test failed{RESET}\n"));

    let lines: Vec<&str> = result.output.lines().collect();
    let max_lines = 30;
    let start = lines.len().saturating_sub(max_lines);

    if start > 0 {
        rprintln(out, &format!("  {DIM}... omitted {start} lines ...{RESET}"));
    }
    for line in &lines[start..] {
        rprintln(out, &format!("  {line}"));
    }
}

fn render_controls(out: &mut impl Write) {
    rprintln(out, "");
    rprintln(
        out,
        &format!("{DIM}  ─────────────────────────────────────────{RESET}"),
    );
    rprintln(
        out,
        &format!(
            "  {BOLD}h{RESET} hint  {BOLD}l{RESET} list  \
             {BOLD}n{RESET}/{BOLD}p{RESET} prev/next  \
             {BOLD}r{RESET} retest  {BOLD}q{RESET} quit"
        ),
    );
    rprintln(
        out,
        &format!("  {DIM}📡 Watching for file changes, automatically retests after saving{RESET}"),
    );
}

fn render_hint(out: &mut impl Write, ex: &Exercise) {
    rprintln(out, &format!("\n  {BOLD}{YELLOW}💡 Hint:{RESET}"));
    for line in ex.hint.lines() {
        rprintln(out, &format!("  {YELLOW}{line}{RESET}"));
    }
}

fn render_list(out: &mut impl Write, exercises: &[Exercise], current: usize, done: &[bool]) {
    rprintln(out, &format!("\n  {BOLD}{BLUE}Exercise list:{RESET}\n"));

    let mut cur_module = String::new();
    for (i, ex) in exercises.iter().enumerate() {
        if ex.module != cur_module {
            cur_module.clone_from(&ex.module);
            rprintln(out, &format!("  {YELLOW}[{cur_module}]{RESET}"));
        }
        let marker = if i == current { "▶" } else { " " };
        let status = if done[i] {
            format!("{GREEN}✅{RESET}")
        } else {
            format!("{RED}  {RESET}")
        };
        rprintln(
            out,
            &format!(
                "  {marker} {status} {:2}. {:<22} ({DIM}{}{RESET})",
                i + 1,
                ex.name,
                ex.package
            ),
        );
    }
}

fn full_redraw(
    out: &mut impl Write,
    exercises: &[Exercise],
    current: usize,
    done_n: usize,
    result: &Option<TestResult>,
    show_hint: bool,
    show_list: bool,
    done: &[bool],
) {
    execute!(io::stdout(), Clear(ClearType::All), cursor::MoveTo(0, 0)).unwrap();

    if show_list {
        render_header(out, exercises, current, done_n);
        render_list(out, exercises, current, done);
    } else {
        render_header(out, exercises, current, done_n);
        if let Some(ref r) = result {
            if r.passed {
                rprintln(out, &format!("\n  {BOLD}{GREEN}✅ Test passed!{RESET}"));
            } else {
                render_failure(out, r);
            }
        }
        if show_hint {
            render_hint(out, &exercises[current]);
        }
    }
    render_controls(out);
    out.flush().unwrap();
}

// ─────────────────────── other modes ───────────────────────

fn list_mode(exercises: &[Exercise]) {
    println!("{BOLD}{BLUE}OS Camp - Exercise list{RESET}\n");

    let mut cur_module = String::new();
    let mut done = 0;

    for (i, ex) in exercises.iter().enumerate() {
        if ex.module != cur_module {
            cur_module.clone_from(&ex.module);
            println!("\n  {YELLOW}[{cur_module}]{RESET}");
        }
        let passed = test_quiet(ex);
        if passed {
            done += 1;
        }
        let status = if passed {
            format!("{GREEN}✅{RESET}")
        } else {
            format!("{RED}❌{RESET}")
        };
        println!(
            "  {status} {:2}. {:<22} ({DIM}cargo test -p {}{RESET})",
            i + 1,
            ex.name,
            ex.package
        );
    }

    let total = exercises.len();
    let bar = progress_bar(done, total, 20);
    println!("\n  Progress: {bar}\n");
}

fn check_mode(exercises: &[Exercise]) {
    println!("{BOLD}{BLUE}OS Camp - Check all exercises{RESET}\n");

    let total = exercises.len();
    let mut done = 0;

    for (i, ex) in exercises.iter().enumerate() {
        print!("  [{:2}/{total}] {:<22} ", i + 1, ex.name);
        io::stdout().flush().unwrap();
        if test_quiet(ex) {
            done += 1;
            println!("{GREEN}✅ PASS{RESET}");
        } else {
            println!("{RED}❌ FAIL{RESET}");
        }
    }

    println!("\n  {BOLD}Result: {done}/{total} passed{RESET}");
    if done == total {
        println!("  {GREEN}🎉 All passed!{RESET}");
    }
}

fn run_mode(exercises: &[Exercise], name: Option<&String>) {
    let name = name.unwrap_or_else(|| {
        eprintln!("Usage: oscamp run <package>");
        std::process::exit(1);
    });
    let ex = find_exercise(exercises, name);

    println!("{BOLD}▶ {} - {}{RESET}", ex.name, ex.description);
    println!("  📄 {}\n", ex.path);

    let result = test_exercise(ex);
    print!("{}", result.output);

    if result.passed {
        println!("\n{BOLD}{GREEN}✅ Test passed!{RESET}");
    } else {
        println!("\n{BOLD}{RED}❌ Test failed{RESET}");
        println!("  💡 Use 'oscamp hint {name}' to view hint");
    }
}

fn hint_mode(exercises: &[Exercise], name: Option<&String>) {
    let name = name.unwrap_or_else(|| {
        eprintln!("Usage: oscamp hint <package>");
        std::process::exit(1);
    });
    let ex = find_exercise(exercises, name);
    println!("{BOLD}{YELLOW}💡 {} - Hint:{RESET}\n", ex.name);
    println!("{}", ex.hint);
}

fn find_exercise<'a>(exercises: &'a [Exercise], name: &str) -> &'a Exercise {
    exercises
        .iter()
        .find(|e| e.package == name)
        .unwrap_or_else(|| {
            eprintln!("Exercise not found: {name}");
            eprintln!("Use 'oscamp list' to see all exercises");
            std::process::exit(1);
        })
}

fn print_usage() {
    println!("{BOLD}{BLUE}OS Camp{RESET} - Rust & OS Advanced Experiments\n");
    println!("Usage: oscamp [command]\n");
    println!("Commands:");
    println!(
        "  {BOLD}watch{RESET}    Interactive exercise mode (default) - real-time file monitoring"
    );
    println!("  {BOLD}list{RESET}     View completion status of all exercises");
    println!("  {BOLD}check{RESET}    Check all exercises in batch");
    println!("  {BOLD}run{RESET}      Run specified exercise  (oscamp run <package>)");
    println!("  {BOLD}hint{RESET}     View exercise hint  (oscamp hint <package>)");
    println!("  {BOLD}help{RESET}     Show this help message");
}
