//! Minimal regex engine (backtracking subset): literals, ., * + ?, [...] [^...],
//! (...) groups, | alternation, ^ $ anchors, escapes \d \w \s \. etc.
//! Enough for DECX `regex=true` filters without external crates.

#[derive(Clone, Debug)]
enum Node {
    Char(char),
    Any,
    Class { neg: bool, items: Vec<ClassItem> },
    Start,
    End,
    Group(Box<Node>),          // capturing not needed; alternation handled inside
    Concat(Vec<Node>),
    Alt(Vec<Node>),
    Star(Box<Node>, bool),     // greedy
    Plus(Box<Node>, bool),
    Opt(Box<Node>),
}

#[derive(Clone, Debug)]
enum ClassItem {
    Ch(char),
    Range(char, char),
    Digit,
    NotDigit,
    Word,
    NotWord,
    Space,
    NotSpace,
}

pub struct Regex {
    root: Node,
}

impl Regex {
    pub fn new(pattern: &str) -> Result<Regex, String> {
        let chars: Vec<char> = pattern.chars().collect();
        let mut pos = 0;
        let root = parse_alt(&chars, &mut pos)?;
        Ok(Regex { root })
    }

    /// unanchored search: any start position
    pub fn is_match(&self, text: &str) -> bool {
        let chars: Vec<char> = text.chars().collect();
        for start in 0..=chars.len() {
            if m(&self.root, &chars, start, &mut |_| true) {
                return true;
            }
        }
        false
    }
}

fn parse_alt(c: &[char], pos: &mut usize) -> Result<Node, String> {
    let mut branches = vec![parse_concat(c, pos)?];
    while *pos < c.len() && c[*pos] == '|' {
        *pos += 1;
        branches.push(parse_concat(c, pos)?);
    }
    if branches.len() == 1 {
        Ok(branches.pop().unwrap())
    } else {
        Ok(Node::Alt(branches))
    }
}

fn parse_concat(c: &[char], pos: &mut usize) -> Result<Node, String> {
    let mut items = Vec::new();
    while *pos < c.len() && c[*pos] != '|' && c[*pos] != ')' {
        items.push(parse_repeat(c, pos)?);
    }
    match items.len() {
        0 => Ok(Node::Concat(Vec::new())),
        1 => Ok(items.pop().unwrap()),
        _ => Ok(Node::Concat(items)),
    }
}

fn parse_repeat(c: &[char], pos: &mut usize) -> Result<Node, String> {
    let atom = parse_atom(c, pos)?;
    if *pos >= c.len() {
        return Ok(atom);
    }
    let q = c[*pos];
    if q != '*' && q != '+' && q != '?' {
        return Ok(atom);
    }
    *pos += 1;
    let mut n = match q {
        '*' => Node::Star(Box::new(atom), true),
        '+' => Node::Plus(Box::new(atom), true),
        _ => Node::Opt(Box::new(atom)),
    };
    if *pos < c.len() && c[*pos] == '?' {
        *pos += 1;
        n = set_lazy(n);
    }
    Ok(n)
}

fn set_lazy(n: Node) -> Node {
    match n {
        Node::Star(inner, _) => Node::Star(inner, false),
        Node::Plus(inner, _) => Node::Plus(inner, false),
        other => other,
    }
}

fn parse_atom(c: &[char], pos: &mut usize) -> Result<Node, String> {
    let ch = *c.get(*pos).ok_or("regex: unexpected end")?;
    *pos += 1;
    Ok(match ch {
        '.' => Node::Any,
        '^' => Node::Start,
        '$' => Node::End,
        '(' => {
            // non-capturing marker (?:...) accepted and ignored
            if *pos + 1 < c.len() && c[*pos] == '?' && c[*pos + 1] == ':' {
                *pos += 2;
            }
            let inner = parse_alt(c, pos)?;
            if *pos >= c.len() || c[*pos] != ')' {
                return Err("regex: missing )".into());
            }
            *pos += 1;
            Node::Group(Box::new(inner))
        }
        '[' => {
            let mut neg = false;
            if *pos < c.len() && c[*pos] == '^' {
                neg = true;
                *pos += 1;
            }
            let mut items = Vec::new();
            let mut first = true;
            while *pos < c.len() && (c[*pos] != ']' || first) {
                first = false;
                let a = c[*pos];
                if a == '\\' && *pos + 1 < c.len() {
                    *pos += 1;
                    items.push(escape_class(c[*pos]));
                    *pos += 1;
                    continue;
                }
                if *pos + 2 < c.len() && c[*pos + 1] == '-' && c[*pos + 2] != ']' {
                    items.push(ClassItem::Range(a, c[*pos + 2]));
                    *pos += 3;
                } else {
                    items.push(ClassItem::Ch(a));
                    *pos += 1;
                }
            }
            if *pos >= c.len() {
                return Err("regex: missing ]".into());
            }
            *pos += 1;
            Node::Class { neg, items }
        }
        '\\' => {
            let e = *c.get(*pos).ok_or("regex: dangling escape")?;
            *pos += 1;
            match e {
                'd' => Node::Class { neg: false, items: vec![ClassItem::Digit] },
                'D' => Node::Class { neg: false, items: vec![ClassItem::NotDigit] },
                'w' => Node::Class { neg: false, items: vec![ClassItem::Word] },
                'W' => Node::Class { neg: false, items: vec![ClassItem::NotWord] },
                's' => Node::Class { neg: false, items: vec![ClassItem::Space] },
                'S' => Node::Class { neg: false, items: vec![ClassItem::NotSpace] },
                'n' => Node::Char('\n'),
                't' => Node::Char('\t'),
                'r' => Node::Char('\r'),
                other => Node::Char(other),
            }
        }
        other => Node::Char(other),
    })
}

