//! RBF0: the binary XML the UI data (fwui) is stored in.

use std::fmt::Write as _;

use crate::{Result, bad, le};

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Int(i64),
    Bool(bool),
    Float(f64),
    Str(String),
}

impl Value {
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Int(i) => Some(*i as f64),
            Value::Float(f) => Some(*f),
            Value::Str(s) => s.trim().parse().ok(),
            Value::Bool(_) => None,
        }
    }
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s),
            _ => None,
        }
    }
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            Value::Str(s) => s.trim().parse().ok(),
            Value::Int(i) => Some(*i != 0),
            Value::Float(_) => None,
        }
    }
}

/// An element: a name, attributes, children, and either text or a value.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Node {
    pub name: String,
    pub attrs: Vec<(String, Value)>,
    pub children: Vec<Node>,
    pub text: Option<String>,
    pub value: Option<Value>,
}

impl Node {
    pub fn child(&self, name: &str) -> Option<&Node> {
        self.children.iter().find(|c| c.name == name)
    }
    pub fn children_named<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Node> {
        self.children.iter().filter(move |c| c.name == name)
    }
    pub fn attr(&self, name: &str) -> Option<&Value> {
        self.attrs.iter().find(|(n, _)| n == name).map(|(_, v)| v)
    }
    /// The `type` attribute (`<Item type="UIImage">`).
    pub fn type_name(&self) -> Option<&str> {
        self.attr("type").and_then(Value::as_str)
    }
    /// Text or value of a leaf, as a string.
    pub fn string(&self) -> Option<String> {
        if let Some(t) = &self.text {
            return Some(t.clone());
        }
        match &self.value {
            Some(Value::Str(s)) => Some(s.clone()),
            Some(Value::Int(i)) => Some(i.to_string()),
            Some(Value::Float(f)) => Some(f.to_string()),
            Some(Value::Bool(b)) => Some(b.to_string()),
            None => None,
        }
    }
    /// Text of the named child (`<Name>SCRN_PauseMenu</Name>`).
    pub fn text_of(&self, child: &str) -> Option<String> {
        self.child(child).and_then(Node::string)
    }
    pub fn f64_of(&self, child: &str) -> Option<f64> {
        let c = self.child(child)?;
        c.value.as_ref().and_then(Value::as_f64).or_else(|| c.text.as_ref()?.trim().parse().ok())
    }
    pub fn bool_of(&self, child: &str) -> Option<bool> {
        let c = self.child(child)?;
        c.value.as_ref().and_then(Value::as_bool).or_else(|| c.text.as_ref()?.trim().parse().ok())
    }

    pub fn to_xml(&self) -> String {
        let mut s = String::new();
        self.write_xml(&mut s, 0);
        s
    }

    fn write_xml(&self, out: &mut String, depth: usize) {
        let pad = "  ".repeat(depth);
        let _ = write!(out, "{pad}<{}", self.name);
        for (k, v) in &self.attrs {
            let _ = write!(out, " {k}=\"{}\"", escape(&value_text(v)));
        }
        if let Some(v) = &self.value {
            let _ = writeln!(out, " value=\"{}\"/>", escape(&value_text(v)));
            return;
        }
        if self.children.is_empty() {
            match &self.text {
                Some(t) => {
                    let _ = writeln!(out, ">{}</{}>", escape(t), self.name);
                }
                None => out.push_str("/>\n"),
            }
            return;
        }
        out.push_str(">\n");
        if let Some(t) = &self.text {
            let _ = writeln!(out, "{pad}  {}", escape(t));
        }
        for c in &self.children {
            c.write_xml(out, depth + 1);
        }
        let _ = writeln!(out, "{pad}</{}>", self.name);
    }
}

fn value_text(v: &Value) -> String {
    match v {
        Value::Int(i) => i.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Float(f) => f.to_string(),
        Value::Str(s) => s.clone(),
    }
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

pub fn is_rbf(b: &[u8]) -> bool {
    b.starts_with(b"RBF0")
}

/// Parse an RBF0 document into its root element.
pub fn parse(b: &[u8]) -> Result<Node> {
    if !is_rbf(b) {
        return bad("not RBF0");
    }
    let mut r = Reader { b, p: 8, names: vec![None; 256] };
    let mut stack: Vec<Node> = vec![Node { name: String::from("#document"), ..Node::default() }];
    while r.p < b.len() {
        let desc = r.u8()?;
        let kind = r.u8()?;
        match (desc, kind) {
            (0xff, 0xff) => {
                if stack.len() < 2 {
                    return bad("close without open");
                }
                let done = stack.pop().unwrap();
                stack.last_mut().unwrap().children.push(done);
            }
            (0xfd, 0xff) => {
                let n = r.u32()? as usize;
                let raw = r.take(n)?;
                let text = String::from_utf8_lossy(le::cstr(raw, 0)).into_owned();
                stack.last_mut().unwrap().text = Some(text);
            }
            _ => {
                let name = r.name(desc)?;
                if kind == 0 {
                    let (_a, _b, count) = (r.i16()?, r.i16()?, r.i16()?);
                    let mut attrs = Vec::with_capacity(count.max(0) as usize);
                    for _ in 0..count.max(0) {
                        let ad = r.u8()?;
                        let at = r.u8()?;
                        let an = r.name(ad)?;
                        attrs.push((an, r.value(at)?));
                    }
                    stack.push(Node { name, attrs, ..Node::default() });
                } else {
                    let value = r.value(kind)?;
                    stack.last_mut().unwrap().children.push(Node { name, value: Some(value), ..Node::default() });
                }
            }
        }
    }
    if stack.len() != 1 {
        return bad("unclosed elements");
    }
    let mut doc = stack.pop().unwrap();
    match doc.children.len() {
        1 => Ok(doc.children.pop().unwrap()),
        _ => bad("document without a single root"),
    }
}

struct Reader<'a> {
    b: &'a [u8],
    p: usize,
    names: Vec<Option<String>>,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let s = self.b.get(self.p..self.p + n).ok_or_else(|| crate::Error::Format("RBF0 truncated".into()))?;
        self.p += n;
        Ok(s)
    }
    fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }
    fn i16(&mut self) -> Result<i16> {
        Ok(le::i16(self.take(2)?, 0))
    }
    fn u16(&mut self) -> Result<u16> {
        Ok(le::u16(self.take(2)?, 0))
    }
    fn u32(&mut self) -> Result<u32> {
        Ok(le::u32(self.take(4)?, 0))
    }
    fn name(&mut self, desc: u8) -> Result<String> {
        if let Some(n) = &self.names[desc as usize] {
            return Ok(n.clone());
        }
        let len = self.u16()? as usize;
        let n = String::from_utf8_lossy(self.take(len)?).into_owned();
        self.names[desc as usize] = Some(n.clone());
        Ok(n)
    }
    fn value(&mut self, kind: u8) -> Result<Value> {
        Ok(match kind {
            0x10 => Value::Int(le::i64(self.take(8)?, 0)),
            0x20 => Value::Bool(true),
            0x30 => Value::Bool(false),
            0x40 => Value::Float(le::f64(self.take(8)?, 0)),
            0x60 => {
                let n = self.u16()? as usize;
                Value::Str(String::from_utf8_lossy(self.take(n)?).into_owned())
            }
            k => return bad(format!("RBF0 value type {k:#x} at {:#x}", self.p)),
        })
    }
}
