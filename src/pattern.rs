//! A small regular-expression matcher, for the indicator files that are
//! written as patterns.
//!
//! `ioc/filenames.txt` holds extended regular expressions matched against a
//! path: `(^|/)temp_auto_push\.bat$`. The shell hands them to `grep -E`. This
//! crate has no dependencies, and the file is data that the weekly review
//! edits, so the patterns cannot be turned into code without the two drifting
//! apart. Hence this: the subset of ERE those patterns use, written out.
//!
//! Supported: literals, `.`, `\` escapes, `^` and `$`, groups with `|`,
//! bracket classes with ranges and negation, and the quantifiers `?`, `*`,
//! `+`, `{n}`, `{n,}` and `{n,m}`. Anything else is a parse error, and a pattern that does not parse
//! stops the scan. A pattern that silently matched nothing would be an
//! indicator that silently stopped working.

/// One compiled pattern. Matching is unanchored unless the pattern anchors
/// itself, as with `grep -E`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pattern {
    source: String,
    root: Node,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Node {
    /// One of several sequences.
    Alt(Vec<Vec<Node>>),
    Char(char),
    Any,
    Class {
        negated: bool,
        items: Vec<(char, char)>,
    },
    Start,
    End,
    Repeat {
        node: Box<Node>,
        min: usize,
        max: Option<usize>,
    },
}

/// Why a pattern could not be used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BadPattern {
    pub pattern: String,
    pub why: &'static str,
}

impl std::fmt::Display for BadPattern {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "cannot use the pattern '{}': {}", self.pattern, self.why)
    }
}

struct Parser<'a> {
    chars: std::iter::Peekable<std::str::Chars<'a>>,
}

impl Parser<'_> {
    fn alternatives(&mut self, in_group: bool) -> Result<Node, &'static str> {
        let mut branches = vec![Vec::new()];
        loop {
            match self.chars.peek().copied() {
                None => {
                    if in_group {
                        return Err("a group is opened and never closed");
                    }
                    break;
                }
                Some(')') => {
                    if !in_group {
                        return Err("a ) with no group to close");
                    }
                    self.chars.next();
                    break;
                }
                Some('|') => {
                    self.chars.next();
                    branches.push(Vec::new());
                }
                Some(_) => {
                    let atom = self.atom()?;
                    let atom = self.quantified(atom)?;
                    if let Some(branch) = branches.last_mut() {
                        branch.push(atom);
                    }
                }
            }
        }
        Ok(Node::Alt(branches))
    }

    fn atom(&mut self) -> Result<Node, &'static str> {
        match self.chars.next() {
            Some('(') => self.alternatives(true),
            Some('[') => self.class(),
            Some('.') => Ok(Node::Any),
            Some('^') => Ok(Node::Start),
            Some('$') => Ok(Node::End),
            Some('\\') => match self.chars.next() {
                Some(c) if c.is_ascii_alphanumeric() => {
                    // \d, \w, \b and friends are not ERE and mean different
                    // things in different tools. Refused, not guessed at.
                    Err("a backslash before a letter or digit is not supported")
                }
                Some(c) => Ok(Node::Char(c)),
                None => Err("a backslash at the end"),
            },
            Some('?' | '*' | '+') => Err("a quantifier with nothing before it"),
            Some('{') => Err("a quantifier with nothing before it"),
            Some(c) => Ok(Node::Char(c)),
            None => Err("the pattern ends where something was expected"),
        }
    }

    fn class(&mut self) -> Result<Node, &'static str> {
        let negated = self.chars.peek() == Some(&'^');
        if negated {
            self.chars.next();
        }
        let mut items = Vec::new();
        loop {
            let c = match self.chars.next() {
                None => return Err("a [ is opened and never closed"),
                Some(']') if !items.is_empty() => break,
                Some('[') => return Err("named classes like [:alpha:] are not supported"),
                Some(c) => c,
            };
            // a-z, unless the dash is last.
            let mut ahead = self.chars.clone();
            if ahead.next() == Some('-') && ahead.peek().is_some_and(|n| *n != ']') {
                self.chars.next();
                match self.chars.next() {
                    Some(end) if end >= c => items.push((c, end)),
                    _ => return Err("a range in [ ] runs backwards"),
                }
            } else {
                items.push((c, c));
            }
        }
        Ok(Node::Class { negated, items })
    }

    fn quantified(&mut self, atom: Node) -> Result<Node, &'static str> {
        let (min, max) = match self.chars.peek() {
            Some('?') => (0, Some(1)),
            Some('*') => (0, None),
            Some('+') => (1, None),
            Some('{') => (0, Some(0)),
            _ => return Ok(atom),
        };
        let (min, max) = if self.chars.next() == Some('{') {
            self.counted()?
        } else {
            (min, max)
        };
        if matches!(atom, Node::Start | Node::End) {
            return Err("a quantifier on ^ or $");
        }
        Ok(Node::Repeat {
            node: Box::new(atom),
            min,
            max,
        })
    }

    /// The inside of `{n}`, `{n,}` or `{n,m}`, after the `{`. Spaces, a
    /// missing lower bound and a bound that runs backwards are refused rather
    /// than read the way one tool or another happens to.
    fn counted(&mut self) -> Result<(usize, Option<usize>), &'static str> {
        let min = self.number()?.ok_or("a { with no count after it")?;
        let max = match self.chars.next() {
            Some('}') => return Ok((min, Some(min))),
            Some(',') => self.number()?,
            _ => return Err("a { that is not {n}, {n,} or {n,m}"),
        };
        if self.chars.next() != Some('}') {
            return Err("a { that is not {n}, {n,} or {n,m}");
        }
        if max.is_some_and(|m| m < min) {
            return Err("a count in { } runs backwards");
        }
        Ok((min, max))
    }

    fn number(&mut self) -> Result<Option<usize>, &'static str> {
        let mut digits = String::new();
        while let Some(c) = self.chars.peek().copied().filter(char::is_ascii_digit) {
            digits.push(c);
            self.chars.next();
        }
        if digits.is_empty() {
            return Ok(None);
        }
        // A count this large is a typo, and backtracking over it would not end.
        match digits.parse::<usize>() {
            Ok(n) if n <= 255 => Ok(Some(n)),
            _ => Err("a count in { } over 255"),
        }
    }
}

