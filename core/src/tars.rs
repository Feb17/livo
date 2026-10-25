//! 虎牙用到的最小 TARS 编解码：整数、字符串、字节数组、结构体、map，外加 TUP3 请求包。
//! 头部：tag<15 时一个字节 (tag<<4 | type)，否则 0xF0|type 后跟 tag；整数一律大端、按值压缩宽度。

const INT8: u8 = 0;
const INT16: u8 = 1;
const INT32: u8 = 2;
const INT64: u8 = 3;
const FLOAT: u8 = 4;
const DOUBLE: u8 = 5;
const STRING1: u8 = 6;
const STRING4: u8 = 7;
const MAP: u8 = 8;
const LIST: u8 = 9;
const STRUCT_BEGIN: u8 = 10;
const STRUCT_END: u8 = 11;
const ZERO: u8 = 12;
const SIMPLE_LIST: u8 = 13;

#[derive(Default)]
pub struct Writer(pub Vec<u8>);

impl Writer {
    fn head(&mut self, tag: u8, ty: u8) {
        if tag < 15 {
            self.0.push(tag << 4 | ty);
        } else {
            self.0.extend([0xF0 | ty, tag]);
        }
    }

    pub fn int(&mut self, tag: u8, v: i64) -> &mut Self {
        if v == 0 {
            self.head(tag, ZERO);
        } else if let Ok(x) = i8::try_from(v) {
            self.head(tag, INT8);
            self.0.push(x as u8);
        } else if let Ok(x) = i16::try_from(v) {
            self.head(tag, INT16);
            self.0.extend(x.to_be_bytes());
        } else if let Ok(x) = i32::try_from(v) {
            self.head(tag, INT32);
            self.0.extend(x.to_be_bytes());
        } else {
            self.head(tag, INT64);
            self.0.extend(v.to_be_bytes());
        }
        self
    }

    pub fn string(&mut self, tag: u8, s: &str) -> &mut Self {
        if let Ok(n) = u8::try_from(s.len()) {
            self.head(tag, STRING1);
            self.0.push(n);
        } else {
            self.head(tag, STRING4);
            self.0.extend((s.len() as u32).to_be_bytes());
        }
        self.0.extend(s.as_bytes());
        self
    }

    pub fn bytes(&mut self, tag: u8, b: &[u8]) -> &mut Self {
        self.head(tag, SIMPLE_LIST);
        self.head(0, INT8);
        self.int(0, b.len() as i64);
        self.0.extend(b);
        self
    }

    pub fn struct_(&mut self, tag: u8, body: &Writer) -> &mut Self {
        self.head(tag, STRUCT_BEGIN);
        self.0.extend(&body.0);
        self.head(0, STRUCT_END);
        self
    }

    pub fn map_bytes(&mut self, tag: u8, entries: &[(&str, &[u8])]) -> &mut Self {
        self.head(tag, MAP);
        self.int(0, entries.len() as i64);
        for (k, v) in entries {
            self.string(0, k);
            self.bytes(1, v);
        }
        self
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Int(i64),
    Float(f64),
    Str(String),
    Bytes(Vec<u8>),
    Struct(Fields),
    List(Vec<Value>),
    Map(Vec<(Value, Value)>),
}

/// 按 tag 存的一组字段（结构体体或顶层字段序列）。
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Fields(pub Vec<(u8, Value)>);

impl Fields {
    fn get(&self, tag: u8) -> Option<&Value> {
        self.0.iter().find(|(t, _)| *t == tag).map(|(_, v)| v)
    }
    pub fn int(&self, tag: u8) -> i64 {
        match self.get(tag) {
            Some(Value::Int(v)) => *v,
            _ => 0,
        }
    }
    pub fn string(&self, tag: u8) -> String {
        match self.get(tag) {
            Some(Value::Str(s)) => s.clone(),
            _ => String::new(),
        }
    }
    pub fn bytes(&self, tag: u8) -> &[u8] {
        match self.get(tag) {
            Some(Value::Bytes(b)) => b,
            _ => &[],
        }
    }
    pub fn struct_(&self, tag: u8) -> Fields {
        match self.get(tag) {
            Some(Value::Struct(f)) => f.clone(),
            _ => Fields::default(),
        }
    }
    /// map<string, bytes> 里取某个 key 的值。
    pub fn map_bytes(&self, tag: u8, key: &str) -> &[u8] {
        let Some(Value::Map(m)) = self.get(tag) else { return &[] };
        m.iter()
            .find_map(|(k, v)| match (k, v) {
                (Value::Str(k), Value::Bytes(b)) if k == key => Some(b.as_slice()),
                _ => None,
            })
            .unwrap_or(&[])
    }
}

struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl Reader<'_> {
    fn take(&mut self, n: usize) -> Option<&[u8]> {
        let s = self.buf.get(self.pos..self.pos.checked_add(n)?)?;
        self.pos += n;
        Some(s)
    }

    fn head(&mut self) -> Option<(u8, u8)> {
        let b = *self.take(1)?.first()?;
        let (tag, ty) = (b >> 4, b & 0x0F);
        Some(if tag == 15 { (*self.take(1)?.first()?, ty) } else { (tag, ty) })
    }

    fn int_of(&mut self, ty: u8) -> Option<i64> {
        Some(match ty {
            ZERO => 0,
            INT8 => self.take(1)?[0] as i8 as i64,
            INT16 => i16::from_be_bytes(self.take(2)?.try_into().ok()?) as i64,
            INT32 => i32::from_be_bytes(self.take(4)?.try_into().ok()?) as i64,
            INT64 => i64::from_be_bytes(self.take(8)?.try_into().ok()?),
            _ => return None,
        })
    }

