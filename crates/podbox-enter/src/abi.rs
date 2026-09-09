//! [`TODO/interpose.md`](../../../TODO/interpose.md) T-0709: choose the
//! interposer by `DT_NEEDED`, and refuse on the version predicate.
//!
//! ⭐ **A READER, and nothing here is executed to find anything out.** T-0702
//! establishes that one object per libc is required. Which object a payload
//! gets is then answered by `readelf`-level facts, and answering it instead by
//! loading one and seeing what happens costs a failed container and produces an
//! error naming a symbol, which sends the reader to debug the symbol rather
//! than the mechanism.
//!
//! Four facts, all of them measured by `experiments/80-interposer-abi.sh` and
//! recorded in `experiments/results/interposer-abi.txt`:
//!
//! 1. **the SONAME discriminator.** glibc's libc declares SONAME `libc.so.6`;
//!    musl's declares none, so an object linked against it records `libc.so` in
//!    `DT_NEEDED`. ⛔ There is no ambiguity and no heuristic;
//! 2. **the cross-libc arms**, in both directions: a glibc object in a musl
//!    payload is `__snprintf_chk: symbol not found`, and a musl object in a
//!    glibc payload is `libc.so: invalid ELF header`, because on a glibc host
//!    that name is a linker script;
//! 3. **the version predicate**, predicted from ELF and then confirmed by the
//!    loader: the object imported up to `GLIBC_2.34`, the payload libc declared
//!    up to `GLIBC_2.31`, the prediction was `refused`, and the loader said
//!    `version 'GLIBC_2.34' not found (required by /i.so)`;
//! 4. ⛔ **the trap that makes a naive reader refuse everything.** A shipped
//!    `libc.so.6` is stripped: on this host `.symtab` has **0** defined symbols
//!    and `.dynsym` has **3136**. A reader asking `.symtab` concludes the
//!    payload's libc defines nothing, refuses every artefact, and reads exactly
//!    like a check that works. [`Elf::defined`] reads `.dynsym` and this module
//!    never looks at `.symtab` at all.
//!
//! ⚠ **The libc is FOUND rather than named.** There is no table of
//! `lib/x86_64-linux-gnu/libc.so.6` here: the payload's own `PT_INTERP` says
//! which loader will run it, and for musl that file IS the libc while for glibc
//! the libc sits in the loader's own directory. A table would be a list of the
//! distributions somebody thought of.

use std::collections::BTreeMap;

/// Which C library an artefact belongs to.
///
/// ⛔ `Unknown` is a third state and never folded into either. A rootfs whose
/// libc podbox could not identify is one podbox refuses to preload into, with
/// that as the reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flavour {
    Glibc,
    Musl,
    Unknown,
}

impl Flavour {
    pub fn word(self) -> &'static str {
        match self {
            Flavour::Glibc => "glibc",
            Flavour::Musl => "musl",
            Flavour::Unknown => "unknown",
        }
    }

    /// What an object's `DT_NEEDED` says it wants.
    ///
    /// ⭐ Check A of `experiments/results/interposer-abi.txt`: `libc.so.6` is
    /// glibc's SONAME and `libc.so` is what a musl-linked object records,
    /// because musl's libc declares no SONAME at all.
    pub fn of_needed(needed: &[String]) -> Flavour {
        for n in needed {
            match Flavour::of_libc_name(n) {
                Flavour::Unknown => continue,
                f => return f,
            }
        }
        Flavour::Unknown
    }

    /// What one library name means.
    ///
    /// ⚠ `libc.musl-*.so.1` and `ld-musl-*.so.1` are both musl's, and on musl
    /// they are the same file: the loader IS the C library.
    pub fn of_libc_name(name: &str) -> Flavour {
        let base = name.rsplit('/').next().unwrap_or(name);
        if base.starts_with("ld-musl-") || base.starts_with("libc.musl-") || base == "libc.so" {
            return Flavour::Musl;
        }
        if base == "libc.so.6" || base.starts_with("ld-linux") {
            return Flavour::Glibc;
        }
        Flavour::Unknown
    }
}

/// A symbol, and the symbol version attached to it.
///
/// ⛔ The version is half the answer and not a detail. Comparing names alone
/// misses the common real failure, which is a build host newer than the target:
/// every name resolves and the loader still refuses, naming a version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Versioned {
    pub name: String,
    /// `None` where the symbol carries no version, which is `VER_NDX_GLOBAL`.
    pub version: Option<String>,
    /// ⛔ **`STB_WEAK`, and an undefined one of these is not a requirement.**
    /// Measured on 2026-09-09: the glibc build of
    /// `references/VHSgunzo__pathmap/tree/path-mapping.c` imports
    /// `_ITM_deregisterTMCloneTable`, `_ITM_registerTMCloneTable` and
    /// `__gmon_start__`, none of which this host's `libc.so.6` defines, and it
    /// loads. GCC's `crtbegin` emits them weak so the loader binds them to 0;
    /// a reader that treated them as requirements refuses **every object gcc
    /// produces**, which reads exactly like a check that works.
    pub weak: bool,
}

