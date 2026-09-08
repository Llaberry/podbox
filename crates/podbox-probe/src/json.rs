//! A JSON writer, because `TODO/probe.md` T-0110 needs `--json` beside the
//! one-word stdout line and `TODO/deps.md` T-0908's sweep has not run.
//!
//! ⛔ Escaping is the whole of it. A probe reason carries file paths and errno
//! text, and one unescaped byte turns a machine-readable answer into a parse
//! error at the consumer, which is where it is least useful.

/// Append `s` as a JSON string, quotes included.
pub fn push_str(out: &mut String, s: &str) {
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            // RFC 8259: everything below 0x20 must be escaped. U+2028 and
            // U+2029 are legal JSON and illegal in a JavaScript string
            // literal, so they are escaped too rather than left to break one
            // consumer in a way no other consumer reveals.
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            '\u{2028}' => out.push_str("\\u2028"),
            '\u{2029}' => out.push_str("\\u2029"),
            c => out.push(c),
        }
    }
    out.push('"');
}

/// A minimal builder. It tracks whether a comma is due so no caller has to.
pub struct Obj<'a> {
    out: &'a mut String,
    first: bool,
}

impl<'a> Obj<'a> {
    pub fn new(out: &'a mut String) -> Obj<'a> {
        out.push('{');
        Obj { out, first: true }
    }

    fn key(&mut self, k: &str) {
        if !self.first {
            self.out.push(',');
        }
        self.first = false;
        push_str(self.out, k);
        self.out.push(':');
    }

    pub fn str(&mut self, k: &str, v: &str) -> &mut Self {
        self.key(k);
        push_str(self.out, v);
        self
    }

    pub fn num(&mut self, k: &str, v: i64) -> &mut Self {
        self.key(k);
        self.out.push_str(&v.to_string());
        self
    }

    pub fn unum(&mut self, k: &str, v: u64) -> &mut Self {
        self.key(k);
        self.out.push_str(&v.to_string());
        self
    }

    pub fn bool(&mut self, k: &str, v: bool) -> &mut Self {
        self.key(k);
        self.out.push_str(if v { "true" } else { "false" });
        self
    }

    /// ⛔ An unknown value is `null`, never a zero or an empty string. A blank
    /// gets checked; a fabricated zero does not.
    pub fn null(&mut self, k: &str) -> &mut Self {
        self.key(k);
        self.out.push_str("null");
        self
    }

    pub fn opt_str(&mut self, k: &str, v: Option<&str>) -> &mut Self {
        match v {
            Some(s) => self.str(k, s),
            None => self.null(k),
        }
    }

    pub fn opt_num(&mut self, k: &str, v: Option<i64>) -> &mut Self {
        match v {
            Some(n) => self.num(k, n),
            None => self.null(k),
        }
    }

    pub fn opt_unum(&mut self, k: &str, v: Option<u64>) -> &mut Self {
        match v {
            Some(n) => self.unum(k, n),
            None => self.null(k),
        }
    }

    /// Nest an object under `k`, built by `f`.
    pub fn obj(&mut self, k: &str, f: impl FnOnce(&mut Obj)) -> &mut Self {
        self.key(k);
        let mut inner = Obj::new(self.out);
        f(&mut inner);
        inner.end();
        self
    }

    /// Nest an array under `k`. `f` receives a writer for each element.
    pub fn arr(&mut self, k: &str, f: impl FnOnce(&mut Arr)) -> &mut Self {
        self.key(k);
        self.out.push('[');
        let mut a = Arr {
            out: self.out,
            first: true,
        };
        f(&mut a);
        self.out.push(']');
        self
    }

    pub fn end(self) {
        self.out.push('}');
    }
}

pub struct Arr<'a> {
    out: &'a mut String,
    first: bool,
}

impl Arr<'_> {
    fn comma(&mut self) {
        if !self.first {
            self.out.push(',');
        }
        self.first = false;
    }

    pub fn obj(&mut self, f: impl FnOnce(&mut Obj)) {
        self.comma();
        let mut inner = Obj::new(self.out);
        f(&mut inner);
        inner.end();
    }

    pub fn num(&mut self, v: i64) {
        self.comma();
        self.out.push_str(&v.to_string());
    }

    pub fn str(&mut self, v: &str) {
        self.comma();
        push_str(self.out, v);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_reason_carrying_quotes_and_control_bytes_survives() {
        let mut s = String::new();
        push_str(&mut s, "he said \"no\"\tand\nleft\\");
        assert_eq!(s, r#""he said \"no\"\tand\nleft\\""#);
    }

    #[test]
    fn a_nested_document_closes_every_brace() {
        let mut s = String::new();
        let mut o = Obj::new(&mut s);
        o.str("rung", "chroot")
            .obj("mounts", |m| {
                m.str("create", "ok").str("attach", "denied");
            })
            .arr("probes", |a| {
                a.obj(|p| {
                    p.str("name", "chroot(/tmp)").opt_num("errno", None);
                });
            });
        o.end();
        assert_eq!(
            s,
            r#"{"rung":"chroot","mounts":{"create":"ok","attach":"denied"},"probes":[{"name":"chroot(/tmp)","errno":null}]}"#
        );
    }

    #[test]
    fn an_unknown_value_is_null_rather_than_zero() {
        let mut s = String::new();
        let mut o = Obj::new(&mut s);
        o.opt_unum("inodes_free", None);
        o.end();
        assert_eq!(s, r#"{"inodes_free":null}"#);
    }
}
