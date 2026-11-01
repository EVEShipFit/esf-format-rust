//! Reading the text form into lines, without a Catalog (SPEC §4.1, grammar.ebnf).

use std::borrow::Cow;

use crate::{Error, Location, State};

pub(crate) const SIGILS: &[u8] = b":+{!@/\"";

pub(crate) struct EmptyLine {
    pub number: usize,
    pub rack: Location,
    pub count: Option<u32>,
    pub index: Option<u32>,
}

#[derive(Default)]
pub(crate) struct ItemLine<'a> {
    pub number: usize,
    pub name: Cow<'a, str>,
    pub count: Option<u32>,
    pub fit_name: Option<Cow<'a, str>>,
    pub charge: Option<Cow<'a, str>>,
    pub charge_count: Option<u32>,
    pub mutaplasmid: Option<Cow<'a, str>>,
    pub overrides: Vec<(&'a str, &'a str)>,
    pub state: Option<State>,
    pub location: Option<Location>,
    pub index: Option<u32>,
}

pub(crate) enum Line<'a> {
    Empty(EmptyLine),
    Item(ItemLine<'a>),
}

pub(crate) struct FitBlock<'a> {
    pub hull_line: usize,
    pub hull: Option<Cow<'a, str>>,
    pub fit_name: Option<Cow<'a, str>>,
    pub mode: Option<Cow<'a, str>>,
    pub lines: Vec<Line<'a>>,
}

pub(crate) fn is_count(token: &str) -> bool {
    token
        .strip_suffix('x')
        .is_some_and(|digits| !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()))
}

struct Cursor<'a> {
    number: usize,
    s: &'a str,
    i: usize,
}

type Parsed<T> = Result<T, Error>;

impl<'a> Cursor<'a> {
    fn error<T>(&self, message: impl Into<String>) -> Parsed<T> {
        Err(Error::at(self.number, message))
    }

    fn peek(&self) -> Option<u8> {
        self.s.as_bytes().get(self.i).copied()
    }