/// Why an ELF file could not be read.
///
/// ⛔ Named rather than swallowed. A libc podbox cannot parse is a **refusal
/// with a reason**, never a pass: T-0109 rule 1, that a skip may not read as a
/// denial or as a pass, applies to a reader exactly as it does to a probe.
#[derive(Debug)]
pub struct Unreadable(pub String);

impl std::fmt::Display for Unreadable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// The parts of an ELF file T-0709 asks about.
#[derive(Debug, Clone)]
pub struct Elf {
    /// Where it was read from, for the refusal message.
    pub path: String,
    /// `e_machine`, so a rootfs of another architecture is refused by name
    /// rather than by a relocation error.
    pub machine: u16,
    /// `PT_INTERP`, where there is one. ⚠ A shared object usually has none;
    /// an executable that is not static-pie always does.
    pub interp: Option<String>,
    pub needed: Vec<String>,
    pub soname: Option<String>,
    /// ⛔ From `.dynsym`, never `.symtab`. Point 4 of this module's header.
    pub defined: Vec<Versioned>,
    pub imported: Vec<Versioned>,
    /// Every version name this file DECLARES, from `.gnu.version_d`.
    pub declares: Vec<String>,
}

impl Elf {
    /// Read `path`, or say why not.
    ///
    /// ⚠ Bounded at 128 MiB. `docs/AGENTS.md`: a runtime whose audience is
    /// automated may not assume a file it was pointed at is the size it
    /// expected, and `libc.so.6` is two.
    pub fn read(path: &str) -> Result<Elf, Unreadable> {
        const CEILING: u64 = 128 * 1024 * 1024;
        let len = std::fs::metadata(path)
            .map_err(|e| Unreadable(format!("{path}: {e}")))?
            .len();
        if len > CEILING {
            return Err(Unreadable(format!(
                "{path} is {len} bytes, over the {CEILING}-byte ceiling podbox \
                 reads an ELF file within"
            )));
        }
        let bytes = std::fs::read(path).map_err(|e| Unreadable(format!("{path}: {e}")))?;
        Elf::parse(path, &bytes)
    }

