//! Renderer-agnostic formula trees for the shared breakdown component.
//!
//! A tree is plain data: every node carries the label, the inputs that produced
//! it, an optional multiplier and source, and its own result. The UI walks the
//! same tree to draw it, search it and copy it, and tests can check that a
//! group's children add up to the value its section reports.

/// How a node's value reads in the UI and in copied text.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ValueFormat {
    /// A plain number, trimmed to four decimals and compacted above 10K.
    #[default]
    Raw,
    /// A whole count with thousands separators.
    Count,
    /// A signed percentage, e.g. `+12.5%`.
    Percent,
    /// A multiplication factor, e.g. `×1.25`.
    Multiplier,
    /// A rate with `K`/`M`/`B` and `/hr`.
    Rate,
}

impl ValueFormat {
    /// The node's result as the UI and copied text show it.
    pub fn format(self, value: f64) -> String {
        if !value.is_finite() {
            return "—".into();
        }
        match self {
            Self::Count => count(value),
            Self::Percent => {
                let sign = if value < 0.0 { "-" } else { "+" };
                format!("{sign}{}%", trimmed(value.abs(), 2))
            }
            Self::Multiplier => format!("×{}", trimmed(value, 4)),
            Self::Rate => rate(value),
            Self::Raw => trimmed(value, 4),
        }
    }
}

fn trimmed(value: f64, precision: usize) -> String {
    if !value.is_finite() {
        return "—".into();
    }
    if value == 0.0 {
        return "0".into();
    }
    let value = if value.abs() >= 1e9 {
        format!("{:.2}B", value / 1e9)
    } else if value.abs() >= 1e6 {
        format!("{:.2}M", value / 1e6)
    } else if value.abs() >= 1e4 {
        format!("{:.2}K", value / 1e3)
    } else {
        format!("{value:.precision$}")
    };
    if value.contains(['B', 'M', 'K']) {
        return value;
    }
    let value = value.trim_end_matches('0').trim_end_matches('.');
    if value == "-0" {
        "0".into()
    } else {
        value.to_string()
    }
}

fn count(value: f64) -> String {
    let rounded = value.round() as i64;
    let digits = rounded.abs().to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3 + 1);
    if rounded < 0 {
        out.push('-');
    }
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

fn rate(value: f64) -> String {
    let magnitude = value.abs();
    let (scaled, suffix) = if magnitude >= 1e15 {
        (value / 1e15, "Q")
    } else if magnitude >= 1e12 {
        (value / 1e12, "T")
    } else if magnitude >= 1e9 {
        (value / 1e9, "B")
    } else if magnitude >= 1e6 {
        (value / 1e6, "M")
    } else if magnitude >= 1e3 {
        (value / 1e3, "K")
    } else {
        (value, "")
    };
    let decimals = if suffix.is_empty() { 0 } else { 2 };
    format!("{}{suffix}/hr", trimmed(scaled, decimals))
}

/// One node of a formula tree.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FormulaNode {
    /// What this term is called, e.g. `Takin' Notes`.
    pub label: String,
    /// The node's own result.
    pub value: f64,
    /// How the result reads.
    pub format: ValueFormat,
    /// The inputs that produced the result, e.g. `5% × Lv 3`.
    pub input: Option<String>,
    /// The multiplier this term applies, when it is one.
    pub multiplier: Option<f64>,
    /// Where the value comes from, e.g. `Grid J6 · Takin' Notes`.
    pub source: Option<String>,
    /// An optional explanation shown under the node and copied with it.
    pub note: Option<String>,
    /// The terms that make up this node; empty for a leaf.
    pub children: Vec<FormulaNode>,
}

impl FormulaNode {
    /// A leaf term named `label` with its result.
    pub fn new(label: impl Into<String>, value: f64) -> Self {
        Self {
            label: label.into(),
            value,
            ..Self::default()
        }
    }

    /// A leaf whose result reads as `format`.
    pub fn leaf(label: impl Into<String>, value: f64, format: ValueFormat) -> Self {
        Self::new(label, value).with_format(format)
    }

    /// A group whose value is the sum of `children`.
    pub fn group(
        label: impl Into<String>,
        format: ValueFormat,
        children: impl IntoIterator<Item = FormulaNode>,
    ) -> Self {
        let children: Vec<FormulaNode> = children.into_iter().collect();
        let value = children.iter().map(|child| child.value).sum();
        Self {
            label: label.into(),
            value,
            format,
            children,
            ..Self::default()
        }
    }

    /// Sets how the result reads.
    pub fn with_format(mut self, format: ValueFormat) -> Self {
        self.format = format;
        self
    }

    /// Sets the inputs that produced the result.
    pub fn with_input(mut self, input: impl Into<String>) -> Self {
        self.input = Some(input.into());
        self
    }

    /// Sets the multiplier this term applies.
    pub fn with_multiplier(mut self, multiplier: f64) -> Self {
        self.multiplier = Some(multiplier);
        self
    }

