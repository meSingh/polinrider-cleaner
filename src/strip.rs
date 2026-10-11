//! Working out what to cut from a build config that carries a payload.
//!
//! This is the one place the tool decides to change the contents of somebody's
//! source file, so it only recognises one shape and refuses everything else.
//! The shape is the campaign's own: the payload is appended after the real
//! `export default` or `module.exports`, behind a long run of spaces that
//! pushes it off the right-hand edge of an editor.
//!
//! Everything here is pure. It is handed bytes and returns a plan, or the
//! reason there is no plan. Nothing is read or written, and git is never
//! consulted: a cleanup tool that runs git inside a repository an attacker
//! has written to is taking instructions from that repository's config.
//! See ADR-0031.

use crate::indicators::Indicators;

/// The shortest run of spaces or tabs that counts as the campaign's padding.
/// The observed run is about 280. Nobody indents a config 64 columns deep, so
/// this leaves room for a variant with less without matching real code.
pub const MIN_PADDING: usize = 64;

/// What to keep and what to cut.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    /// The file as it should be afterwards.
    pub keep: Vec<u8>,
    /// How many bytes go, padding included.
    pub removed: usize,
    /// The last line that stays, 1-based.
    pub last_kept_line: usize,
    /// The start of what goes, with the padding skipped, for showing to the
    /// operator before anything is changed.
    pub preview: String,
}

/// Why a file carrying an indicator will not be edited.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// The indicator is not behind a run of padding. Not the known shape.
    NoPadding,
    /// Nothing that looks like the end of a module comes before the payload.
    NoModuleEnd,
    /// What would remain still contains an indicator.
    IndicatorRemains,
    /// Real code follows the payload, so it was inserted, not appended.
    CodeAfterPayload,
    /// What would remain has unbalanced brackets: the cut lands mid-statement.
    Incomplete,
}

impl Refusal {
    /// Why, in the words printed under the finding.
    pub const fn reason(self) -> &'static str {
        match self {
            Refusal::NoPadding => "the indicator is not behind the padding this campaign uses",
            Refusal::NoModuleEnd => "there is no export default or module.exports before it",
            Refusal::IndicatorRemains => "an indicator would still be left in the file",
            Refusal::CodeAfterPayload => {
                "there is code after the payload, so it was not simply appended"
            }
            Refusal::Incomplete => "the file would not be complete once the payload was cut",
        }
    }
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || needle.len() > haystack.len() {
        return None;
    }
    haystack.windows(needle.len()).position(|w| w == needle)
}

fn first_indicator(bytes: &[u8], ind: &Indicators) -> Option<usize> {
    ind.strong
        .iter()
        .filter_map(|i| find(bytes, i.as_bytes()))
        .min()
}

/// The start of the first run of at least [`MIN_PADDING`] spaces or tabs
/// before `limit`.
///
/// The first, not the nearest. A payload split across two padded pieces is
/// cut from the first piece, so nothing padded is left behind. If the first
/// run turns out to be the project's own, the code after it makes the plan
/// refuse, which is the safe way for that to go.
fn padding_before(bytes: &[u8], limit: usize) -> Option<usize> {
    let mut run_start = 0usize;
    let mut run_len = 0usize;
    for (at, byte) in bytes.iter().enumerate().take(limit) {
        if *byte == b' ' || *byte == b'\t' {
            if run_len == 0 {
                run_start = at;
            }
            run_len += 1;
            if run_len >= MIN_PADDING {
                return Some(run_start);
            }
        } else {
            run_len = 0;
        }
    }
    None
}

fn is_module_end(line: &str) -> bool {
    line.starts_with("export default") || line.starts_with("module.exports")
}

/// A line that only makes sense as part of the project's own code. Finding
/// one after the payload means the payload sits inside the module.
fn is_project_code(line: &str) -> bool {
    let line = line.trim();
    is_module_end(line)
        || line.starts_with("import ")
        || matches!(
            line,
            "}" | "};" | "]" | "];" | ")" | ");" | "})" | "});" | "]);"
        )
}