    /// The same, from bytes already in hand, which is what the tests drive.
    pub fn parse(path: &str, b: &[u8]) -> Result<Elf, Unreadable> {
        let why = |s: String| Unreadable(format!("{path}: {s}"));
        if b.len() < 64 || &b[0..4] != b"\x7fELF" {
            return Err(why("not an ELF file: the magic is not \\x7fELF".into()));
        }
        let class64 = match b[4] {
            1 => false,
            2 => true,
            c => return Err(why(format!("unknown ELF class {c}"))),
        };
        let msb = match b[5] {
            1 => false,
            2 => true,
            d => return Err(why(format!("unknown ELF data encoding {d}"))),
        };
        let r = Bytes { b, msb, class64 };

        let machine = r.u16(18).ok_or_else(|| why("truncated header".into()))?;
        let interp = r.interp();
        let sections = r.sections().ok_or_else(|| {
            why(
                "no section table podbox could read. ⛔ podbox refuses rather \
                 than preloading an object it could not check"
                    .into(),
            )
        })?;

        let mut out = Elf {
            path: path.to_string(),
            machine,
            interp,
            needed: Vec::new(),
            soname: None,
            defined: Vec::new(),
            imported: Vec::new(),
            declares: Vec::new(),
        };

        // `.dynamic`, for DT_NEEDED and DT_SONAME.
        if let Some(dynamic) = sections.get(".dynamic") {
            let strs = sections
                .by_index(dynamic.link as usize)
                .map(|s| s.slice(b))
                .unwrap_or_default();
            let step = if class64 { 16 } else { 8 };
            let body = dynamic.slice(b);
            let mut at = 0usize;
            while at + step <= body.len() {
                let d = Bytes {
                    b: &body[at..],
                    msb,
                    class64,
                };
                let (tag, val) = if class64 {
                    (d.u64(0).unwrap_or(0) as i64, d.u64(8).unwrap_or(0))
                } else {
                    (
                        d.u32(0).unwrap_or(0) as i32 as i64,
                        u64::from(d.u32(4).unwrap_or(0)),
                    )
                };
                match tag {
                    0 => break, // DT_NULL
                    1 => {
                        if let Some(s) = cstr(strs, val as usize) {
                            out.needed.push(s);
                        }
                    }
                    14 => out.soname = cstr(strs, val as usize),
                    _ => {}
                }
                at += step;
            }
        }

        // `.dynsym`, its strings, and the three version sections.
        if let Some(dynsym) = sections.get(".dynsym") {
            let strs = sections
                .by_index(dynsym.link as usize)
                .map(|s| s.slice(b))
                .unwrap_or_default();
            let versym = sections.get(".gnu.version").map(|s| s.slice(b));
            let need = sections
                .get(".gnu.version_r")
                .map(|s| r.verneed(s.slice(b), strs))
                .unwrap_or_default();
            let def = sections
                .get(".gnu.version_d")
                .map(|s| r.verdef(s.slice(b), strs))
                .unwrap_or_default();
            out.declares = def.values().cloned().collect();
            out.declares.sort();
            out.declares.dedup();

            let step = if class64 { 24 } else { 16 };
            let body = dynsym.slice(b);
            let mut i = 0usize;
            while (i + 1) * step <= body.len() {
                let s = Bytes {
                    b: &body[i * step..],
                    msb,
                    class64,
                };
                let name_off = s.u32(0).unwrap_or(0) as usize;
                // ⚠ `st_info` and `st_shndx` sit at different offsets in the two
                // classes, which is the whole reason this reader is class-aware
                // rather than a 64-bit one with a comment.
                let (info_at, shndx_at) = if class64 { (4, 6) } else { (12, 14) };
                let shndx = s.u16(shndx_at).unwrap_or(0);
                // `STB_WEAK` is binding 2, in the high nibble of `st_info`.
                let weak = s.b.get(info_at).map(|i| i >> 4) == Some(2);
                let ndx = versym
                    .and_then(|v| Bytes { b: v, msb, class64 }.u16(i * 2))
                    .unwrap_or(1);
                let Some(name) = cstr(strs, name_off) else {
                    i += 1;
                    continue;
                };
                if name.is_empty() {
                    i += 1;
                    continue;
                }
                // ⚠ 0 is VER_NDX_LOCAL and 1 is VER_NDX_GLOBAL; neither names a
                // version. The high bit is the "hidden" flag and not part of the
                // index.
                let key = ndx & 0x7fff;
                let version = if key < 2 {
                    None
                } else if shndx == 0 {
                    need.get(&key).cloned()
                } else {
                    def.get(&key).cloned()
                };
                let v = Versioned {
                    name,
                    version,
                    weak,
                };
                if shndx == 0 {
                    out.imported.push(v);
                } else {
                    out.defined.push(v);
                }
                i += 1;
            }
        }
        Ok(out)
    }

    /// Which libc this file belongs to.
    ///
    /// ⭐ For a **library**, the answer is its own SONAME: glibc's declares
    /// `libc.so.6` and musl's declares none. For anything else it is what the
    /// file asks for in `DT_NEEDED`, and for an executable its `PT_INTERP`
    /// settles it first, because the loader named there is the one that will
    /// actually run it.
    pub fn flavour(&self) -> Flavour {
        if let Some(i) = &self.interp {
            match Flavour::of_libc_name(i) {
                Flavour::Unknown => {}
                f => return f,
            }
        }
        if let Some(s) = &self.soname {
            match Flavour::of_libc_name(s) {
                Flavour::Unknown => {}
                f => return f,
            }
        }
        // ⚠ musl's libc declares no SONAME, so a file with none, no interpreter
        // and no libc in DT_NEEDED is identified by the name it was read under.
        match Flavour::of_needed(&self.needed) {
            Flavour::Unknown => match Flavour::of_libc_name(&self.path) {
                // ⭐ And a last answer from the CONTENT rather than the name,
                // for a libc copied out of an image under some other filename.
                // glibc's declares `GLIBC_*` version definitions; musl's
                // declares none at all and is its own program interpreter, so
                // it defines the entry point every C program starts at.
                Flavour::Unknown => {
                    if self.declares.iter().any(|d| d.starts_with("GLIBC_")) {
                        Flavour::Glibc
                    } else if self.declares.is_empty()
                        && self.defines(&Versioned {
                            name: "__libc_start_main".into(),
                            version: None,
                            weak: false,
                        })
                    {
                        Flavour::Musl
                    } else {
                        Flavour::Unknown
                    }
                }
                f => f,
            },
            f => f,
        }
    }

    fn defines(&self, want: &Versioned) -> bool {
        self.defined.iter().any(|d| d.name == want.name)
    }
}