    /// 长度字段本身是一个 tag 0 的整数。
    fn len(&mut self) -> Option<usize> {
        let (_, ty) = self.head()?;
        usize::try_from(self.int_of(ty)?).ok()
    }

    fn value(&mut self, ty: u8) -> Option<Value> {
        Some(match ty {
            ZERO | INT8 | INT16 | INT32 | INT64 => Value::Int(self.int_of(ty)?),
            FLOAT => Value::Float(f32::from_be_bytes(self.take(4)?.try_into().ok()?) as f64),
            DOUBLE => Value::Float(f64::from_be_bytes(self.take(8)?.try_into().ok()?)),
            STRING1 | STRING4 => {
                let n = if ty == STRING1 { self.take(1)?[0] as usize } else { u32::from_be_bytes(self.take(4)?.try_into().ok()?) as usize };
                Value::Str(String::from_utf8_lossy(self.take(n)?).into_owned())
            }
            MAP => {
                let n = self.len()?;
                let mut m = Vec::with_capacity(n.min(1024));
                for _ in 0..n {
                    let (_, kt) = self.head()?;
                    let k = self.value(kt)?;
                    let (_, vt) = self.head()?;
                    m.push((k, self.value(vt)?));
                }
                Value::Map(m)
            }
            LIST => {
                let n = self.len()?;
                let mut l = Vec::with_capacity(n.min(1024));
                for _ in 0..n {
                    let (_, t) = self.head()?;
                    l.push(self.value(t)?);
                }
                Value::List(l)
            }
            SIMPLE_LIST => {
                self.head()?; // 元素类型头，固定 INT8
                let n = self.len()?;
                Value::Bytes(self.take(n)?.to_vec())
            }
            STRUCT_BEGIN => Value::Struct(self.fields(true)?),
            _ => return None,
        })
    }

    fn fields(&mut self, in_struct: bool) -> Option<Fields> {
        let mut out = Fields::default();
        while self.pos < self.buf.len() {
            let (tag, ty) = self.head()?;
            if ty == STRUCT_END {
                return Some(out);
            }
            out.0.push((tag, self.value(ty)?));
        }
        // 结构体没遇到结束符说明数据被截断
        (!in_struct).then_some(out)
    }
}

/// 解一段顶层字段序列；数据损坏时返回空。
pub fn decode(buf: &[u8]) -> Fields {
    Reader { buf, pos: 0 }.fields(false).unwrap_or_default()
}

/// TUP3 请求包：4 字节大端总长 + RequestPacket，参数放在 sBuffer 的 map 里（key "tReq"）。
pub fn tup_request(servant: &str, func: &str, req: &Writer) -> Vec<u8> {
    let mut arg = Writer::default();
    arg.struct_(0, req);
    let mut data = Writer::default();
    data.map_bytes(0, &[("tReq", &arg.0)]);
    let mut pkt = Writer::default();
    pkt.int(1, 3).int(2, 0).int(3, 0).int(4, 0).string(5, servant).string(6, func).bytes(7, &data.0).int(8, 0).map_bytes(9, &[]).map_bytes(10, &[]);
    let mut out = ((pkt.0.len() + 4) as u32).to_be_bytes().to_vec();
    out.extend(pkt.0);
    out
}

/// 解 TUP3 响应，返回 (返回码, tRsp 结构体)。
pub fn tup_response(buf: &[u8]) -> (i64, Fields) {
    let pkt = decode(buf.get(4..).unwrap_or_default());
    let data = decode(pkt.bytes(7));
    let code = decode(data.map_bytes(0, "")).int(0);
    (code, decode(data.map_bytes(0, "tRsp")).struct_(0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_fields() {
        let mut inner = Writer::default();
        inner.int(0, 300).string(2, "小明");
        let mut w = Writer::default();
        w.int(0, 7).int(1, -5).int(2, 0).int(20, 1 << 40).string(3, &"x".repeat(300)).bytes(4, &[1, 2, 3]).struct_(5, &inner);
        let f = decode(&w.0);
        assert_eq!((f.int(0), f.int(1), f.int(2), f.int(20)), (7, -5, 0, 1 << 40));
        assert_eq!(f.string(3).len(), 300);
        assert_eq!(f.bytes(4), &[1, 2, 3]);
        assert_eq!((f.struct_(5).int(0), f.struct_(5).string(2)), (300, "小明".to_string()));
    }

    #[test]
    fn tup_roundtrip() {
        // 用请求编码器拼一个“响应”：返回码 0 + tRsp
        let mut rsp = Writer::default();
        rsp.string(0, "token");
        let pkt = tup_request("liveui", "f", &rsp);
        let outer = decode(&pkt[4..]);
        assert_eq!((outer.int(1), outer.string(5), outer.string(6)), (3, "liveui".into(), "f".into()));
        let mut code = Writer::default();
        code.int(0, 0);
        let mut arg = Writer::default();
        arg.struct_(0, &rsp);
        let mut data = Writer::default();
        data.map_bytes(0, &[("", &code.0), ("tRsp", &arg.0)]);
        let mut p = Writer::default();
        p.int(1, 3).bytes(7, &data.0);
        let mut buf = vec![0; 4];
        buf.extend(p.0);
        let (c, f) = tup_response(&buf);
        assert_eq!((c, f.string(0)), (0, "token".into()));
    }

    #[test]
    fn truncated_input_does_not_panic() {
        let mut w = Writer::default();
        w.string(0, "hello").bytes(1, &[9; 40]);
        for n in 0..w.0.len() {
            let _ = decode(&w.0[..n]);
        }
    }
}
