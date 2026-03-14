use std::fs;
use std::io::{self, Read, Write};
use std::process::Command;

fn paint(text: &str, hex: &str) -> String {
    let hex = hex.trim_start_matches('#');
    if hex.len() < 6 { return text.to_string(); }
    let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(255);
    let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(255);
    let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(255);
    format!("\x1b[38;2;{};{};{}m{}\x1b[0m", r, g, b, text)
}

fn get_cursor_row() -> usize {
    let Ok(mut tty) = std::fs::OpenOptions::new().read(true).write(true).open("/dev/tty") else {
        return 1;
    };
    use std::os::unix::io::AsRawFd;
    let fd = tty.as_raw_fd();
    unsafe {
        let mut t: libc::termios = std::mem::zeroed();
        libc::tcgetattr(fd, &mut t);
        let mut raw = t;
        libc::cfmakeraw(&mut raw);
        libc::tcsetattr(fd, libc::TCSANOW, &raw);
        write!(tty, "\x1b[6n").ok();
        tty.flush().ok();
        let mut buf = Vec::new();
        let mut b = [0u8; 1];
        loop {
            if tty.read(&mut b).unwrap_or(0) == 0 { break; }
            buf.push(b[0]);
            if b[0] == b'R' { break; }
        }
        libc::tcsetattr(fd, libc::TCSANOW, &t);
        String::from_utf8(buf).ok()
            .and_then(|s| {
                let s = s.trim_start_matches('\x1b').trim_start_matches('[').trim_end_matches('R');
                s.splitn(2, ';').next()?.parse().ok()
            })
            .unwrap_or(1)
    }
}

/// Versionstring aus Command-Output extrahieren – erste Zeile, erstes "x.y.z"-Token
fn cmd_version(bin: &str, args: &[&str]) -> String {
    Command::new(bin).args(args).output().ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .or_else(|| {
            Command::new(bin).args(args).output().ok()
                .and_then(|o| String::from_utf8(o.stderr).ok())
        })
        .and_then(|s| {
            s.split_whitespace()
                .find(|t| t.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false))
                .map(|v| v.trim_end_matches(',').to_string())
        })
        .unwrap_or_else(|| "?".to_string())
}

/// Terminal ermitteln: Umgebungsvariablen → Prozessbaum-Fallback
fn detect_terminal() -> String {
    // Kitty setzt TERM=xterm-kitty, andere setzen TERM_PROGRAM
    if let Ok(t) = std::env::var("TERM_PROGRAM") {
        if !t.is_empty() { return t; }
    }
    if let Ok(t) = std::env::var("TERM") {
        if t == "xterm-kitty" { return "kitty".to_string(); }
    }
    // Fallback: Prozessbaum – Elternprozess des Elternprozesses
    if let Ok(ppid) = fs::read_to_string("/proc/self/status")
        .map(|s| s.lines()
            .find(|l| l.starts_with("PPid:"))
            .and_then(|l| l.split_whitespace().nth(1)?.parse::<u32>().ok())
            .unwrap_or(0))
    {
        if ppid > 0 {
            if let Ok(grandppid) = fs::read_to_string(format!("/proc/{}/status", ppid))
                .map(|s| s.lines()
                    .find(|l| l.starts_with("PPid:"))
                    .and_then(|l| l.split_whitespace().nth(1)?.parse::<u32>().ok())
                    .unwrap_or(0))
            {
                if grandppid > 0 {
                    if let Ok(comm) = fs::read_to_string(format!("/proc/{}/comm", grandppid)) {
                        return comm.trim().to_string();
                    }
                }
            }
        }
    }
    "unknown".to_string()
}

fn main() {
    let home = std::env::var("HOME").unwrap_or_default();

    let user     = std::env::var("USER").unwrap_or_else(|_| "mjo".into());
    let host     = fs::read_to_string("/proc/sys/kernel/hostname")
        .map(|s| s.trim().to_string()).unwrap_or_default();
    let os       = fs::read_to_string("/etc/os-release").unwrap_or_default()
        .lines().find(|l| l.starts_with("PRETTY_NAME="))
        .map(|l| l[13..].replace('"', "")).unwrap_or_default();
    let colors: Vec<String> = fs::read_to_string(format!("{}/.cache/wal/colors", home))
        .map(|c| c.lines().take(16).map(|s| s.to_string()).collect())
        .unwrap_or_else(|_| vec!["#ffffff".to_string(); 16]);
    let sys_pkgs  = fs::read_dir("/nix/var/nix/gcroots/auto").map(|d| d.count()).unwrap_or(0);
    let user_pkgs = fs::read_dir(format!("{}/.nix-profile/bin", home)).map(|d| d.count()).unwrap_or(0);

    let wm       = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_else(|_| "WM".into());
    let wm_ver   = cmd_version("hyprctl", &["version"]);

    let shell    = std::env::var("SHELL").unwrap_or_default()
        .split('/').last().unwrap_or("shell").to_string();
    let shell_ver = cmd_version("fish", &["--version"]);

    let terminal = detect_terminal();
    let term_ver = if terminal.to_lowercase().contains("kitty") {
    cmd_version("kitty", &["--version"])
    } else {
        String::new()
    };
    let c1    = &colors[1];
    let c2    = &colors[2];
    let c3    = &colors[3];
    let arrow = paint(">", &colors[2]);

    let lines: Vec<String> = vec![
        "              ".to_string(),
        format!("{}{}{}", paint(&user, c1),paint("@", c2), paint(&host, c3)),
        "              ".to_string(),
        format!("{}  {}", arrow, os),
        format!("{}  {} (system), {} (user)", arrow, sys_pkgs, user_pkgs),
        format!("{}  {} {}", arrow, wm, wm_ver),
        format!("{}  {} {}", arrow, shell, shell_ver),
        format!("{}  {} {}", arrow, terminal,term_ver),
        String::new(),
        format!("{}  {} {}", arrow,
            paint(env!("CARGO_PKG_NAME"), c1),
            paint(env!("CARGO_PKG_VERSION"),c1)),
        String::new(),
        {
            let mut dots = String::new();
            for i in 1..7 {
                dots.push_str(&format!("{}{}  ",
                    paint("", &colors[i]),
                    paint(" ", &colors[i])));
            }
            dots
        },
    ];

    let img_w: usize = 32;
    let img_h: usize = lines.len() + 1; // Höhe dynamisch an Zeilenanzahl anpassen
    let text_col = img_w + 2;

    // 1. Platz reservieren → Terminal scrollt jetzt falls nötig
    for _ in 0..img_h { println!(); }
    io::stdout().flush().unwrap();

    // 2. Position nach dem Scroll abfragen und zurückrechnen
    let bottom_row = get_cursor_row();
    let start_row  = bottom_row.saturating_sub(img_h);

    // 3. Bild platzieren
    let place      = format!("{}x{}@1x{}", img_w, img_h, start_row);
    let image_path = format!("{}/dotfiles/flakes/Yuki.jpg", home);
    let _ = Command::new("kitten")
        .args(["icat", "--silent", "--transfer-mode", "memory",
               "--place", &place, "--align", "left", &image_path])
        .status();

    // 4. Text rechts vom Bild
    for (i, line) in lines.iter().enumerate() {
        print!("\x1b[{};1H", start_row + i);
        print!("\x1b[{}G{}", text_col, line);
    }

    // 5. Cursor unter den Block
    print!("\x1b[{};1H", start_row + img_h);
    println!();
    io::stdout().flush().unwrap();
}