/// What [`admits`] answers.
///
/// ⛔ Two outcomes and each is actionable. A refusal names the libc the rootfs
/// carries and the one the object was built against, which is
/// [`TODO/interpose.md`](../../../TODO/interpose.md) T-0706's channel and
/// T-0108's wording.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    Admitted,
    Refused(String),
}

impl Verdict {
    pub fn is_admitted(&self) -> bool {
        matches!(self, Verdict::Admitted)
    }
}

/// May `object` be preloaded into a payload served by `libc`?
///
/// ⛔ **Select and assert, never try.** The assertion is cheap, it runs before
/// anything is executed, and it turns a runtime failure inside somebody's
/// container into a refusal with a reason.
///
/// Four gates, in the order a loader would hit them:
///
/// 1. the **architecture**, because a relocation error is what an unchecked
///    mismatch produces;
/// 2. the **libc**, from `DT_NEEDED` against the libc's own SONAME. ⚠ This is
///    the coarse guard T-0709 asks for by name: a rootfs carrying `ld-musl-*`
///    handed a glibc object says so in those words;
/// 3. every **imported symbol** is defined by the libc, read from `.dynsym`;
/// 4. every **imported version** is one the libc declares.
pub fn admits(object: &Elf, libc: &Elf) -> Verdict {
    let refuse = |s: String| Verdict::Refused(s);
    if object.machine != libc.machine {
        return refuse(format!(
            "{} is ELF machine 0x{:x} and this rootfs's C library {} is 0x{:x}. \
             podbox refuses to preload an object of another architecture",
            object.path, object.machine, libc.path, libc.machine
        ));
    }
    let (want, have) = (Flavour::of_needed(&object.needed), libc.flavour());
    if have == Flavour::Unknown {
        return refuse(format!(
            "podbox could not identify the C library in {}: it declares no \
             SONAME podbox recognises and its name is not one either. ⛔ podbox \
             refuses rather than preloading an object it cannot check",
            libc.path
        ));
    }
    if want != have {
        return refuse(format!(
            "{} names {} in DT_NEEDED, so it was built against {}, and this \
             rootfs carries {} ({}). ⛔ A preloaded object is loaded by the \
             payload's OWN loader and resolves its imports against the payload's \
             libc, so this one cannot serve it",
            object.path,
            object.needed.join(", "),
            want.word(),
            have.word(),
            libc.path
        ));
    }
    for want in &object.imported {
        // ⛔ A WEAK undefined symbol is not a requirement: the loader binds it
        // to 0 and runs. See [`Versioned::weak`] for the three gcc emits into
        // every object it produces.
        if want.weak {
            continue;
        }
        // ⛔ **THE VERSION FIRST, and the order is the loader's own.** Measured
        // on 2026-09-09 by `experiments/80-interposer-abi.sh` check E against
        // the pinned glibc 2.31 payload: the object imports `dlsym@GLIBC_2.34`,
        // because glibc 2.34 merged `libdl` into `libc` and the build host is
        // 2.39, so the symbol is BOTH undefined there and at a version that
        // libc does not declare. The loader says `version 'GLIBC_2.34' not
        // found`; a reader that asked "is it defined" first said `dlsym` is
        // missing, which is true and sends the reader to look for libdl instead
        // of at the build host.
        if let Some(v) = &want.version {
            if !libc.declares.iter().any(|d| d == v) {
                return refuse(format!(
                    "{} imports `{}` at version `{v}`, and {} declares {}. ⛔ This \
                     is a build host newer than the target: the loader refuses \
                     naming the version, whatever the NAME resolves to",
                    object.path,
                    want.name,
                    libc.path,
                    if libc.declares.is_empty() {
                        "no versions at all".to_string()
                    } else {
                        format!("up to `{}`", newest(&libc.declares))
                    }
                ));
            }
        }
        if !libc.defines(want) {
            return refuse(format!(
                "{} imports `{}`, which {} does not define. ⚠ Read from .dynsym: \
                 a shipped libc is stripped and its .symtab defines nothing",
                object.path, want.name, libc.path
            ));
        }
    }
    Verdict::Admitted
}

/// The highest version among `declares`, for the message.
///
/// ⚠ A LEXICAL maximum over the numeric fields, which is right for the
/// `GLIBC_2.34` family and is only ever used in a sentence: nothing branches on
/// it, so a tie broken the other way costs a word rather than a decision.
fn newest(declares: &[String]) -> String {
    let key = |s: &String| -> (String, Vec<u64>) {
        let (name, rest) = s.split_once('_').unwrap_or((s.as_str(), ""));
        (
            name.to_string(),
            rest.split('.').filter_map(|p| p.parse().ok()).collect(),
        )
    };
    declares
        .iter()
        .max_by_key(|s| key(s))
        .cloned()
        .unwrap_or_default()
}

