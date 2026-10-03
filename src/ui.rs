//! What things look like, and nothing else.
//!
//! The wordmark, the colours and the few lines that say which build this is.
//! No scanning logic lives here and nothing in this module decides whether
//! something is infected, so it can be read in a minute and skipped in an
//! audit. It runs no command and reads no file. ADR-0014 set that rule for
//! the shell's `ui/`; it holds here.
//!
//! Two properties matter and both are tested:
//!
//! - **Colour adds colour and nothing else.** [`Ui::paint`] wraps text in
//!   escape codes and never changes a character of it. Strip the codes and the
//!   output is byte for byte what a pipe or a report file gets.
//! - **Colour means something.** In a report, red is a confirmed finding and
//!   nothing else. The fewer colours in play, the more each one says.
//!
//! Honoured without being asked: `NO_COLOR` (any value), `TERM=dumb`, output
//! that is not a terminal, and `PRC_ASCII` for a terminal with no font for
//! the wordmark.

use std::io::IsTerminal;

/// The wordmark is this many columns wide.
pub const BANNER_WIDTH: usize = 75;

const AUTHOR: &str = "Mandeep Singh";
const AUTHOR_URL: &str = "https://github.com/meSingh";

const RESET: &str = "\x1b[0m";
const BOLD: &str = "\x1b[1m";
const DIM: &str = "\x1b[2m";
const RED: &str = "\x1b[31m";
const GREEN: &str = "\x1b[32m";
const YELLOW: &str = "\x1b[33m";
const BLUE: &str = "\x1b[34m";
const CYAN: &str = "\x1b[36m";
const GREY: &str = "\x1b[90m";

const WORDMARK_TOP: [&str; 3] = [
    "  ██████╗  ██████╗ ██╗     ██╗███╗   ██╗██████╗ ██╗██████╗ ███████╗██████╗ ",
    "  ██╔══██╗██╔═══██╗██║     ██║████╗  ██║██╔══██╗██║██╔══██╗██╔════╝██╔══██╗",
    "  ██████╔╝██║   ██║██║     ██║██╔██╗ ██║██████╔╝██║██║  ██║█████╗  ██████╔╝",
];
const WORDMARK_BOTTOM: [&str; 3] = [
    "  ██╔═══╝ ██║   ██║██║     ██║██║╚██╗██║██╔══██╗██║██║  ██║██╔══╝  ██╔══██╗",
    "  ██║     ╚██████╔╝███████╗██║██║ ╚████║██║  ██║██║██████╔╝███████╗██║  ██║",
    "  ╚═╝      ╚═════╝ ╚══════╝╚═╝╚═╝  ╚═══╝╚═╝  ╚═╝╚═╝╚═════╝ ╚══════╝╚═╝  ╚═╝",
];

/// What this output can show.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ui {
    pub color: bool,
    pub unicode: bool,
}

impl Ui {
    /// Nothing but the text. What a pipe, a report file and a test get.
    pub const fn plain() -> Self {
        Self {
            color: false,
            unicode: false,
        }
    }

    /// What standard output can take, worked out from the environment.
    pub fn for_stdout() -> Self {
        let var = |name: &str| std::env::var(name).unwrap_or_default();
        let locale = [var("LC_ALL"), var("LC_CTYPE"), var("LANG")]
            .into_iter()
            .find(|v| !v.is_empty())
            .unwrap_or_default()
            .to_ascii_lowercase();
        Self {
            color: std::io::stdout().is_terminal()
                && std::env::var_os("NO_COLOR").is_none()
                && var("TERM") != "dumb",
            unicode: std::env::var_os("PRC_ASCII").is_none()
                && (locale.contains("utf-8") || locale.contains("utf8")),
        }
    }

    fn wrap(&self, codes: &[&str], text: &str) -> String {
        if self.color && !text.is_empty() {
            format!("{}{text}{RESET}", codes.concat())
        } else {
            text.to_string()
        }
    }

    pub fn bold(&self, text: &str) -> String {
        self.wrap(&[BOLD], text)
    }

    pub fn dim(&self, text: &str) -> String {
        self.wrap(&[DIM], text)
    }

    /// The colour of a value worth reading first: a version, a count.
    pub fn accent(&self, text: &str) -> String {
        self.wrap(&[CYAN, BOLD], text)
    }