fn escape_class(e: char) -> ClassItem {
    match e {
        'd' => ClassItem::Digit,
        'D' => ClassItem::NotDigit,
        'w' => ClassItem::Word,
        'W' => ClassItem::NotWord,
        's' => ClassItem::Space,
        'S' => ClassItem::NotSpace,
        'n' => ClassItem::Ch('\n'),
        't' => ClassItem::Ch('\t'),
        other => ClassItem::Ch(other),
    }
}

fn class_matches(items: &[ClassItem], neg: bool, ch: char) -> bool {
    let mut hit = false;
    for it in items {
        let ok = match it {
            ClassItem::Ch(c) => *c == ch,
            ClassItem::Range(a, b) => ch >= *a && ch <= *b,
            ClassItem::Digit => ch.is_ascii_digit(),
            ClassItem::NotDigit => !ch.is_ascii_digit(),
            ClassItem::Word => ch.is_alphanumeric() || ch == '_',
            ClassItem::NotWord => !(ch.is_alphanumeric() || ch == '_'),
            ClassItem::Space => ch.is_whitespace(),
            ClassItem::NotSpace => !ch.is_whitespace(),
        };
        if ok {
            hit = true;
            break;
        }
    }
    hit != neg
}

/// Continuation-passing matcher. k = continuation for remaining input.
fn m(node: &Node, s: &[char], i: usize, k: &mut dyn FnMut(usize) -> bool) -> bool {
    match node {
        Node::Char(c) => i < s.len() && s[i] == *c && k(i + 1),
        Node::Any => i < s.len() && s[i] != '\n' && k(i + 1),
        Node::Class { neg, items } => i < s.len() && class_matches(items, *neg, s[i]) && k(i + 1),
        Node::Start => i == 0 && k(i),
        Node::End => i == s.len() && k(i),
        Node::Group(inner) => m(inner, s, i, k),
        Node::Concat(items) => concat_m(items, s, i, k),
        Node::Alt(branches) => branches.iter().any(|b| m(b, s, i, k)),
        Node::Opt(inner) => m(inner, s, i, k) || k(i),
        Node::Plus(inner, greedy) => {
            // one or more
            plus_m(inner, s, i, *greedy, 0, k)
        }
        Node::Star(inner, greedy) => {
            if *greedy {
                star_greedy(inner, s, i, k)
            } else {
                k(i) || m(inner, s, i, &mut |j| if j == i { false } else { star_lazy(inner, s, j, k) })
            }
        }
    }
}

fn concat_m(items: &[Node], s: &[char], i: usize, k: &mut dyn FnMut(usize) -> bool) -> bool {
    match items.split_first() {
        None => k(i),
        Some((head, rest)) => m(head, s, i, &mut |j| concat_m(rest, s, j, k)),
    }
}

fn star_greedy(inner: &Node, s: &[char], i: usize, k: &mut dyn FnMut(usize) -> bool) -> bool {
    // try longest first: recurse via inner then fallback
    let advanced = m(inner, s, i, &mut |j| if j == i { false } else { star_greedy(inner, s, j, k) });
    advanced || k(i)
}

fn star_lazy(inner: &Node, s: &[char], i: usize, k: &mut dyn FnMut(usize) -> bool) -> bool {
    k(i) || m(inner, s, i, &mut |j| if j == i { false } else { star_lazy(inner, s, j, k) })
}

fn plus_m(inner: &Node, s: &[char], i: usize, greedy: bool, count: usize, k: &mut dyn FnMut(usize) -> bool) -> bool {
    let once = m(inner, s, i, &mut |j| {
        if greedy {
            // more first, then stop
            plus_m(inner, s, j, greedy, count + 1, k) || k(j)
        } else {
            k(j) || plus_m(inner, s, j, greedy, count + 1, k)
        }
    });
    once
}

#[cfg(test)]
mod tests {
    use super::Regex;

    #[test]
    fn basics() {
        assert!(Regex::new("abc").unwrap().is_match("xxabcyy"));
        assert!(!Regex::new("^abc$").unwrap().is_match("xxabcyy"));
        assert!(Regex::new("^abc$").unwrap().is_match("abc"));
        assert!(Regex::new("a.c").unwrap().is_match("abc"));
        assert!(Regex::new("ab*c").unwrap().is_match("ac"));
        assert!(Regex::new("ab+c").unwrap().is_match("abbc"));
        assert!(!Regex::new("ab+c").unwrap().is_match("ac"));
        assert!(Regex::new("[a-z]+\\d").unwrap().is_match("hello5"));
        assert!(!Regex::new("[^a-z]").unwrap().is_match("abc"));
        assert!(Regex::new("cat|dog").unwrap().is_match("hotdog"));
        assert!(Regex::new("(ab)+").unwrap().is_match("ababab"));
        assert!(Regex::new("a?b").unwrap().is_match("b"));
        assert!(Regex::new("Lcom/.*/Main;").unwrap().is_match("Lcom/vivo/weather/Main;"));
    }
}