/// Where a rootfs's C library is, found from the payload's own interpreter.
///
/// ⭐ **No table of distribution paths.** `PT_INTERP` names the loader that will
/// run this payload. On musl that file IS the C library; on glibc the loader
/// lives in the same directory as `libc.so.6`, so the libc is the loader's own
/// directory plus that name. ⚠ Both are returned as paths INSIDE `rootfs`, so
/// the caller reads them before the chroot.
pub fn libc_beside(rootfs: &str, interp: &str) -> Option<String> {
    let rel = interp.trim_start_matches('/');
    match Flavour::of_libc_name(rel) {
        Flavour::Musl => Some(format!("{}/{rel}", rootfs.trim_end_matches('/'))),
        Flavour::Glibc => {
            let dir = rel.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
            let root = rootfs.trim_end_matches('/');
            let p = if dir.is_empty() {
                format!("{root}/libc.so.6")
            } else {
                format!("{root}/{dir}/libc.so.6")
            };
            std::path::Path::new(&p).exists().then_some(p)
        }
        Flavour::Unknown => None,
    }
}

// ------------------------------------------------------------------ the bytes
//
// ⚠ Every read is bounds-checked and returns `Option`. This module reads files
// out of somebody else's image, and a truncated or hostile one must produce a
// refusal rather than a panic inside `podbox run`.

struct Bytes<'a> {
    b: &'a [u8],
    msb: bool,
    class64: bool,
}

