//! Pustaka Standar Reguler Ekspresi (Regex) DapCode
//!
//! Modul ini menyediakan mesin pencocokan regex murni tanpa dependensi eksternal,
//! mendukung:
//! - Karakter harfiah & titik sembarang (.)
//! - Kuantifier greedy: *, +, ?, {n}, {n,m}
//! - Kelas karakter: [a-z], [0-9], [^...], \d, \D, \w, \W, \s, \S
//! - Penanda batas kata (\b, \B) dan jangkar (^, $)
//! - Grup penangkapan: (...) dan non-capturing (?:...)
//! - Karakter pelarian (escaped characters)
//! - Alternasi cabang (a|b)
//!
//! Fungsi utama:
//! - `cocok_regex(pola: &str, teks: &str) -> bool`
//! - `cari_regex(pola: &str, teks: &str) -> Vec<String>`
//! - `ganti_regex(pola: &str, teks: &str, pengganti: &str) -> String`
//! - `ekstrak_regex(pola: &str, teks: &str) -> Result<String, String>`
//! - `ekstrak_grup_regex(pola: &str, teks: &str) -> Result<Vec<String>, String>`
//! - `pisah_regex(pola: &str, teks: &str) -> Vec<String>`

use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub enum CharClass {
    Any,
    Literal(char),
    Range(char, char),
    Digit,
    NonDigit,
    Word,
    NonWord,
    Whitespace,
    NonWhitespace,
    Union(Vec<CharClass>),
    Negated(Box<CharClass>),
}