    /// Red, outside a report: the one thing as bad as a finding, a tool that
    /// cannot scan at all.
    pub fn alarm(&self, text: &str) -> String {
        self.wrap(&[RED, BOLD], text)
    }

    /// The wordmark, the signature and one line saying which build this is.
    /// Printed at the top of every run, so a screenshot or a pasted log says
    /// what produced it.
    pub fn banner(&self, version: &str) -> String {
        let mut out = String::new();
        if self.unicode {
            for line in WORDMARK_TOP {
                out.push_str(&self.wrap(&[CYAN], line));
                out.push('\n');
            }
            for line in WORDMARK_BOTTOM {
                out.push_str(&self.wrap(&[BLUE], line));
                out.push('\n');
            }
        } else {
            out.push_str(&format!("\n  {}\n", self.accent("POLINRIDER")));
        }

        // The credit, right-aligned to the wordmark's edge so it reads as a
        // signature under it. A link where the terminal can show one.
        let credit = format!("by {AUTHOR}");
        let pad = BANNER_WIDTH.saturating_sub(credit.chars().count()).max(2);
        let credit = if self.color {
            format!("\x1b]8;;{AUTHOR_URL}\x1b\\{credit}\x1b]8;;\x1b\\")
        } else {
            credit
        };
        out.push_str(&format!("{}{}\n", " ".repeat(pad), self.dim(&credit)));

        out.push_str(&format!(
            "\n  {} {}  {}\n",
            self.bold("cleaner"),
            self.accent(version),
            self.dim("read-only until you say otherwise")
        ));
        out
    }

    /// One labelled fact, for `--version`: the label quiet, the value loud.
    pub fn fact(&self, label: &str, value: &str) -> String {
        format!("  {}  {value}\n", self.dim(&format!("{label:<10}")))
    }

    /// Colour rendered output by what each line means. Understands the lines
    /// this tool prints and leaves anything else exactly as it is: unknown
    /// text never borrows the colour of a finding.
    pub fn paint(&self, text: &str) -> String {
        if !self.color {
            return text.to_string();
        }
        let mut out = String::with_capacity(text.len() + text.len() / 8);
        for piece in text.split_inclusive('\n') {
            let (line, newline) = match piece.strip_suffix('\n') {
                Some(line) => (line, "\n"),
                None => (piece, ""),
            };
            out.push_str(&self.paint_line(line));
            out.push_str(newline);
        }
        out
    }