impl Bytes<'_> {
    fn u16(&self, at: usize) -> Option<u16> {
        let s: [u8; 2] = self.b.get(at..at + 2)?.try_into().ok()?;
        Some(if self.msb {
            u16::from_be_bytes(s)
        } else {
            u16::from_le_bytes(s)
        })
    }

    fn u32(&self, at: usize) -> Option<u32> {
        let s: [u8; 4] = self.b.get(at..at + 4)?.try_into().ok()?;
        Some(if self.msb {
            u32::from_be_bytes(s)
        } else {
            u32::from_le_bytes(s)
        })
    }

    fn u64(&self, at: usize) -> Option<u64> {
        let s: [u8; 8] = self.b.get(at..at + 8)?.try_into().ok()?;
        Some(if self.msb {
            u64::from_be_bytes(s)
        } else {
            u64::from_le_bytes(s)
        })
    }

    /// A field that is 8 bytes in ELF64 and 4 in ELF32.
    fn word(&self, at: usize) -> Option<u64> {
        if self.class64 {
            self.u64(at)
        } else {
            self.u32(at).map(u64::from)
        }
    }

    /// `PT_INTERP`'s contents, where the file has one.
    fn interp(&self) -> Option<String> {
        let (phoff, phentsize, phnum) = if self.class64 {
            (self.u64(32)?, self.u16(54)?, self.u16(56)?)
        } else {
            (u64::from(self.u32(28)?), self.u16(42)?, self.u16(44)?)
        };
        for i in 0..phnum as usize {
            let at = phoff as usize + i * phentsize as usize;
            let p = Bytes {
                b: self.b.get(at..)?,
                msb: self.msb,
                class64: self.class64,
            };
            if p.u32(0)? != 3 {
                continue; // PT_INTERP
            }
            let off = if self.class64 {
                p.u64(8)?
            } else {
                u64::from(p.u32(4)?)
            };
            return cstr(self.b, off as usize);
        }
        None
    }

    fn sections(&self) -> Option<Sections> {
        let (shoff, shentsize, mut shnum, mut shstrndx) = if self.class64 {
            (
                self.u64(40)?,
                self.u16(58)?,
                self.u16(60)? as usize,
                self.u16(62)? as usize,
            )
        } else {
            (
                u64::from(self.u32(32)?),
                self.u16(46)?,
                self.u16(48)? as usize,
                self.u16(50)? as usize,
            )
        };
        if shoff == 0 || shentsize == 0 {
            return None;
        }
        let one = |i: usize| -> Option<Section> {
            let at = shoff as usize + i * shentsize as usize;
            let s = Bytes {
                b: self.b.get(at..)?,
                msb: self.msb,
                class64: self.class64,
            };
            let (off_at, size_at, link_at) = if self.class64 {
                (24, 32, 40)
            } else {
                (16, 20, 24)
            };
            Some(Section {
                name_off: s.u32(0)? as usize,
                offset: s.word(off_at)? as usize,
                size: s.word(size_at)? as usize,
                link: s.u32(link_at)?,
            })
        };
        // ⚠ `e_shnum` 0 and `e_shstrndx` SHN_XINDEX both mean "the real value is
        // in section 0", which is how an object with more than 0xff00 sections
        // is encoded. A reader that took the header fields at face value would
        // read no sections at all from one.
        let zero = one(0)?;
        if shnum == 0 {
            shnum = zero.size;
        }
        if shstrndx == 0xffff {
            shstrndx = zero.link as usize;
        }
        let mut all = Vec::with_capacity(shnum);
        for i in 0..shnum {
            all.push(one(i)?);
        }
        let names = all.get(shstrndx)?.slice(self.b);
        let mut by_name = BTreeMap::new();
        for s in &all {
            if let Some(n) = cstr(names, s.name_off) {
                by_name.insert(n, s.clone());
            }
        }
        Some(Sections { all, by_name })
    }

    /// `.gnu.version_d`: version index -> the name it declares.
    fn verdef(&self, body: &[u8], strs: &[u8]) -> BTreeMap<u16, String> {
        let mut out = BTreeMap::new();
        let mut at = 0usize;
        // ⚠ Bounded by the section rather than by the `vd_next` chain alone: a
        // hostile file can make that chain a loop.
        for _ in 0..1024 {
            let d = Bytes {
                b: match body.get(at..) {
                    Some(s) if s.len() >= 20 => s,
                    _ => break,
                },
                msb: self.msb,
                class64: self.class64,
            };
            let (flags, ndx, aux, next) = match (d.u16(2), d.u16(4), d.u32(12), d.u32(16)) {
                (Some(f), Some(a), Some(b), Some(c)) => (f, a, b as usize, c as usize),
                _ => break,
            };
            // ⛔ `VER_FLG_BASE` is the file's OWN SONAME wearing a version
            // entry's clothes, and it is not a version anything imports.
            // Collecting it put `libc.so.6` in `declares` beside `GLIBC_2.39`,
            // and it sorts after every real one.
            if flags & 0x1 != 0 {
                if next == 0 {
                    break;
                }
                at += next;
                continue;
            }
            // The FIRST verdaux is the version's own name; the ones after it are
            // the versions it inherits.
            if let Some(name) = d
                .u32(aux)
                .and_then(|off| cstr(strs, off as usize))
                .filter(|s| !s.is_empty())
            {
                out.insert(ndx & 0x7fff, name);
            }
            if next == 0 {
                break;
            }
            at += next;
        }
        out
    }

    /// `.gnu.version_r`: version index -> the name required at it.
    fn verneed(&self, body: &[u8], strs: &[u8]) -> BTreeMap<u16, String> {
        let mut out = BTreeMap::new();
        let mut at = 0usize;
        for _ in 0..1024 {
            let n = Bytes {
                b: match body.get(at..) {
                    Some(s) if s.len() >= 16 => s,
                    _ => break,
                },
                msb: self.msb,
                class64: self.class64,
            };
            let (cnt, aux, next) = match (n.u16(2), n.u32(8), n.u32(12)) {
                (Some(a), Some(b), Some(c)) => (a as usize, b as usize, c as usize),
                _ => break,
            };
            let mut a = aux;
            for _ in 0..cnt.min(1024) {
                let x = Bytes {
                    b: match n.b.get(a..) {
                        Some(s) if s.len() >= 16 => s,
                        _ => break,
                    },
                    msb: self.msb,
                    class64: self.class64,
                };
                let (other, name, anext) = match (x.u16(6), x.u32(8), x.u32(12)) {
                    (Some(o), Some(m), Some(k)) => (o, m as usize, k as usize),
                    _ => break,
                };
                if let Some(s) = cstr(strs, name) {
                    out.insert(other & 0x7fff, s);
                }
                if anext == 0 {
                    break;
                }
                a += anext;
            }
            if next == 0 {
                break;
            }
            at += next;
        }
        out
    }
}

#[derive(Debug, Clone)]
struct Section {
    name_off: usize,
    offset: usize,
    size: usize,
    link: u32,
}

impl Section {
    fn slice<'a>(&self, b: &'a [u8]) -> &'a [u8] {
        b.get(self.offset..self.offset.saturating_add(self.size))
            .unwrap_or_default()
    }
}

struct Sections {
    all: Vec<Section>,
    by_name: BTreeMap<String, Section>,
}

impl Sections {
    fn get(&self, name: &str) -> Option<&Section> {
        self.by_name.get(name)
    }

    fn by_index(&self, i: usize) -> Option<&Section> {
        self.all.get(i)
    }
}