    /// Sets where the value comes from.
    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source = Some(source.into());
        self
    }

    /// Sets the node's explanation.
    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.note = Some(note.into());
        self
    }

    /// Sets the terms below this node, keeping the node's own result.
    pub fn with_children(mut self, children: impl IntoIterator<Item = FormulaNode>) -> Self {
        self.children = children.into_iter().collect();
        self
    }

    /// The sum of the immediate children.
    pub fn sum_children(&self) -> f64 {
        self.children.iter().map(|child| child.value).sum()
    }

    /// Whether one of this node's fields matches `query`, case-insensitively.
    pub fn matches(&self, query: &str) -> bool {
        let query = query.trim().to_lowercase();
        if query.is_empty() {
            return false;
        }
        [
            Some(self.label.as_str()),
            self.input.as_deref(),
            self.source.as_deref(),
            self.note.as_deref(),
        ]
        .into_iter()
        .flatten()
        .chain(std::iter::once(self.format.format(self.value).as_str()))
        .any(|field| field.to_lowercase().contains(&query))
    }

    /// How many nodes in this subtree match `query`.
    pub fn match_count(&self, query: &str) -> usize {
        if query.trim().is_empty() {
            return 0;
        }
        usize::from(self.matches(query))
            + self
                .children
                .iter()
                .map(|child| child.match_count(query))
                .sum::<usize>()
    }

    /// How many nodes this subtree holds, including itself.
    pub fn node_count(&self) -> usize {
        1 + self
            .children
            .iter()
            .map(FormulaNode::node_count)
            .sum::<usize>()
    }

    /// A stable key for every node in the tree, root first, using child indexes.
    pub fn keys(&self) -> Vec<String> {
        fn walk(node: &FormulaNode, path: &str, keys: &mut Vec<String>) {
            keys.push(path.to_string());
            for (index, child) in node.children.iter().enumerate() {
                walk(child, &format!("{path}/{index}"), keys);
            }
        }
        let mut keys = Vec::with_capacity(self.node_count());
        walk(self, "root", &mut keys);
        keys
    }

    /// Copy-friendly text: one indented line per node with its inputs and source.
    pub fn copy_text(&self) -> String {
        fn walk(node: &FormulaNode, depth: usize, out: &mut String) {
            out.push_str(&"  ".repeat(depth));
            out.push_str(&node.label);
            out.push_str(": ");
            out.push_str(&node.format.format(node.value));
            if let Some(input) = &node.input {
                out.push_str(" · input ");
                out.push_str(input);
            }
            if let Some(multiplier) = node.multiplier {
                out.push_str(" · ×");
                out.push_str(&trimmed(multiplier, 4));
            }
            if let Some(source) = &node.source {
                out.push_str(" · ");
                out.push_str(source);
            }
            if let Some(note) = &node.note {
                out.push_str(" · ");
                out.push_str(note);
            }
            out.push('\n');
            for child in &node.children {
                walk(child, depth + 1, out);
            }
        }
        let mut out = String::new();
        walk(self, 0, &mut out);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> FormulaNode {
        FormulaNode::group(
            "Total",
            ValueFormat::Rate,
            [
                FormulaNode::leaf("Grid", 25.0, ValueFormat::Percent)
                    .with_source("Grid A12 · Sharp Eye"),
                FormulaNode::leaf("Takin' Notes", 12.0, ValueFormat::Percent)
                    .with_input("12% × Lv 1"),
            ],
        )
    }

    #[test]
    fn a_group_sums_its_children() {
        let tree = sample();
        assert_eq!(tree.value, 37.0);
        assert_eq!(tree.sum_children(), 37.0);
        assert_eq!(tree.node_count(), 3);
    }

    #[test]
    fn search_reads_labels_inputs_sources_and_notes() {
        let tree = sample();
        assert_eq!(tree.match_count("grid"), 1);
        assert_eq!(tree.match_count("a12"), 1);
        assert_eq!(tree.match_count("12% ×"), 1);
        assert_eq!(tree.match_count("missing"), 0);
        assert_eq!(tree.match_count("  "), 0);
    }

    #[test]
    fn values_read_in_their_own_format() {
        assert_eq!(ValueFormat::Percent.format(12.5), "+12.5%");
        assert_eq!(ValueFormat::Percent.format(-2.0), "-2%");
        assert_eq!(ValueFormat::Multiplier.format(1.25), "×1.25");
        assert_eq!(ValueFormat::Count.format(1234.0), "1,234");
        assert_eq!(ValueFormat::Rate.format(1652.1), "1.65K/hr");
        assert_eq!(ValueFormat::Rate.format(2.5e9), "2.5B/hr");
        assert_eq!(ValueFormat::Rate.format(2.5e12), "2.5T/hr");
        assert_eq!(ValueFormat::Rate.format(2.5e15), "2.5Q/hr");
        assert_eq!(ValueFormat::Raw.format(1652.1), "1652.1");
        assert_eq!(ValueFormat::Raw.format(f64::INFINITY), "—");
    }

    #[test]
    fn copy_text_keeps_the_tree_and_the_provenance() {
        let text = sample().copy_text();
        assert!(text.starts_with("Total: 37/hr\n"));
        assert!(text.contains("  Grid: +25% · Grid A12 · Sharp Eye"));
        assert!(text.contains("  Takin' Notes: +12% · input 12% × Lv 1"));
    }

    #[test]
    fn keys_walk_the_tree_in_order() {
        assert_eq!(sample().keys(), ["root", "root/0", "root/1"]);
    }
}