impl CharClass {
    pub fn matches(&self, c: char) -> bool {
        match self {
            CharClass::Any => c != '\n',
            CharClass::Literal(ch) => *ch == c,
            CharClass::Range(start, end) => c >= *start && c <= *end,
            CharClass::Digit => c.is_ascii_digit(),
            CharClass::NonDigit => !c.is_ascii_digit(),
            CharClass::Word => c.is_ascii_alphanumeric() || c == '_',
            CharClass::NonWord => !(c.is_ascii_alphanumeric() || c == '_'),
            CharClass::Whitespace => c.is_whitespace(),
            CharClass::NonWhitespace => !c.is_whitespace(),
            CharClass::Union(classes) => classes.iter().any(|cls| cls.matches(c)),
            CharClass::Negated(inner) => !inner.matches(c),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum RegexNode {
    AnchorStart,
    AnchorEnd,
    WordBoundary,
    NonWordBoundary,
    Class(CharClass),
    ZeroOrMore(Box<RegexNode>),
    OneOrMore(Box<RegexNode>),
    ZeroOrOne(Box<RegexNode>),
    Repeat(Box<RegexNode>, usize, Option<usize>),
    Group(usize, Box<RegexNode>),
    GroupEnd(usize, usize),
    Concat(Vec<RegexNode>),
    Alternation(Vec<RegexNode>),
}

pub struct RegexParser {
    chars: Vec<char>,
    pos: usize,
    group_counter: usize,
}

impl RegexParser {
    pub fn new(pattern: &str) -> Self {
        RegexParser {
            chars: pattern.chars().collect(),
            pos: 0,
            group_counter: 0,
        }
    }

    fn peek(&self) -> Option<char> {
        if self.pos < self.chars.len() {
            Some(self.chars[self.pos])
        } else {
            None
        }
    }

    fn advance(&mut self) -> Option<char> {
        if self.pos < self.chars.len() {
            let ch = self.chars[self.pos];
            self.pos += 1;
            Some(ch)
        } else {
            None
        }
    }

    pub fn parse(&mut self) -> Result<Vec<RegexNode>, String> {
        let (nodes, _) = self.parse_with_group_count()?;
        Ok(nodes)
    }

    pub fn parse_with_group_count(&mut self) -> Result<(Vec<RegexNode>, usize), String> {
        let node = self.parse_alternation()?;
        let nodes = match node {
            RegexNode::Concat(nodes) => nodes,
            other => vec![other],
        };
        Ok((nodes, self.group_counter))
    }

    fn parse_alternation(&mut self) -> Result<RegexNode, String> {
        let mut branches = vec![self.parse_concat()?];

        while let Some('|') = self.peek() {
            self.advance();
            branches.push(self.parse_concat()?);
        }

        if branches.len() == 1 {
            Ok(branches.remove(0))
        } else {
            Ok(RegexNode::Alternation(branches))
        }
    }

    fn parse_concat(&mut self) -> Result<RegexNode, String> {
        let mut nodes = Vec::new();

        while let Some(ch) = self.peek() {
            if ch == '|' || ch == ')' {
                break;
            }
            nodes.push(self.parse_piece()?);
        }

        Ok(RegexNode::Concat(nodes))
    }

    fn parse_piece(&mut self) -> Result<RegexNode, String> {
        let atom = self.parse_atom()?;

        match self.peek() {
            Some('*') => {
                self.advance();
                Ok(RegexNode::ZeroOrMore(Box::new(atom)))
            }
            Some('+') => {
                self.advance();
                Ok(RegexNode::OneOrMore(Box::new(atom)))
            }
            Some('?') => {
                self.advance();
                Ok(RegexNode::ZeroOrOne(Box::new(atom)))
            }
            Some('{') => {
                self.advance();
                let mut num_str = String::new();
                while let Some(c) = self.peek() {
                    if c.is_ascii_digit() {
                        num_str.push(c);
                        self.advance();
                    } else {
                        break;
                    }
                }
                let min = num_str.parse::<usize>().map_err(|_| "Batas quantifier '{' tidak valid")?;
                let mut max = Some(min);

                if let Some(',') = self.peek() {
                    self.advance();
                    let mut max_str = String::new();
                    while let Some(c) = self.peek() {
                        if c.is_ascii_digit() {
                            max_str.push(c);
                            self.advance();
                        } else {
                            break;
                        }
                    }
                    if max_str.is_empty() {
                        max = None;
                    } else {
                        max = Some(max_str.parse::<usize>().map_err(|_| "Batas maksimum '{' tidak valid")?);
                    }
                }

                if let Some('}') = self.peek() {
                    self.advance();
                } else {
                    return Err("Quantifier '{' tidak ditutup dengan '}'".to_string());
                }

                Ok(RegexNode::Repeat(Box::new(atom), min, max))
            }
            _ => Ok(atom),
        }
    }

    fn parse_atom(&mut self) -> Result<RegexNode, String> {
        let ch = match self.advance() {
            Some(c) => c,
            None => return Err("Akhir pola regex tidak diharapkan".to_string()),
        };

        match ch {
            '^' => Ok(RegexNode::AnchorStart),
            '$' => Ok(RegexNode::AnchorEnd),
            '.' => Ok(RegexNode::Class(CharClass::Any)),
            '(' => {
                if self.peek() == Some('?') {
                    self.advance(); // consume '?'
                    if self.peek() == Some(':') {
                        self.advance(); // consume ':'
                        let inner = self.parse_alternation()?;
                        if let Some(')') = self.advance() {
                            Ok(inner)
                        } else {
                            Err("Kurung '(?:' tidak ditutup dengan ')'".to_string())
                        }
                    } else {
                        Err("Ekspresi '(?...)' tidak didukung".to_string())
                    }
                } else {
                    self.group_counter += 1;
                    let group_id = self.group_counter;
                    let inner = self.parse_alternation()?;
                    if let Some(')') = self.advance() {
                        Ok(RegexNode::Group(group_id, Box::new(inner)))
                    } else {
                        Err("Kurung '(' tidak ditutup dengan ')'".to_string())
                    }
                }
            }
            '[' => self.parse_bracket(),
            '\\' => {
                let escaped = match self.advance() {
                    Some(c) => c,
                    None => return Err("Karakter escape '\\' menggantung di akhir pola".to_string()),
                };
                match escaped {
                    'b' => Ok(RegexNode::WordBoundary),
                    'B' => Ok(RegexNode::NonWordBoundary),
                    'd' => Ok(RegexNode::Class(CharClass::Digit)),
                    'D' => Ok(RegexNode::Class(CharClass::NonDigit)),
                    'w' => Ok(RegexNode::Class(CharClass::Word)),
                    'W' => Ok(RegexNode::Class(CharClass::NonWord)),
                    's' => Ok(RegexNode::Class(CharClass::Whitespace)),
                    'S' => Ok(RegexNode::Class(CharClass::NonWhitespace)),
                    'n' => Ok(RegexNode::Class(CharClass::Literal('\n'))),
                    't' => Ok(RegexNode::Class(CharClass::Literal('\t'))),
                    'r' => Ok(RegexNode::Class(CharClass::Literal('\r'))),
                    other => Ok(RegexNode::Class(CharClass::Literal(other))),
                }
            }
            other => Ok(RegexNode::Class(CharClass::Literal(other))),
        }
    }

    fn parse_bracket(&mut self) -> Result<RegexNode, String> {
        let is_negated = if let Some('^') = self.peek() {
            self.advance();
            true
        } else {
            false
        };

        let mut classes = Vec::new();

        while let Some(ch) = self.peek() {
            if ch == ']' {
                self.advance();
                break;
            }

            let start_ch = if ch == '\\' {
                self.advance();
                let esc = self.advance().ok_or("Escape '\\' tidak lengkap dalam '['")?;
                match esc {
                    'b' => '\x08',
                    'd' => {
                        classes.push(CharClass::Digit);
                        continue;
                    }
                    'D' => {
                        classes.push(CharClass::NonDigit);
                        continue;
                    }
                    'w' => {
                        classes.push(CharClass::Word);
                        continue;
                    }
                    'W' => {
                        classes.push(CharClass::NonWord);
                        continue;
                    }
                    's' => {
                        classes.push(CharClass::Whitespace);
                        continue;
                    }
                    'S' => {
                        classes.push(CharClass::NonWhitespace);
                        continue;
                    }
                    'n' => '\n',
                    't' => '\t',
                    'r' => '\r',
                    other => other,
                }
            } else {
                self.advance().unwrap()
            };

            // Cek rentang karakter: misal 'a-z'
            if let Some('-') = self.peek() {
                if self.pos + 1 < self.chars.len() && self.chars[self.pos + 1] != ']' {
                    self.advance(); // lewati '-'
                    let end_ch = if self.peek() == Some('\\') {
                        self.advance();
                        self.advance().unwrap_or('-')
                    } else {
                        self.advance().unwrap_or('-')
                    };
                    classes.push(CharClass::Range(start_ch, end_ch));
                    continue;
                }
            }

            classes.push(CharClass::Literal(start_ch));
        }

        let union_cls = CharClass::Union(classes);
        if is_negated {
            Ok(RegexNode::Class(CharClass::Negated(Box::new(union_cls))))
        } else {
            Ok(RegexNode::Class(union_cls))
        }
    }
}

pub fn parse_regex(pattern: &str) -> Result<Vec<RegexNode>, String> {
    let mut parser = RegexParser::new(pattern);
    parser.parse()
}

fn match_single(
    node: &RegexNode,
    chars: &[char],
    idx: usize,
    captures: &mut HashMap<usize, (usize, usize)>,
) -> Option<usize> {
    match node {
        RegexNode::Class(cls) => {
            if idx < chars.len() && cls.matches(chars[idx]) {
                Some(idx + 1)
            } else {
                None
            }
        }
        RegexNode::Concat(nodes) => match_here(nodes, chars, idx, captures),
        RegexNode::Alternation(branches) => {
            let snap = captures.clone();
            for b in branches {
                *captures = snap.clone();
                if let Some(end) = match_single(b, chars, idx, captures) {
                    return Some(end);
                }
            }
            None
        }
        _ => match_here(std::slice::from_ref(node), chars, idx, captures),
    }
}

fn match_here(
    nodes: &[RegexNode],
    chars: &[char],
    idx: usize,
    captures: &mut HashMap<usize, (usize, usize)>,
) -> Option<usize> {
    if nodes.is_empty() {
        return Some(idx);
    }

    match &nodes[0] {
        RegexNode::AnchorStart => {
            if idx == 0 {
                match_here(&nodes[1..], chars, idx, captures)
            } else {
                None
            }
        }
        RegexNode::AnchorEnd => {
            if idx == chars.len() {
                match_here(&nodes[1..], chars, idx, captures)
            } else {
                None
            }
        }
        RegexNode::WordBoundary => {
            let is_prev_word = idx > 0 && CharClass::Word.matches(chars[idx - 1]);
            let is_curr_word = idx < chars.len() && CharClass::Word.matches(chars[idx]);
            if is_prev_word != is_curr_word {
                match_here(&nodes[1..], chars, idx, captures)
            } else {
                None
            }
        }
        RegexNode::NonWordBoundary => {
            let is_prev_word = idx > 0 && CharClass::Word.matches(chars[idx - 1]);
            let is_curr_word = idx < chars.len() && CharClass::Word.matches(chars[idx]);
            if is_prev_word == is_curr_word {
                match_here(&nodes[1..], chars, idx, captures)
            } else {
                None
            }
        }
        RegexNode::Group(id, inner) => {
            let marker = RegexNode::GroupEnd(*id, idx);
            let mut combined = vec![*inner.clone(), marker];
            combined.extend_from_slice(&nodes[1..]);
            match_here(&combined, chars, idx, captures)
        }
        RegexNode::GroupEnd(id, start) => {
            let old_cap = captures.insert(*id, (*start, idx));
            if let Some(end) = match_here(&nodes[1..], chars, idx, captures) {
                return Some(end);
            }
            if let Some(prev) = old_cap {
                captures.insert(*id, prev);
            } else {
                captures.remove(id);
            }
            None
        }
        RegexNode::Class(cls) => {
            if idx < chars.len() && cls.matches(chars[idx]) {
                match_here(&nodes[1..], chars, idx + 1, captures)
            } else {
                None
            }
        }
        RegexNode::ZeroOrOne(inner) => {
            let snap = captures.clone();
            if let Some(next_idx) = match_single(inner, chars, idx, captures) {
                if let Some(end) = match_here(&nodes[1..], chars, next_idx, captures) {
                    return Some(end);
                }
            }
            *captures = snap;
            match_here(&nodes[1..], chars, idx, captures)
        }
        RegexNode::ZeroOrMore(inner) => {
            let mut states = vec![(idx, captures.clone())];
            let mut cur = idx;
            while let Some(next) = match_single(inner, chars, cur, captures) {
                if next == cur {
                    break;
                }
                states.push((next, captures.clone()));
                cur = next;
            }
            for (pos, snap) in states.into_iter().rev() {
                *captures = snap;
                if let Some(end) = match_here(&nodes[1..], chars, pos, captures) {
                    return Some(end);
                }
            }
            None
        }
        RegexNode::OneOrMore(inner) => {
            let first_idx = match_single(inner, chars, idx, captures)?;
            let mut states = vec![(first_idx, captures.clone())];
            let mut cur = first_idx;
            while let Some(next) = match_single(inner, chars, cur, captures) {
                if next == cur {
                    break;
                }
                states.push((next, captures.clone()));
                cur = next;
            }
            for (pos, snap) in states.into_iter().rev() {
                *captures = snap;
                if let Some(end) = match_here(&nodes[1..], chars, pos, captures) {
                    return Some(end);
                }
            }
            None
        }
        RegexNode::Repeat(inner, min, max) => {
            let mut states = Vec::new();
            if *min == 0 {
                states.push((idx, captures.clone()));
            }
            let mut cur = idx;
            let limit = max.unwrap_or(usize::MAX);
            let mut count = 0;
            while count < limit {
                if let Some(next) = match_single(inner, chars, cur, captures) {
                    if next == cur {
                        break;
                    }
                    count += 1;
                    if count >= *min {
                        states.push((next, captures.clone()));
                    }
                    cur = next;
                } else {
                    break;
                }
            }
            if count < *min {
                return None;
            }
            for (pos, snap) in states.into_iter().rev() {
                *captures = snap;
                if let Some(end) = match_here(&nodes[1..], chars, pos, captures) {
                    return Some(end);
                }
            }
            None
        }
        RegexNode::Concat(inner_nodes) => {
            let mut combined = inner_nodes.clone();
            combined.extend_from_slice(&nodes[1..]);
            match_here(&combined, chars, idx, captures)
        }
        RegexNode::Alternation(branches) => {
            let snap = captures.clone();
            for branch in branches {
                *captures = snap.clone();
                let mut combined = vec![branch.clone()];
                combined.extend_from_slice(&nodes[1..]);
                if let Some(end) = match_here(&combined, chars, idx, captures) {
                    return Some(end);
                }
            }
            None
        }
    }
}

pub fn find_first(nodes: &[RegexNode], chars: &[char]) -> Option<(usize, usize)> {
    let mut dummy = HashMap::new();
    let has_anchor_start = matches!(nodes.first(), Some(RegexNode::AnchorStart));

    if has_anchor_start {
        match_here(nodes, chars, 0, &mut dummy).map(|end| (0, end))
    } else {
        for start in 0..=chars.len() {
            if let Some(end) = match_here(nodes, chars, start, &mut dummy) {
                return Some((start, end));
            }
        }
        None
    }
}

pub fn find_first_with_captures(
    nodes: &[RegexNode],
    chars: &[char],
) -> Option<(usize, usize, HashMap<usize, (usize, usize)>)> {
    let has_anchor_start = matches!(nodes.first(), Some(RegexNode::AnchorStart));

    if has_anchor_start {
        let mut captures = HashMap::new();
        if let Some(end) = match_here(nodes, chars, 0, &mut captures) {
            return Some((0, end, captures));
        }
        None
    } else {
        for start in 0..=chars.len() {
            let mut captures = HashMap::new();
            if let Some(end) = match_here(nodes, chars, start, &mut captures) {
                return Some((start, end, captures));
            }
        }
        None
    }
}

pub fn find_all(nodes: &[RegexNode], chars: &[char]) -> Vec<(usize, usize)> {
    let mut results = Vec::new();
    let has_anchor_start = matches!(nodes.first(), Some(RegexNode::AnchorStart));

    let mut idx = 0;
    while idx <= chars.len() {
        let mut dummy = HashMap::new();
        if let Some(end) = match_here(nodes, chars, idx, &mut dummy) {
            results.push((idx, end));
            if has_anchor_start {
                break;
            }
            if end > idx {
                idx = end;
            } else {
                idx += 1;
            }
        } else {
            if has_anchor_start {
                break;
            }
            idx += 1;
        }
    }
    results
}

/// Memeriksa apakah suatu pola regex cocok pada teks
pub fn cocok_regex(pola: &str, teks: &str) -> bool {
    let nodes = match parse_regex(pola) {
        Ok(n) => n,
        Err(_) => return false,
    };
    let chars: Vec<char> = teks.chars().collect();
    find_first(&nodes, &chars).is_some()
}

/// Menemukan semua substring yang cocok dengan pola regex
pub fn cari_regex(pola: &str, teks: &str) -> Vec<String> {
    let nodes = match parse_regex(pola) {
        Ok(n) => n,
        Err(_) => return Vec::new(),
    };
    let chars: Vec<char> = teks.chars().collect();
    let matches = find_all(&nodes, &chars);
    matches
        .into_iter()
        .map(|(s, e)| chars[s..e].iter().collect::<String>())
        .collect()
}

/// Mengganti seluruh kemunculan pola regex dengan string pengganti
pub fn ganti_regex(pola: &str, teks: &str, pengganti: &str) -> String {
    let nodes = match parse_regex(pola) {
        Ok(n) => n,
        Err(_) => return teks.to_string(),
    };
    let chars: Vec<char> = teks.chars().collect();
    let matches = find_all(&nodes, &chars);
    if matches.is_empty() {
        return teks.to_string();
    }

    let mut result = String::new();
    let mut last_end = 0;
    for (start, end) in matches {
        if start > last_end {
            result.extend(&chars[last_end..start]);
        }
        result.push_str(pengganti);
        last_end = end;
    }
    if last_end < chars.len() {
        result.extend(&chars[last_end..]);
    }
    result
}

/// Mengekstrak kemunculan pertama pola regex atau mengembalikan galat
pub fn ekstrak_regex(pola: &str, teks: &str) -> Result<String, String> {
    let nodes = parse_regex(pola)?;
    let chars: Vec<char> = teks.chars().collect();
    if let Some((s, e)) = find_first(&nodes, &chars) {
        Ok(chars[s..e].iter().collect())
    } else {
        Err(format!("Pola regex '{}' tidak cocok pada teks", pola))
    }
}

/// Mengekstrak seluruh grup penangkapan (capturing groups) dari pola regex
/// Index 0: Teks lengkap yang cocok (full match)
/// Index 1..n: Grup penangkapan sub-ekspresi (1, 2, ...)
pub fn ekstrak_grup_regex(pola: &str, teks: &str) -> Result<Vec<String>, String> {
    let mut parser = RegexParser::new(pola);
    let (nodes, group_count) = parser.parse_with_group_count()?;
    let chars: Vec<char> = teks.chars().collect();

    if let Some((s, e, captures)) = find_first_with_captures(&nodes, &chars) {
        let mut res = Vec::with_capacity(group_count + 1);
        res.push(chars[s..e].iter().collect());
        for i in 1..=group_count {
            if let Some(&(gs, ge)) = captures.get(&i) {
                res.push(chars[gs..ge].iter().collect());
            } else {
                res.push(String::new());
            }
        }
        Ok(res)
    } else {
        Err(format!("Pola regex '{}' tidak cocok pada teks", pola))
    }
}

/// Memecah teks menjadi daftar substring berdasarkan pemisah pola regex
pub fn pisah_regex(pola: &str, teks: &str) -> Vec<String> {
    let nodes = match parse_regex(pola) {
        Ok(n) => n,
        Err(_) => return vec![teks.to_string()],
    };
    let chars: Vec<char> = teks.chars().collect();
    if chars.is_empty() {
        return vec![String::new()];
    }
    let matches = find_all(&nodes, &chars);
    if matches.is_empty() {
        return vec![teks.to_string()];
    }

    let mut parts = Vec::new();
    let mut last_end = 0;
    for (start, end) in matches {
        parts.push(chars[last_end..start].iter().collect::<String>());
        last_end = end;
    }
    parts.push(chars[last_end..].iter().collect::<String>());
    parts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_literal_and_wildcard() {
        assert!(cocok_regex("dap", "dapcode"));
        assert!(cocok_regex("d.p", "dapcode"));
        assert!(!cocok_regex("d.p", "docode"));
    }

    #[test]
    fn test_quantifiers() {
        assert!(cocok_regex("a*b", "b"));
        assert!(cocok_regex("a*b", "aaab"));
        assert!(cocok_regex("a+b", "ab"));
        assert!(!cocok_regex("a+b", "b"));
        assert!(cocok_regex("colou?r", "color"));
        assert!(cocok_regex("colou?r", "colour"));
    }

    #[test]
    fn test_character_classes() {
        assert!(cocok_regex(r"\d+", "id: 12345"));
        assert!(!cocok_regex(r"^\d+$", "123a45"));
        assert!(cocok_regex(r"^\d+$", "12345"));
        assert!(cocok_regex(r"[a-z]+", "DapCode 2026"));
        assert!(cocok_regex(r"[^0-9]+", "abc"));
    }

    #[test]
    fn test_word_boundaries() {
        assert!(cocok_regex(r"\bcat\b", "black cat"));
        assert!(cocok_regex(r"\bcat\b", "cat"));
        assert!(!cocok_regex(r"\bcat\b", "concatenate"));
        assert!(cocok_regex(r"\Bcat", "concatenate"));
    }

    #[test]
    fn test_capturing_groups() {
        let groups = ekstrak_grup_regex(r"(\d{4})-(\d{2})-(\d{2})", "Tanggal: 2026-10-16").unwrap();
        assert_eq!(groups.len(), 4);
        assert_eq!(groups[0], "2026-10-16");
        assert_eq!(groups[1], "2026");
        assert_eq!(groups[2], "10");
        assert_eq!(groups[3], "16");
    }

    #[test]
    fn test_pisah_regex() {
        let parts = pisah_regex(r",\s*", "kopi, teh,  susu,gula");
        assert_eq!(parts, vec!["kopi", "teh", "susu", "gula"]);

        let words = pisah_regex(r"\s+", "DapCode   bahasa   modern");
        assert_eq!(words, vec!["DapCode", "bahasa", "modern"]);
    }

    #[test]
    fn test_cari_regex() {
        let items = cari_regex(r"\d+", "Kopi 15000, Teh 8000, Susu 12000");
        assert_eq!(items, vec!["15000", "8000", "12000"]);
    }

    #[test]
    fn test_ganti_regex() {
        let res = ganti_regex(r"\d+", "Pesanan: 123 roti dan 456 susu", "XXX");
        assert_eq!(res, "Pesanan: XXX roti dan XXX susu");
    }

    #[test]
    fn test_ekstrak_regex() {
        let res = ekstrak_regex(r"\w+@\w+\.com", "Kontak kami di support@dapcode.com untuk info").unwrap();
        assert_eq!(res, "support@dapcode.com");
    }

    #[test]
    fn test_alternation() {
        assert!(cocok_regex("kucing|anjing", "Saya memelihara anjing"));
        assert!(cocok_regex("kucing|anjing", "Saya memelihara kucing"));
        assert!(!cocok_regex("kucing|anjing", "Saya memelihara kelinci"));
    }
}