impl Pattern {
    pub fn parse(source: &str) -> Result<Self, BadPattern> {
        let mut parser = Parser {
            chars: source.chars().peekable(),
        };
        match parser.alternatives(false) {
            Ok(root) => Ok(Self {
                source: source.to_string(),
                root,
            }),
            Err(why) => Err(BadPattern {
                pattern: source.to_string(),
                why,
            }),
        }
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    /// Does the pattern match anywhere in `text`?
    pub fn is_match(&self, text: &str) -> bool {
        let chars: Vec<char> = text.chars().collect();
        (0..=chars.len()).any(|start| {
            let mut found = false;
            walk(std::slice::from_ref(&self.root), &chars, start, &mut |_| {
                found = true;
                true
            });
            found
        })
    }
}

/// Match `nodes` in sequence from `at`, calling `then` with each position the
/// sequence can end at until it returns true. Backtracking, which is fine for
/// patterns a person wrote into an indicator file and a path as the input.
fn walk(nodes: &[Node], text: &[char], at: usize, then: &mut dyn FnMut(usize) -> bool) -> bool {
    let Some((first, rest)) = nodes.split_first() else {
        return then(at);
    };
    match first {
        Node::Char(c) => text.get(at) == Some(c) && walk(rest, text, at + 1, then),
        Node::Any => at < text.len() && walk(rest, text, at + 1, then),
        Node::Class { negated, items } => {
            text.get(at)
                .is_some_and(|c| items.iter().any(|(lo, hi)| (*lo..=*hi).contains(c)) != *negated)
                && walk(rest, text, at + 1, then)
        }
        Node::Start => at == 0 && walk(rest, text, at, then),
        Node::End => at == text.len() && walk(rest, text, at, then),
        Node::Alt(branches) => branches
            .iter()
            .any(|branch| walk(branch, text, at, &mut |end| walk(rest, text, end, then))),
        Node::Repeat { node, min, max } => repeat(node, *min, *max, rest, text, at, 0, then),
    }
}

#[allow(clippy::too_many_arguments)]
fn repeat(
    node: &Node,
    min: usize,
    max: Option<usize>,
    rest: &[Node],
    text: &[char],
    at: usize,
    done: usize,
    then: &mut dyn FnMut(usize) -> bool,
) -> bool {
    // Greedy: one more if allowed, and only then the rest.
    if max.is_none_or(|m| done < m)
        && walk(std::slice::from_ref(node), text, at, &mut |end| {
            // A repetition that consumed nothing would loop for ever.
            end > at && repeat(node, min, max, rest, text, end, done + 1, then)
        })
    {
        return true;
    }
    done >= min && walk(rest, text, at, then)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn m(pattern: &str, text: &str) -> bool {
        Pattern::parse(pattern).expect("parses").is_match(text)
    }

    #[test]
    fn the_patterns_in_the_indicator_file_match_what_grep_matches() {
        // The three shapes ioc/filenames.txt uses, with synthetic names.
        let bat = r"(^|/)temp_helper\.bat$";
        assert!(m(bat, "temp_helper.bat"));
        assert!(m(bat, "scripts/win/temp_helper.bat"));
        assert!(!m(bat, "not_temp_helper.bat"));
        assert!(!m(bat, "temp_helper.bat.txt"));
        assert!(!m(bat, "temp_helperXbat"), "an escaped dot is a dot");

        let bin = r"(^|/)ImplantName(\.exe|-win\.exe|-linux|-darwin-(x64|arm64))?$";
        for name in [
            "ImplantName",
            "bin/ImplantName.exe",
            "ImplantName-win.exe",
            "a/b/ImplantName-linux",
            "ImplantName-darwin-x64",
            "ImplantName-darwin-arm64",
        ] {
            assert!(m(bin, name), "{name}");
        }
        for name in [
            "ImplantName-darwin",
            "ImplantName.txt",
            "MyImplantName",
            "ImplantNameX",
        ] {
            assert!(!m(bin, name), "{name}");
        }
    }

    #[test]
    fn unanchored_patterns_match_anywhere_as_grep_does() {
        assert!(m("fonts", "public/fonts/a.woff2"));
        assert!(m(r"\.md$", "docs/README.md"));
        assert!(!m(r"\.md$", "docs/README.mdx"));
        assert!(m("(^|/)docs/", "docs/a"));
        assert!(m("(^|/)docs/", "x/docs/a"));
        assert!(!m("(^|/)docs/", "mydocs/a"));
    }

    #[test]
    fn classes_and_repetition() {
        assert!(m(
            r"(^|/)\.github/workflows/[^/]*scan[^/]*\.(yml|yaml)$",
            ".github/workflows/my-scan-v2.yml"
        ));
        assert!(!m(
            r"(^|/)\.github/workflows/[^/]*scan[^/]*\.(yml|yaml)$",
            ".github/workflows/sub/scan.yml"
        ));
        assert!(m("^a[0-9]+b$", "a2026b"));
        assert!(!m("^a[0-9]+b$", "ab"));
        assert!(m("^ab*c$", "ac"));
        assert!(m("^ab*c$", "abbbc"));
        assert!(m("^(ab)+$", "ababab"));
        assert!(!m("^(ab)+$", "aba"));
        assert!(m("^[a-c.-]+$", "a-b.c"));
        // A repetition of something that can match nothing must still end.
        assert!(m("^(a?)*b$", "aab"));
        assert!(!m("^(a?)*b$", "aac"));
    }

    #[test]
    fn counted_repetition_matches_as_grep_e_does() {
        // The shape the 5 October 2026 review added to ioc/filenames.txt.
        let font = r"(^|/)fa-solid-[0-9]{3}\.llf$";
        assert!(m(font, "fa-solid-900.llf"));
        assert!(m(font, "public/fonts/fa-solid-300.llf"));
        assert!(!m(font, "fa-solid-30.llf"), "two digits is not three");
        assert!(!m(font, "fa-solid-9000.llf"), "four digits is not three");
        assert!(!m(font, "fa-solid-900.woff2"));
        assert!(!m(font, "fa-solid-brands.llf"));

        assert!(m("^a{2,}$", "aa"));
        assert!(m("^a{2,}$", "aaaaa"));
        assert!(!m("^a{2,}$", "a"));
        assert!(m("^a{1,3}b$", "aaab"));
        assert!(!m("^a{1,3}b$", "aaaab"));
        assert!(!m("^a{1,3}b$", "b"));
        assert!(m("^(ab){2}$", "abab"));
        assert!(!m("^(ab){2}$", "ababab"));
        assert!(m("^x{0}y$", "y"));
    }

    #[test]
    fn a_pattern_it_cannot_honour_is_refused_not_guessed() {
        // Each of these would otherwise match nothing, or the wrong thing,
        // and nobody would find out.
        for bad in [
            r"\d+",
            "a{2,1}",
            "a{,3}",
            "a{x}",
            "a{2",
            "a{ 2}",
            "a{256}",
            "{2}a",
            "(open",
            "close)",
            "[abc",
            "*a",
            "[[:alpha:]]",
            "a\\",
        ] {
            assert!(Pattern::parse(bad).is_err(), "{bad} should be refused");
        }
        let e = Pattern::parse("(open").expect_err("refused");
        assert!(e.to_string().contains("(open"));
    }
}