    fn paint_line(&self, line: &str) -> String {
        let tagged = |tag: &str| {
            line.find(tag)
                .map(|at| line.split_at(at + tag.len()))
                .map(|(head, rest)| (head.to_string(), rest.to_string()))
        };
        if line.contains("[HIT]") {
            // The whole line. A confirmed finding is never half-coloured.
            return self.wrap(&[RED, BOLD], line);
        }
        if let Some((head, rest)) = tagged("[review]") {
            return format!("{}{rest}", self.wrap(&[YELLOW, BOLD], &head));
        }
        if let Some((head, rest)) = tagged("[ok]") {
            return format!("{}{rest}", self.wrap(&[GREEN], &head));
        }
        if line.contains("[info]") {
            return self.wrap(&[GREY], line);
        }
        if line.starts_with("  ##") || line.contains("VERDICT: COMPROMISED") {
            return self.wrap(&[RED, BOLD], line);
        }
        if line.starts_with("VERDICT: clean") {
            return self.wrap(&[GREEN, BOLD], line);
        }
        if line.starts_with("VERDICT:") {
            return self.wrap(&[YELLOW, BOLD], line);
        }
        // The opening: what kind of run this is, before anything else.
        if line.starts_with("  DRY RUN") {
            return self.wrap(&[GREEN, BOLD], line);
        }
        if line.starts_with("  APPLY") {
            return self.wrap(&[YELLOW, BOLD], line);
        }
        if line.starts_with("  Detected a ") {
            return self.wrap(&[CYAN, BOLD], line);
        }
        if let Some(rest) = line
            .strip_prefix("  directories  ")
            .or_else(|| line.strip_prefix("  host state   "))
        {
            let label = line.get(..line.len() - rest.len()).unwrap_or_default();
            return format!("{}{rest}", self.wrap(&[DIM], label));
        }
        if (line.starts_with("== ") && line.ends_with(" =="))
            || line.starts_with("PolinRider ")
            || line.starts_with("Step ")
        {
            return self.wrap(&[BOLD], line);
        }
        if line.contains("QUARANTINE FAILED") || line.contains("STRIP FAILED") {
            // Not red: red is the finding. This is the tool failing to deal
            // with one, which needs a person.
            return self.wrap(&[YELLOW, BOLD], line);
        }
        line.to_string()
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    const COLOUR: Ui = Ui {
        color: true,
        unicode: true,
    };

    /// Remove SGR colour codes and OSC 8 links, leaving what was wrapped.
    fn strip(text: &str) -> String {
        let mut out = String::new();
        let mut chars = text.chars().peekable();
        while let Some(c) = chars.next() {
            if c != '\x1b' {
                out.push(c);
                continue;
            }
            match chars.next() {
                // CSI: ends at the first letter.
                Some('[') => {
                    for d in chars.by_ref() {
                        if d.is_ascii_alphabetic() {
                            break;
                        }
                    }
                }
                // OSC: ends at ESC backslash.
                Some(']') => {
                    while let Some(d) = chars.next() {
                        if d == '\x1b' && chars.peek() == Some(&'\\') {
                            chars.next();
                            break;
                        }
                    }
                }
                _ => {}
            }
        }
        out
    }

    const REPORT: &str = "PolinRider local check\n\n  Detected a Linux system.\n  DRY RUN, read-only. No changes will be made at this stage.\n  APPLY. Confirmed artifacts will be moved to quarantine. Nothing is deleted.\n\n  directories  /home/x/code\n  host state   this machine (linux)\n\n== Second-stage implant ==\n    4242 implant\n  [HIT]    an implant process is running now. Kill it before anything else:\n           kill -9 4242\n  [review] user crontab is not empty, review every line:\n  [ok]     no ~/.npmrc\n  [info]   3 units changed\n\n== RESULT ==\n  ##   VERDICT: COMPROMISED   ##\nVERDICT: clean against the current indicator set.\nVERDICT: no confirmed indicator.\nsomething this module has never seen\n";

    #[test]
    fn without_colour_the_text_is_untouched() {
        // What a pipe, a report file and the conformance corpus all get.
        assert_eq!(Ui::plain().paint(REPORT), REPORT);
        assert!(!Ui::plain().banner("2.0.0").contains('\x1b'));
        assert!(!Ui::plain().fact("version", "2.0.0").contains('\x1b'));
    }

    #[test]
    fn colour_adds_colour_and_changes_no_character() {
        let painted = COLOUR.paint(REPORT);
        assert_ne!(painted, REPORT, "something should have been coloured");
        assert_eq!(strip(&painted), REPORT);
        // Without a trailing newline too: nothing is added at the end.
        assert_eq!(strip(&COLOUR.paint("  [ok]     fine")), "  [ok]     fine");
    }

    #[test]
    fn red_is_a_confirmed_finding_and_nothing_else() {
        for line in COLOUR.paint(REPORT).lines() {
            let plain = strip(line);
            let confirmed = plain.contains("[HIT]") || plain.contains("COMPROMISED");
            assert_eq!(line.contains(RED), confirmed, "{plain}");
        }
        // Text nobody taught it stays as it is.
        assert_eq!(
            COLOUR.paint("something this module has never seen"),
            "something this module has never seen"
        );
    }

    #[test]
    fn the_banner_names_the_build_and_fits_its_width() {
        let banner = strip(&COLOUR.banner("2.0.0-beta.1 (abc1234)"));
        assert!(banner.contains("cleaner 2.0.0-beta.1 (abc1234)"));
        assert!(banner.contains("by Mandeep Singh"));
        for line in banner.lines().take(6) {
            assert_eq!(line.chars().count(), BANNER_WIDTH, "{line}");
        }
        let credit = banner.lines().nth(6).expect("the credit line");
        assert_eq!(
            credit.chars().count(),
            BANNER_WIDTH,
            "right-aligned to the wordmark"
        );
    }

    #[test]
    fn a_terminal_without_the_font_gets_the_name_in_letters() {
        let banner = Ui::plain().banner("2.0.0");
        assert!(banner.contains("POLINRIDER"));
        assert!(banner.is_ascii(), "{banner}");
    }
}