    fn rest(&self) -> &'a str {
        &self.s[self.i..]
    }

    fn token(&self) -> &'a str {
        let rest = self.rest();
        &rest[..rest.find(' ').unwrap_or(rest.len())]
    }

    fn take_while(&mut self, keep: impl Fn(u8) -> bool) -> &'a str {
        let rest = self.rest();
        let len = rest.bytes().position(|b| !keep(b)).unwrap_or(rest.len());
        self.i += len;
        &rest[..len]
    }

    fn skip_spaces(&mut self) -> bool {
        !self.take_while(|b| b == b' ').is_empty()
    }

    /// Step over the separating space; false at the end of the line or a comment.
    fn next_element(&mut self) -> Parsed<bool> {
        let spaced = self.skip_spaces();
        if self.i >= self.s.len() {
            return Ok(false);
        }
        if self.rest().starts_with("//") {
            if !spaced {
                return self.error("a comment needs a space before it");
            }
            self.i = self.s.len();
            return Ok(false);
        }
        if !spaced {
            return self.error(format!("expected a space before {:?}", self.token()));
        }
        Ok(true)
    }

    fn positive(&self, digits: &str, what: &str) -> Parsed<u32> {
        let digits = digits.trim_start_matches('0');
        if digits.is_empty() {
            return self.error(format!("{what} must be at least 1"));
        }
        digits
            .parse()
            .or_else(|_| self.error(format!("{what} is too large")))
    }

    fn finish(&mut self) -> Parsed<()> {
        if self.next_element()? {
            return self.error(format!("unexpected {:?}", self.token()));
        }
        Ok(())
    }

    fn count(&mut self) -> Parsed<Option<u32>> {
        let token = self.token();
        if !is_count(token) {
            return Ok(None);
        }
        self.i += token.len();
        if !self.next_element()? {
            return self.error("a count must be followed by a type name");
        }
        self.positive(&token[..token.len() - 1], "a count")
            .map(Some)
    }

    fn quoted(&mut self) -> Parsed<Cow<'a, str>> {
        let bytes = self.s.as_bytes();
        let start = self.i + 1;
        let mut end = start;
        let mut escaped = false;
        loop {
            match bytes.get(end) {
                None => return self.error("unterminated quoted string"),
                Some(b'"') if bytes.get(end + 1) == Some(&b'"') => {
                    escaped = true;
                    end += 2;
                }
                Some(b'"') => break,
                Some(_) => end += 1,
            }
        }
        if end == start {
            return self.error("a quoted string cannot be empty");
        }
        self.i = end + 1;
        let inner = &self.s[start..end];
        if escaped {
            Ok(Cow::Owned(inner.replace("\"\"", "\"")))
        } else {
            Ok(Cow::Borrowed(inner))
        }
    }

    fn word(&mut self) {
        self.i += self.token().len();
    }

    fn type_name(&mut self) -> Parsed<Cow<'a, str>> {
        match self.peek() {
            Some(b'"') => return self.quoted(),
            None | Some(b' ') => return self.error("expected a type name"),
            Some(c) if SIGILS.contains(&c) => return self.error("expected a type name"),
            Some(c @ (b'-' | b'%')) => {
                return self.error(format!(
                    "a name starting with '{}' must be quoted",
                    c as char
                ));
            }
            Some(_) => {}
        }
        let start = self.i;
        let mut single_spaced = true;
        self.word();
        loop {
            let end = self.i;
            let gap = self.take_while(|b| b == b' ').len();
            if gap == 0 || self.peek().is_none_or(|c| SIGILS.contains(&c)) {
                self.i = end;
                break;
            }
            single_spaced &= gap == 1;
            self.word();
        }
        let name = &self.s[start..self.i];
        if single_spaced {
            return Ok(Cow::Borrowed(name));
        }
        let words: Vec<&str> = name.split(' ').filter(|w| !w.is_empty()).collect();
        Ok(Cow::Owned(words.join(" ")))
    }

    fn location(&mut self) -> Parsed<(Location, Option<u32>)> {
        let token = self.token();
        self.i += 1;
        let name = self.take_while(|b| b.is_ascii_lowercase());
        let index = self.take_while(|b| b.is_ascii_digit());
        let Some(location) = Location::from_name(name) else {
            return self.error(format!("unknown location {token:?}"));
        };
        if index.is_empty() {
            return Ok((location, None));
        }
        if !location.is_rack() {
            return self.error(format!("unknown rack {name:?}"));
        }
        Ok((location, Some(self.positive(index, "a slot number")?)))
    }

    fn state(&mut self) -> Parsed<State> {
        let rest = &self.rest()[1..];
        let Some(state) = State::ALL
            .into_iter()
            .find(|s| rest.starts_with(s.as_str()))
        else {
            return self.error(format!("unknown state {:?}", self.token()));
        };
        self.i += 1 + state.as_str().len();
        Ok(state)
    }

    fn overrides(&mut self) -> Parsed<Vec<(&'a str, &'a str)>> {
        self.i += 1;
        self.skip_spaces();
        let mut pairs = Vec::new();
        loop {
            let Some(pair) = self.override_pair() else {
                return self.error("malformed override; expected 'attribute value'");
            };
            pairs.push(pair);
            self.skip_spaces();
            match self.peek() {
                Some(b',') => {
                    self.i += 1;
                    self.skip_spaces();
                }
                Some(b'}') => {
                    self.i += 1;
                    return Ok(pairs);
                }
                _ => return self.error("malformed overrides; expected ',' or '}'"),
            }
        }
    }

    /// `name +value`, where a name is `[A-Za-z][A-Za-z0-9]*` and a value `-?[0-9]+(\.[0-9]+)?`.
    fn override_pair(&mut self) -> Option<(&'a str, &'a str)> {
        if !self.peek().is_some_and(|c| c.is_ascii_alphabetic()) {
            return None;
        }
        let name = self.take_while(|b| b.is_ascii_alphanumeric());
        if !self.skip_spaces() {
            return None;
        }
        let value_start = self.i;
        if self.peek() == Some(b'-') {
            self.i += 1;
        }
        if self.take_while(|b| b.is_ascii_digit()).is_empty() {
            return None;
        }
        let whole_end = self.i;
        if self.peek() == Some(b'.') {
            self.i += 1;
            if self.take_while(|b| b.is_ascii_digit()).is_empty() {
                self.i = whole_end;
            }
        }
        Some((name, &self.s[value_start..self.i]))
    }
}

fn hull<'a>(cur: &mut Cursor<'a>) -> Parsed<FitBlock<'a>> {
    let mut block = FitBlock {
        hull_line: cur.number,
        hull: None,
        fit_name: None,
        mode: None,
        lines: Vec::new(),
    };
    cur.skip_spaces();
    if cur.token() == "-" {
        cur.i += 1;
        if cur.next_element()? {
            if cur.peek() != Some(b'"') {
                return cur.error("a hull line of '-' takes only a fit name");
            }
            block.fit_name = Some(cur.quoted()?);
        }
        cur.finish()?;
        return Ok(block);
    }

    if is_count(cur.token()) {
        return cur.error("the hull line cannot have a count");
    }
    block.hull = Some(cur.type_name()?);
    let mut more = cur.next_element()?;
    if more && cur.peek() == Some(b'"') {
        block.fit_name = Some(cur.quoted()?);
        more = cur.next_element()?;
    }
    if more {
        if cur.peek() != Some(b'/') {
            return cur.error(format!("unexpected {:?} on the hull line", cur.token()));
        }
        cur.i += 1;
        block.mode = Some(cur.type_name()?);
    }
    cur.finish()?;
    Ok(block)
}

fn empty_slot(cur: &mut Cursor<'_>, count: Option<u32>) -> Parsed<EmptyLine> {
    cur.i += 1;
    if !cur.next_element()? || cur.peek() != Some(b'@') {
        return cur.error("an empty slot must name its rack");
    }
    let (rack, index) = cur.location()?;
    if !rack.is_rack() {
        return cur.error("an empty slot must name a rack");
    }
    if index.is_some() && count.is_some() {
        return cur.error("a count and a pinned slot are mutually exclusive");
    }
    cur.finish()?;
    Ok(EmptyLine {
        number: cur.number,
        rack,
        count,
        index,
    })
}