/// Brackets open and close the same number of times. Not a parser: a bracket
/// inside a string or a regular expression is counted. That makes this refuse
/// some files it could have handled, which is the direction to be wrong in.
fn brackets_balance(text: &str) -> bool {
    let count = |c: char| text.chars().filter(|x| *x == c).count();
    count('{') == count('}') && count('[') == count(']') && count('(') == count(')')
}

/// Plan the cut, or say why there is not one. `None` when the file carries no
/// indicator at all.
pub fn plan(bytes: &[u8], ind: &Indicators) -> Option<Result<Plan, Refusal>> {
    let indicator_at = first_indicator(bytes, ind)?;
    Some(plan_from(bytes, indicator_at, ind))
}

fn plan_from(bytes: &[u8], indicator_at: usize, ind: &Indicators) -> Result<Plan, Refusal> {
    let cut = padding_before(bytes, indicator_at).ok_or(Refusal::NoPadding)?;
    let (before, after) = bytes.split_at(cut);

    // What stays: everything before the padding, without the trailing blanks
    // the cut leaves on its last line, ending in exactly one newline.
    let mut keep = before.to_vec();
    while keep.last().is_some_and(|b| *b == b' ' || *b == b'\t') {
        keep.pop();
    }
    if keep.last().is_some_and(|b| *b != b'\n') {
        keep.push(b'\n');
    }

    let kept_text = String::from_utf8_lossy(&keep);
    if !kept_text.lines().any(is_module_end) {
        return Err(Refusal::NoModuleEnd);
    }
    // Cannot happen as the code stands: the cut is before the first
    // indicator. Checked anyway, because this is the function that edits
    // source and "the result is clean" should be observed, not deduced.
    if first_indicator(&keep, ind).is_some() {
        return Err(Refusal::IndicatorRemains);
    }

    let removed_text = String::from_utf8_lossy(after);
    if removed_text.lines().any(is_project_code) {
        return Err(Refusal::CodeAfterPayload);
    }
    if !brackets_balance(&kept_text) {
        return Err(Refusal::Incomplete);
    }

    Ok(Plan {
        removed: after.len(),
        last_kept_line: kept_text.lines().count(),
        preview: removed_text.trim_start().chars().take(72).collect(),
        keep,
    })
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    // A synthetic marker, never a live indicator.
    const MARK: &str = "MARKER-ALPHA";

    fn ind() -> Indicators {
        Indicators {
            strong: vec![MARK.into()],
            ..Indicators::default()
        }
    }

    fn pad() -> String {
        " ".repeat(280)
    }

    fn planned(text: &str) -> Result<Plan, Refusal> {
        plan(text.as_bytes(), &ind()).expect("the file carries an indicator")
    }

    fn kept(text: &str) -> String {
        String::from_utf8(planned(text).expect("a plan").keep).expect("utf-8")
    }

    #[test]
    fn a_file_without_an_indicator_has_nothing_to_plan() {
        assert!(plan(b"export default {}\n", &ind()).is_none());
    }

    #[test]
    fn a_payload_on_its_own_line_is_cut_and_the_module_is_kept_whole() {
        let text = format!(
            "export default {{ plugins: {{}} }}\n{}var x='{MARK}';\n",
            pad()
        );
        assert_eq!(kept(&text), "export default { plugins: {} }\n");
    }

    #[test]
    fn a_payload_on_the_module_line_is_cut_and_the_line_keeps_its_newline() {
        // The usual form: the padding pushes it off the edge of the editor, on
        // the same line as the closing brace.
        let text = format!(
            "module.exports = {{ plugins: [] }};{}var x='{MARK}';",
            pad()
        );
        assert_eq!(kept(&text), "module.exports = { plugins: [] };\n");
    }

    #[test]
    fn uncommitted_work_above_the_payload_survives_byte_for_byte() {
        let mine = "import tailwind from 'tailwindcss'\n\n// my half-finished change\nconst extra = [1, 2, 3]\n\nexport default {\n  plugins: [tailwind, ...extra],\n}\n";
        let text = format!("{mine}{}eval('{MARK}')\n", pad());
        assert_eq!(kept(&text), mine);
        let p = planned(&text).expect("a plan");
        assert_eq!(p.last_kept_line, 8);
        assert!(p.preview.starts_with("eval('MARKER-ALPHA')"));
        assert_eq!(p.removed, 280 + "eval('MARKER-ALPHA')\n".len());
    }

    #[test]
    fn a_multi_line_payload_is_cut_whole() {
        let text = format!(
            "export default {{}}\n{}var a='{MARK}';\nvar b=atob('eA==');\nrun(a,b);\n",
            pad()
        );
        assert_eq!(kept(&text), "export default {}\n");
    }

    #[test]
    fn a_payload_with_no_padding_is_not_the_known_shape_and_is_left() {
        // It may well be a payload. It is not one this code recognises, and
        // editing source on a guess is how a cleanup tool breaks a build.
        let text = format!("export default {{}}\nvar x='{MARK}';\n");
        assert_eq!(planned(&text), Err(Refusal::NoPadding));
        // Ordinary indentation is not padding.
        let text = format!("export default {{}}\n        var x='{MARK}';\n");
        assert_eq!(planned(&text), Err(Refusal::NoPadding));
    }

    #[test]
    fn a_payload_inside_the_module_is_refused() {
        // Cutting to the end of the file would take the rest of the module.
        let text = format!(
            "module.exports = {{{}var x='{MARK}';\n  plugins: [],\n}}\n",
            pad()
        );
        assert_eq!(planned(&text), Err(Refusal::CodeAfterPayload));

        // The same without a lone closing bracket to give it away: what would
        // remain opens a brace it never closes.
        let text = format!(
            "module.exports = {{{}var x='{MARK}';\n  plugins: [] }}\n",
            pad()
        );
        assert_eq!(planned(&text), Err(Refusal::Incomplete));
    }

    #[test]
    fn a_file_with_no_module_before_the_payload_is_refused() {
        let text = format!("const config = {{}}\n{}var x='{MARK}';\n", pad());
        assert_eq!(planned(&text), Err(Refusal::NoModuleEnd));
    }

    #[test]
    fn an_indicator_that_is_not_behind_padding_stops_the_whole_cut() {
        // Two payloads, or one that was not appended. The first indicator in
        // the file decides: if that one is not behind padding, a cut further
        // down would leave it in place and call the file clean.
        let padded = format!("export default {{}}\n{}var x='{MARK}';\n", pad());
        assert_eq!(
            planned(&format!("// {MARK}\n{padded}")),
            Err(Refusal::NoPadding)
        );

        let text = format!(
            "export default {{}}\nvar early='{MARK}';\n{}var x='{MARK}';\n",
            pad()
        );
        assert_eq!(planned(&text), Err(Refusal::NoPadding));
    }

    #[test]
    fn a_payload_in_two_padded_pieces_is_cut_from_the_first() {
        // Cutting at the padding nearest the indicator would leave the first
        // piece behind, 280 columns off screen.
        let text = format!(
            "export default {{}}\n{}var a=1;{}var x='{MARK}';\n",
            pad(),
            pad()
        );
        assert_eq!(kept(&text), "export default {}\n");
    }

    #[test]
    fn every_refusal_says_why_in_words() {
        for r in [
            Refusal::NoPadding,
            Refusal::NoModuleEnd,
            Refusal::IndicatorRemains,
            Refusal::CodeAfterPayload,
            Refusal::Incomplete,
        ] {
            assert!(!r.reason().is_empty());
        }
    }
}