/// A NUL-terminated string at `off` in `b`, where there is one.
fn cstr(b: &[u8], off: usize) -> Option<String> {
    let rest = b.get(off..)?;
    let end = rest.iter().position(|c| *c == 0).unwrap_or(rest.len());
    Some(String::from_utf8_lossy(&rest[..end]).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ⭐ Check A of `experiments/results/interposer-abi.txt`, as a predicate.
    #[test]
    fn the_soname_discriminator_is_the_one_that_was_measured() {
        assert_eq!(Flavour::of_libc_name("libc.so.6"), Flavour::Glibc);
        assert_eq!(Flavour::of_libc_name("libc.so"), Flavour::Musl);
        assert_eq!(
            Flavour::of_libc_name("/lib/ld-musl-x86_64.so.1"),
            Flavour::Musl
        );
        assert_eq!(
            Flavour::of_libc_name("/lib/libc.musl-aarch64.so.1"),
            Flavour::Musl
        );
        assert_eq!(
            Flavour::of_libc_name("/lib64/ld-linux-x86-64.so.2"),
            Flavour::Glibc
        );
        // ⛔ And a third state for anything else, never a guess.
        assert_eq!(Flavour::of_libc_name("libz.so.1"), Flavour::Unknown);
        assert_eq!(
            Flavour::of_needed(&["libgcc_s.so.1".into(), "libc.so.6".into()]),
            Flavour::Glibc
        );
    }

    /// ⛔ The reader reads THIS binary, which is the only ELF file every host
    /// running this test is guaranteed to have. ⚠ It is static-pie under the
    /// workspace's own `+crt-static`, so it has no `PT_INTERP` and no
    /// `DT_NEEDED`: what is asserted is that the header and the section table
    /// parse and that the machine is this architecture's.
    #[test]
    fn this_binarys_own_header_parses() {
        let me = std::env::current_exe().unwrap();
        let got = Elf::read(me.to_str().unwrap()).unwrap();
        assert_eq!(got.machine, podbox_probe::binfmt::SELF_MACHINE);
    }

    /// ⛔ **The `.symtab` trap, as an assertion rather than a comment.** A
    /// shipped `libc.so.6` has 0 defined symbols in `.symtab` and 3136 in
    /// `.dynsym` (`experiments/results/interposer-abi.txt` check D), so a reader
    /// asking `.symtab` refuses every artefact and reads exactly like a check
    /// that works. ⚠ This runs only where a system libc is present; where it is
    /// not, the assertion is skipped rather than passed, because "could not run"
    /// is a third state.
    #[test]
    fn a_shipped_libc_defines_its_symbols_in_dynsym() {
        let Some(p) = [
            "/lib/x86_64-linux-gnu/libc.so.6",
            "/lib64/libc.so.6",
            "/usr/lib/libc.so.6",
            "/lib/ld-musl-x86_64.so.1",
        ]
        .into_iter()
        .find(|p| std::path::Path::new(p).exists()) else {
            eprintln!("SKIP: this host ships no libc at a conventional path");
            return;
        };
        let libc = Elf::read(p).unwrap();
        assert!(
            libc.defined.len() > 100,
            "{p} defined only {} symbols; .symtab was read instead of .dynsym",
            libc.defined.len()
        );
        assert!(libc.defines(&Versioned {
            name: "open".into(),
            version: None,
            weak: false,
        }));
        assert_ne!(libc.flavour(), Flavour::Unknown, "{p}");
    }

    /// ⭐ Check C, as the predicate rather than as a run: an import at a version
    /// the libc does not declare is REFUSED, and the refusal names the version
    /// the loader would have named.
    #[test]
    fn an_import_at_a_version_the_libc_does_not_declare_is_refused() {
        let libc = fake(
            "/lib/libc.so.6",
            Some("libc.so.6"),
            &[],
            &[("open", None), ("snprintf", Some("GLIBC_2.4"))],
            &["GLIBC_2.4", "GLIBC_2.31"],
        );
        let obj = fake_import(
            "/i.so",
            &["libc.so.6"],
            &[("open", None), ("snprintf", Some("GLIBC_2.34"))],
        );
        let v = admits(&obj, &libc);
        let Verdict::Refused(why) = v else {
            panic!("admitted an import at GLIBC_2.34 against a libc declaring 2.31");
        };
        assert!(why.contains("GLIBC_2.34"), "{why}");
        assert!(why.contains("GLIBC_2.31"), "{why}");
    }

    /// ⛔ The coarse guard T-0709 asks for by name: a musl rootfs handed a glibc
    /// object says so in those words, not through a relocation error.
    #[test]
    fn a_glibc_object_against_a_musl_rootfs_is_refused_in_those_words() {
        let libc = fake(
            "/lib/ld-musl-x86_64.so.1",
            None,
            &[],
            &[("open", None)],
            &[],
        );
        let obj = fake_import("/i.so", &["libc.so.6"], &[("open", None)]);
        let Verdict::Refused(why) = admits(&obj, &libc) else {
            panic!("a glibc object was admitted into a musl rootfs");
        };
        assert!(why.contains("musl"), "{why}");
        assert!(why.contains("glibc"), "{why}");
    }

    /// ⚠ And the matching pair is ADMITTED, or the check would be a refusal
    /// rather than a gate.
    #[test]
    fn a_matching_object_and_libc_are_admitted() {
        let libc = fake(
            "/lib/libc.so.6",
            Some("libc.so.6"),
            &[],
            &[("open", None), ("snprintf", Some("GLIBC_2.4"))],
            &["GLIBC_2.4"],
        );
        let obj = fake_import(
            "/i.so",
            &["libc.so.6"],
            &[("open", None), ("snprintf", Some("GLIBC_2.4"))],
        );
        assert_eq!(admits(&obj, &libc), Verdict::Admitted);
    }

    /// ⛔ A symbol the libc does not define at all is refused before the version
    /// question is asked, and the message names the symbol.
    #[test]
    fn an_undefined_symbol_is_refused_by_name() {
        let libc = fake(
            "/lib/libc.so.6",
            Some("libc.so.6"),
            &[],
            &[("open", None)],
            &[],
        );
        let obj = fake_import("/i.so", &["libc.so.6"], &[("__snprintf_chk", None)]);
        let Verdict::Refused(why) = admits(&obj, &libc) else {
            panic!("admitted an object importing a symbol the libc does not define");
        };
        assert!(why.contains("__snprintf_chk"), "{why}");
        assert!(why.contains(".dynsym"), "{why}");
    }

    /// ⚠ The libc is derived from the payload's own interpreter rather than
    /// looked up in a table of distributions.
    #[test]
    fn the_libc_is_found_beside_the_interpreter() {
        let d = std::env::temp_dir().join(format!("podbox-abi-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("lib/x86_64-linux-gnu")).unwrap();
        std::fs::write(d.join("lib/x86_64-linux-gnu/libc.so.6"), b"x").unwrap();
        let root = d.to_string_lossy().to_string();
        assert_eq!(
            libc_beside(&root, "/lib/x86_64-linux-gnu/ld-linux-x86-64.so.2"),
            Some(format!("{root}/lib/x86_64-linux-gnu/libc.so.6"))
        );
        // ⭐ On musl the interpreter IS the C library, so no second file is
        // looked for and none has to exist.
        assert_eq!(
            libc_beside(&root, "/lib/ld-musl-x86_64.so.1"),
            Some(format!("{root}/lib/ld-musl-x86_64.so.1"))
        );
        // ⛔ And an interpreter podbox does not recognise gets no answer rather
        // than a guessed one.
        assert_eq!(libc_beside(&root, "/opt/weird/loader"), None);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn the_newest_declared_version_is_reported_numerically() {
        // ⛔ Not lexically over the whole string: `GLIBC_2.9` sorts after
        // `GLIBC_2.34` as text and is older.
        let v: Vec<String> = ["GLIBC_2.4", "GLIBC_2.34", "GLIBC_2.9"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(newest(&v), "GLIBC_2.34");
    }

    #[test]
    fn a_file_that_is_not_elf_is_a_named_refusal_and_not_a_panic() {
        let e = Elf::parse("/x", b"#!/bin/sh\necho hi\n").unwrap_err();
        assert!(format!("{e}").contains("not an ELF file"), "{e}");
        // ⛔ And a truncated one, which is the shape a hostile image takes.
        let e = Elf::parse("/x", b"\x7fELF\x02\x01").unwrap_err();
        assert!(format!("{e}").contains("not an ELF file"), "{e}");
    }

    // ------------------------------------------------------------ fixtures

    fn fake(
        path: &str,
        soname: Option<&str>,
        needed: &[&str],
        defined: &[(&str, Option<&str>)],
        declares: &[&str],
    ) -> Elf {
        Elf {
            path: path.to_string(),
            machine: podbox_probe::binfmt::SELF_MACHINE,
            interp: None,
            needed: needed.iter().map(|s| (*s).to_string()).collect(),
            soname: soname.map(str::to_string),
            defined: defined.iter().map(|(n, v)| ver(n, *v)).collect(),
            imported: Vec::new(),
            declares: declares.iter().map(|s| (*s).to_string()).collect(),
        }
    }

    fn fake_import(path: &str, needed: &[&str], imported: &[(&str, Option<&str>)]) -> Elf {
        let mut e = fake(path, None, needed, &[], &[]);
        e.imported = imported.iter().map(|(n, v)| ver(n, *v)).collect();
        e
    }

    fn ver(name: &str, version: Option<&str>) -> Versioned {
        Versioned {
            name: name.to_string(),
            version: version.map(str::to_string),
            weak: false,
        }
    }
}