fn entry<'a>(cur: &mut Cursor<'a>) -> Parsed<Line<'a>> {
    cur.skip_spaces();
    let count = cur.count()?;
    if cur.token() == "-" {
        return empty_slot(cur, count).map(Line::Empty);
    }

    let mut line = ItemLine {
        number: cur.number,
        name: cur.type_name()?,
        count,
        ..ItemLine::default()
    };
    let mut more = cur.next_element()?;

    if more && cur.peek() == Some(b'"') {
        line.fit_name = Some(cur.quoted()?);
        if cur.next_element()? {
            if cur.peek() != Some(b'@') {
                return cur.error("a reference takes only a count and a location");
            }
            let (location, index) = cur.location()?;
            if index.is_some() || !location.is_stored() {
                return cur.error("a reference can only be stored");
            }
            line.location = Some(location);
            cur.finish()?;
        }
        return Ok(Line::Item(line));
    }

    let mut stage = 0;
    while more {
        match cur.peek() {
            Some(b':') if stage < 1 => {
                cur.i += 1;
                line.charge_count = cur.count()?;
                line.charge = Some(cur.type_name()?);
                stage = 1;
            }
            Some(b'+') if stage < 2 => {
                cur.i += 1;
                line.mutaplasmid = Some(cur.type_name()?);
                stage = 2;
            }
            Some(b'{') if stage < 3 => {
                line.overrides = cur.overrides()?;
                stage = 3;
            }
            Some(b'!') if stage < 4 => {
                line.state = Some(cur.state()?);
                stage = 4;
            }
            Some(b'@') if stage < 5 => {
                let (location, index) = cur.location()?;
                if index.is_none() && location.is_rack() {
                    return cur
                        .error("an item names a rack only to pin a slot, which needs an index");
                }
                if index.is_some() && count.is_some() {
                    return cur.error("a count and a pinned slot are mutually exclusive");
                }
                line.location = Some(location);
                line.index = index;
                stage = 5;
            }
            _ => return cur.error(format!("unexpected {:?}", cur.token())),
        }
        more = cur.next_element()?;
    }
    Ok(Line::Item(line))
}

fn is_blank(text: &str) -> bool {
    let text = text.trim_start_matches(' ');
    text.is_empty() || text.starts_with("//")
}

/// The name and version of a header line: ` *%([a-z][a-z0-9-]*)/([0-9]+) *`.
fn header(text: &str) -> Option<(&str, &str)> {
    let (name, version) = text.trim_matches(' ').strip_prefix('%')?.split_once('/')?;
    let name_ok = name.starts_with(|c: char| c.is_ascii_lowercase())
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
    let version_ok = !version.is_empty() && version.bytes().all(|b| b.is_ascii_digit());
    (name_ok && version_ok).then_some((name, version))
}

/// Split a text document into fits; other blocks are skipped.
pub(crate) fn parse(data: &[u8]) -> Parsed<Vec<FitBlock<'_>>> {
    let text = std::str::from_utf8(data)
        .map_err(|e| Error::new(format!("not valid UTF-8 at byte {}", e.valid_up_to())))?;
    let text = text.strip_suffix('\n').unwrap_or(text);

    let mut fits: Vec<FitBlock<'_>> = Vec::new();
    let mut in_fit = false;
    let mut header_line = None;
    let mut in_block = false;
    for (raw, number) in text.split('\n').zip(1..) {
        let raw = raw.strip_suffix('\r').unwrap_or(raw);
        if raw.contains('\r') {
            return Err(Error::at(number, "a CR must be followed by an LF"));
        }

        if raw.trim_start_matches(' ').starts_with('%') {
            let Some((name, version)) = header(raw) else {
                return Err(Error::at(number, "malformed header"));
            };
            if let Some(line) = header_line {
                return Err(Error::at(line, "a fit needs a hull line"));
            }
            in_block = true;
            in_fit = false;
            if name == "esf" {
                if version != "1" {
                    return Err(Error::at(
                        number,
                        format!("unsupported version esf/{version}"),
                    ));
                }
                header_line = Some(number);
            }
            continue;
        }

        if !in_block {
            return Err(Error::at(number, "a document starts with a header"));
        }
        if is_blank(raw) {
            continue;
        }

        let mut cur = Cursor {
            number,
            s: raw,
            i: 0,
        };
        if header_line.take().is_some() {
            fits.push(hull(&mut cur)?);
            in_fit = true;
        } else if in_fit {
            let line = entry(&mut cur)?;
            fits.last_mut().expect("in a fit").lines.push(line);
        }
    }

    if let Some(line) = header_line {
        return Err(Error::at(line, "a fit needs a hull line"));
    }
    if fits.is_empty() {
        return Err(Error::new("a document holds at least one fit"));
    }
    Ok(fits)
}
