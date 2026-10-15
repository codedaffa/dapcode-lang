//! Pustaka Standar Reguler Ekspresi (Regex) DapCode
//!
//! Modul ini menyediakan mesin pencocokan regex murni tanpa dependensi eksternal,
//! mendukung:
//! - Karakter harfiah & titik sembarang (.)
//! - Kuantifier greedy: *, +, ?, {n}, {n,m}
//! - Kelas karakter: [a-z], [0-9], [^...], \d, \D, \w, \W, \s, \S
//! - Karakter pelarian (escaped characters)
//! - Penanda jangkar: ^ (awal teks), $ (akhir teks)
//! - Alternasi cabang (a|b)
//!
//! Fungsi utama:
//! - `cocok_regex(pola: &str, teks: &str) -> bool`
//! - `cari_regex(pola: &str, teks: &str) -> Vec<String>`
//! - `ganti_regex(pola: &str, teks: &str, pengganti: &str) -> String`
//! - `ekstrak_regex(pola: &str, teks: &str) -> Result<String, String>`

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
    Class(CharClass),
    ZeroOrMore(Box<RegexNode>),
    OneOrMore(Box<RegexNode>),
    ZeroOrOne(Box<RegexNode>),
    Repeat(Box<RegexNode>, usize, Option<usize>),
    Concat(Vec<RegexNode>),
    Alternation(Vec<RegexNode>),
}

pub struct RegexParser {
    chars: Vec<char>,
    pos: usize,
}

impl RegexParser {
    pub fn new(pattern: &str) -> Self {
        RegexParser {
            chars: pattern.chars().collect(),
            pos: 0,
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
        let node = self.parse_alternation()?;
        match node {
            RegexNode::Concat(nodes) => Ok(nodes),
            other => Ok(vec![other]),
        }
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
                let inner = self.parse_alternation()?;
                if let Some(')') = self.advance() {
                    Ok(inner)
                } else {
                    Err("Kurung '(' tidak ditutup dengan ')'".to_string())
                }
            }
            '[' => self.parse_bracket(),
            '\\' => {
                let escaped = match self.advance() {
                    Some(c) => c,
                    None => return Err("Karakter escape '\\' menggantung di akhir pola".to_string()),
                };
                match escaped {
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

fn match_single(node: &RegexNode, chars: &[char], idx: usize) -> Option<usize> {
    match node {
        RegexNode::Class(cls) => {
            if idx < chars.len() && cls.matches(chars[idx]) {
                Some(idx + 1)
            } else {
                None
            }
        }
        RegexNode::Concat(nodes) => match_here(nodes, chars, idx),
        RegexNode::Alternation(branches) => {
            for b in branches {
                if let Some(end) = match_single(b, chars, idx) {
                    return Some(end);
                }
            }
            None
        }
        _ => match_here(std::slice::from_ref(node), chars, idx),
    }
}

fn match_here(nodes: &[RegexNode], chars: &[char], idx: usize) -> Option<usize> {
    if nodes.is_empty() {
        return Some(idx);
    }

    match &nodes[0] {
        RegexNode::AnchorStart => {
            if idx == 0 {
                match_here(&nodes[1..], chars, idx)
            } else {
                None
            }
        }
        RegexNode::AnchorEnd => {
            if idx == chars.len() {
                match_here(&nodes[1..], chars, idx)
            } else {
                None
            }
        }
        RegexNode::Class(cls) => {
            if idx < chars.len() && cls.matches(chars[idx]) {
                match_here(&nodes[1..], chars, idx + 1)
            } else {
                None
            }
        }
        RegexNode::ZeroOrOne(inner) => {
            if let Some(next_idx) = match_single(inner, chars, idx) {
                if let Some(end) = match_here(&nodes[1..], chars, next_idx) {
                    return Some(end);
                }
            }
            match_here(&nodes[1..], chars, idx)
        }
        RegexNode::ZeroOrMore(inner) => {
            let mut positions = vec![idx];
            let mut cur = idx;
            while let Some(next) = match_single(inner, chars, cur) {
                if next == cur {
                    break;
                }
                positions.push(next);
                cur = next;
            }
            for &pos in positions.iter().rev() {
                if let Some(end) = match_here(&nodes[1..], chars, pos) {
                    return Some(end);
                }
            }
            None
        }
        RegexNode::OneOrMore(inner) => {
            let first_idx = match_single(inner, chars, idx)?;
            let mut positions = vec![first_idx];
            let mut cur = first_idx;
            while let Some(next) = match_single(inner, chars, cur) {
                if next == cur {
                    break;
                }
                positions.push(next);
                cur = next;
            }
            for &pos in positions.iter().rev() {
                if let Some(end) = match_here(&nodes[1..], chars, pos) {
                    return Some(end);
                }
            }
            None
        }
        RegexNode::Repeat(inner, min, max) => {
            let mut positions = vec![idx];
            let mut cur = idx;
            let limit = max.unwrap_or(usize::MAX);
            let mut count = 0;
            while count < limit {
                if let Some(next) = match_single(inner, chars, cur) {
                    if next == cur {
                        break;
                    }
                    count += 1;
                    if count >= *min {
                        positions.push(next);
                    }
                    cur = next;
                } else {
                    break;
                }
            }
            if count < *min {
                return None;
            }
            for &pos in positions.iter().rev() {
                if let Some(end) = match_here(&nodes[1..], chars, pos) {
                    return Some(end);
                }
            }
            None
        }
        RegexNode::Concat(inner_nodes) => {
            let mut combined = inner_nodes.clone();
            combined.extend_from_slice(&nodes[1..]);
            match_here(&combined, chars, idx)
        }
        RegexNode::Alternation(branches) => {
            for branch in branches {
                let mut combined = vec![branch.clone()];
                combined.extend_from_slice(&nodes[1..]);
                if let Some(end) = match_here(&combined, chars, idx) {
                    return Some(end);
                }
            }
            None
        }
    }
}

pub fn find_first(nodes: &[RegexNode], chars: &[char]) -> Option<(usize, usize)> {
    let has_anchor_start = matches!(nodes.first(), Some(RegexNode::AnchorStart));

    if has_anchor_start {
        match_here(nodes, chars, 0).map(|end| (0, end))
    } else {
        for start in 0..=chars.len() {
            if let Some(end) = match_here(nodes, chars, start) {
                return Some((start, end));
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
        if let Some(end) = match_here(nodes, chars, idx) {
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
